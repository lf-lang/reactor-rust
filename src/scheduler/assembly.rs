/*
 * Copyright (c) 2021, TU Dresden.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 *    this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
 * THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
 * THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

use crate::*;
use crate::scheduler::depgraph::DepGraph;
use index_vec::IndexVec;


pub(in super) struct RootAssembler {
    reactor_id: ReactorId,
    graph: DepGraph,
    reactors: IndexVec<ReactorId, Box<dyn ReactorBehavior + 'static>>,
    id_registry: IdRegistry,
}

impl Default for RootAssembler {
    fn default() -> Self {
        Self {
            reactor_id: ReactorId::new(0),
            graph: Default::default(),
            id_registry: Default::default(),
            reactors: Default::default(),
        }
    }
}


/// Helper struct to assemble reactors during initialization.
/// One assembly context is used per reactor, they can't be shared.
pub struct AssemblyCtx<'x> {
    globals: &'x mut RootAssembler,
    /// Constant id of the reactor currently being built.
    reactor_id: Option<ReactorId>,
    /// Next local ID for components != reactions
    cur_local: LocalReactionId,
    /// Next ID for a reaction
    cur_reaction: LocalReactionId,
}

impl<'x> AssemblyCtx<'x> {
    /// The ID of the reactor being built.
    ///
    /// ### Panics
    /// If fix_cur_id has not been called.
    pub fn get_id(&self) -> ReactorId {
        self.reactor_id.unwrap_or_else(|| panic!("fix_cur_id has not been called"))
    }

    /// Note: this needs to be called after all children reactors
    /// have been built, as they're pushed into the global reactor
    /// vec before their parent. So the ID of the parent needs to
    /// be fixed only after all descendants have been built.
    pub fn fix_cur_id(&mut self) -> ReactorId {
        let id = self.globals.reactor_id;
        self.reactor_id = Some(id);
        self.globals.reactor_id += 1;
        id
    }

    pub fn new_port<T>(&mut self, lf_name: &'static str) -> Port<T> {
        let id = self.next_comp_id(Some(lf_name));
        self.globals.graph.record_port(id);
        Port::new(id)
    }

    pub fn new_logical_action<T: Clone>(&mut self,
                                        lf_name: &'static str,
                                        min_delay: Option<Duration>) -> LogicalAction<T> {
        let id = self.next_comp_id(Some(lf_name));
        self.globals.graph.record_laction(id);
        LogicalAction::new(id, min_delay)
    }

    pub fn new_physical_action<T: Clone>(&mut self,
                                         lf_name: &'static str,
                                         min_delay: Option<Duration>) -> PhysicalAction<T> {
        let id = self.next_comp_id(Some(lf_name));
        self.globals.graph.record_paction(id);
        PhysicalAction::new(id, min_delay)
    }

    pub fn new_timer(&mut self, lf_name: &'static str, offset: Duration, period: Duration) -> Timer {
        let id = self.next_comp_id(Some(lf_name));
        self.globals.graph.record_paction(id);
        Timer::new(id, offset, period)
    }

    pub fn new_reaction(&mut self, lf_name: Option<&'static str>) -> GlobalReactionId {
        let id = self.next_comp_id(lf_name);
        self.globals.graph.record_reaction(id);
        GlobalReactionId(id)
    }

    /// Create and return a new global id for a new component.
    ///
    /// ### Panics
    ///
    /// See [get_id].
    pub fn next_comp_id(&mut self, debug_name: Option<&'static str>) -> GlobalId {
        let id = GlobalId::new(self.get_id(), self.cur_local);
        if let Some(label) = debug_name {
            self.globals.id_registry.record(id, label);
        }
        self.cur_local += 1;
        id
    }

    /// Register a child reactor.
    pub fn register_reactor<S: ReactorInitializer + 'static>(&mut self, child: S) {
        let vec_id = self.globals.reactors.push(Box::new(child));
        debug_assert_eq!(self.globals.reactors[vec_id].id(), vec_id, "Improper initialization order!");
    }

    /// Assemble a child reactor. The child needs to be registered
    /// using [register_reactor] later.
    #[inline]
    pub fn assemble_sub<S: ReactorInitializer>(&mut self, args: S::Params) -> S {
        AssemblyCtx::assemble_impl(&mut self.globals, args)
    }

    #[inline]
    fn assemble_impl<S: ReactorInitializer>(globals: &mut RootAssembler, args: S::Params) -> S {
        let mut sub = AssemblyCtx::new::<S>(globals);
        S::assemble(args, &mut sub)
    }

    pub(in super) fn new<S: ReactorInitializer>(globals: &'x mut RootAssembler) -> Self {
        Self {
            globals,
            reactor_id: None,
            cur_local: S::MAX_REACTION_ID,
            cur_reaction: LocalReactionId::ZERO,
        }
    }
}