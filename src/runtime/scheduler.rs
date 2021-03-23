

use std::cmp::Reverse;



use std::hash::{Hash};
use std::marker::PhantomData;


use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;


use crate::runtime::{Logical, LogicalAction, Physical, PhysicalAction, ReactorAssembler};
use crate::runtime::ports::{InputPort, OutputPort};

use super::{Action, Dependencies, ReactionInvoker};
use super::time::*;
use bitset_fixed::BitSet;
use std::cell::Cell;

/// An order to execute some reaction
type ReactionOrder = Arc<ReactionInvoker>;
type TimeCell = Arc<Mutex<Cell<LogicalTime>>>;

#[derive(Eq, PartialEq, Hash)]
struct Event {
    process_at: LogicalTime,
    todo: Vec<ReactionOrder>,
}

/// Main public API for the scheduler. Contains the priority queue
/// and public launch routine with event loop.
pub struct SyncScheduler {
    /// The latest processed logical time (necessarily behind physical time)
    cur_logical_time: TimeCell,

    /// The receiver end of the communication channels. Reactions
    /// contexts each have their own [Sender]. The main event loop
    /// polls this to make progress.
    ///
    /// That the receiver is unique.
    receiver: Receiver<Event>,

    /// A sender bound to the receiver, which may be cloned.
    canonical_sender: Sender<Event>,
    // todo the priority (Reverse<LogicalTime>) should take into account relative reaction priority
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,
    max_reaction_id: u32,
}

impl SyncScheduler {
    /// Creates a new scheduler. Its state is initialized to nothing.
    pub fn new(max_reaction_id: u32) -> Self {
        let (sender, receiver) = channel::<Event>();
        Self {
            cur_logical_time: <_>::default(),
            receiver,
            canonical_sender: sender,
            queue: PriorityQueue::new(),
            max_reaction_id,
        }
    }

    pub fn launch_async(mut self, timeout: Duration) -> JoinHandle<()> {
        use std::thread;
        thread::spawn(move || {
            loop {
                if let Ok(evt) = self.receiver.recv_timeout(timeout) {
                    // received a new event

                    let eta = evt.process_at;      // logical time of the processing
                    self.queue.push(evt, Reverse(eta)); // maybe some other event is expected to be processed before
                    let (evt, Reverse(eta)) = self.queue.pop().unwrap();

                    self.step(evt, eta);
                } else {
                    // all senders have hung up
                    #[cfg(not(feature = "benchmarking"))] {
                        eprintln!("Shutting down scheduler, channel timed out after {} ms", timeout.as_millis());
                    }
                    assert!(self.queue.len() == 0);
                    break;
                }
            }
        })
    }

    fn step(&mut self, event: Event, process_at_time: LogicalTime) {
        let time = Self::catch_up_physical_time(process_at_time);
        self.cur_logical_time.lock().unwrap().set(time);
        self.new_wave(time, event.todo).consume();
    }

    fn catch_up_physical_time(up_to_time: LogicalTime) -> LogicalTime {
        let now = Instant::now();
        if now < up_to_time.instant {
            let t = up_to_time.instant - now;
            std::thread::sleep(t);
            LogicalTime::now()
        } else {
            LogicalTime { instant: now, microstep: 0 }
        }
    }

    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    fn new_wave(&self, logical_time: LogicalTime, reactions: Vec<ReactionOrder>) -> ReactionWave {
        ReactionWave {
            logical_time,
            todo: reactions.iter().map(|r| Some(r.clone())).collect::<Vec<_>>(),
            done: BitSet::new(self.max_reaction_id as usize),
            sender: self.canonical_sender.clone(),
        }
    }

    pub fn start(&self, r: &mut impl ReactorAssembler) {
        let ctx = SchedulerLink {
            last_processed_logical_time: self.cur_logical_time.clone(),
            sender: self.canonical_sender.clone(),
        };
        let mut startup_wave = self.new_wave(LogicalTime::now(), vec![]);
        r.start(ctx, &mut startup_wave.new_ctx())
    }
}

/// A "wave" of reactions executing at the same logical time.
/// Waves can enqueue new reactions to execute at the same time,
/// they're processed in exec order.
///
/// todo would there be a way to "split" waves into workers?
struct ReactionWave {
    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    logical_time: LogicalTime,

    /// Remaining reactions to execute before the wave dies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in order into this
    /// queue.
    todo: Vec<Option<ReactionOrder>>,

    /// The set of reactions that have been processed (or scheduled)
    /// in this wave, used to avoid duplication. todo this is a bad idea
    done: BitSet,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<Event>,

}

impl ReactionWave {
    /// Add new reactions to execute in the same wave.
    /// TODO topology information & deduplication
    ///  Eg for a diamond situation this will execute reactions several times...
    ///  This is why I added a bitset to patch it, but the size of it is really bad.
    ///
    fn enqueue_now(&mut self, downstream: Dependencies) {
        for reaction in downstream.reactions.iter() {
            let rid = reaction.id() as usize;
            if !self.done[rid] {
                self.done.set(rid, true);
                self.todo.push(Some(reaction.clone()));
            }
        }
    }

    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    fn enqueue_later(&mut self, downstream: &Dependencies, process_at: LogicalTime) {
        debug_assert!(process_at > self.logical_time);

        // todo merge events at equal tags by merging their dependencies
        let evt = Event { process_at, todo: downstream.reactions.clone() };
        self.sender.send(evt).unwrap();
    }

    fn new_ctx(&mut self) -> LogicalCtx {
        LogicalCtx { scheduler: self }
    }

    /// Execute the wave until completion
    fn consume(mut self) {
        let mut i = 0;
        while i < self.todo.len() {
            if let Some(reaction) = self.todo[i].take() {
                // this might add some elements to the vec, but only at the end
                // todo this is bad memory wise, we keep using memory for the prefix of the list that's already been processed
                reaction.fire(&mut self.new_ctx())
            }
            i += 1;
        }
    }
}

/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct LogicalCtx<'a> {
    scheduler: &'a mut ReactionWave,
}

impl LogicalCtx<'_> {
    /// Get the value of a port at this time.
    pub fn get<T: Copy>(&self, port: &InputPort<T>) -> Option<T> {
        port.get()
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    pub fn set<T>(&mut self, port: &mut OutputPort<T>, value: T) {
        let downstream = port.set(value);
        self.scheduler.enqueue_now(downstream);
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    pub fn schedule(&mut self, action: &LogicalAction, offset: Offset) {
        self.schedule_impl(action, offset);
    }

    // private
    fn schedule_impl<T>(&mut self, action: &Action<T>, offset: Offset) {
        self.scheduler.enqueue_later(&action.downstream, action.make_eta(self.scheduler.logical_time, offset.to_duration()));
    }

    pub fn get_physical_time(&self) -> Instant {
        Instant::now()
    }

    /// Request a shutdown which will be acted upon at the end
    /// of this reaction.
    pub fn request_shutdown(self) {
        // todo
        // self.scheduler.shutdown()
    }

    pub fn get_logical_time(&self) -> LogicalTime {
        self.scheduler.logical_time
    }
}

/// A type that can affect the logical event queue to implement
/// asynchronous physical actions. This is a "link" to the event
/// system, from the outside work.
pub struct SchedulerLink {
    last_processed_logical_time: TimeCell,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<Event>,
}

impl SchedulerLink {
    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    pub fn schedule_physical(&mut self, action: &PhysicalAction, offset: Offset) {
        // we have to fetch the time at which the logical timeline is currently running,
        // this may be far behind the current physical time
        let time_in_logical_subsystem = self.last_processed_logical_time.lock().unwrap().get();
        let process_at = action.make_eta(time_in_logical_subsystem, offset.to_duration());

        // todo merge events at equal tags by merging their dependencies
        let evt = Event { process_at, todo: action.downstream.reactions.clone() };
        self.sender.send(evt).unwrap();
    }
}
