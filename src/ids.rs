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


use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter, Result};
use index_vec::IndexVec;
use crate::{ReactorBehavior, ReactorInitializer};
use core::any::type_name;

// private implementation types
type ReactionIdImpl = u16;
type ReactorIdImpl = u16;
pub(in crate) type GlobalIdImpl = u32;

define_index_type! {
    /// Type of a local reaction ID
    pub struct LocalReactionId = ReactionIdImpl;
    DISABLE_MAX_INDEX_CHECK = cfg!(not(debug_assertions));
    DISPLAY_FORMAT = "{}";
}

impl LocalReactionId {
    pub const ZERO: LocalReactionId = LocalReactionId::new_const(0);

    // a const fn to be able to use this in const context
    pub const fn new_const(u: ReactionIdImpl) -> Self {
        Self { _raw: u }
    }
}


define_index_type! {
    /// The unique identifier of a reactor instance during
    /// execution.
    pub struct ReactorId = ReactorIdImpl;
    DISABLE_MAX_INDEX_CHECK = cfg!(not(debug_assertions));
    DISPLAY_FORMAT = "{}";
    DEFAULT = Self::new(0);
}

impl ReactorId {
    // a const fn to be able to use this in const context
    pub const fn new_const(u: ReactorIdImpl) -> Self {
        Self { _raw: u }
    }
}

macro_rules! global_id_newtype {
    ($id:ident) => {
        #[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Copy, Clone)]
        pub struct $id(pub(in crate) GlobalId);

        impl $id {
            pub fn new(container: $crate::ReactorId, local: $crate::LocalReactionId) -> Self {
                Self($crate::GlobalId::new(container, local))
            }
        }

        impl Debug for $id {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }

        impl Display for $id {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

// Identifies a component of a reactor using the ID of its container
// and a local component ID.
global_id_newtype!(GlobalReactionId);

// Identifies a trigger (port, action, timer)
global_id_newtype!(TriggerId);

/// A set of reactions
pub type ReactionSet = Vec<GlobalReactionId>;


/// Identifies a component of a reactor using the ID of its container
/// and a local component ID.
#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Debug, Copy, Clone)]
pub struct GlobalId {
    _raw: GlobalIdImpl,
}


impl GlobalId {
    pub fn new(container: ReactorId, local: LocalReactionId) -> Self {
        let _raw: GlobalIdImpl = (container._raw as GlobalIdImpl) << ReactionIdImpl::BITS | (local._raw as GlobalIdImpl);
        Self { _raw }
    }

    #[cfg(test)]
    pub fn next_id(&self) -> GlobalId {
        // todo check overflow
        assert_ne!(self.local(), 0xffff, "Overflow while allocating next id");
        Self { _raw: self._raw + 1 }
    }

    #[cfg(test)]
    pub const fn first_id() -> GlobalId {
        GlobalId { _raw: 0 }
    }

    pub(in crate) const fn container(&self) -> ReactorId {
        ReactorId::new_const((self._raw >> 16) as u16)
    }

    pub(in crate) const fn local(&self) -> LocalReactionId {
        LocalReactionId::new_const((self._raw & 0xffff) as u16)
    }
}

impl Display for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}/{}", self.container(), self.local())
    }
}


pub(in crate) trait GloballyIdentified {
    fn get_id(&self) -> GlobalId;
}

pub trait TriggerLike {
    fn get_id(&self) -> TriggerId;
}


pub type PortId = GlobalId;

#[derive(Clone)]
pub(in crate) struct ReactorDebugInfo {
    /// Type name
    pub type_name: &'static str,
    /// Simple name of the instantiation (last segment of the path)
    pub inst_name: &'static str,
    /// Path to this instantiation (eg "/parent/child")
    inst_path: String,
}

impl ReactorDebugInfo {
    pub(in crate) fn root<R>() -> Self {
        Self {
            type_name: type_name::<R>(),
            inst_name: "<main>",
            inst_path: "<main>".into(),
        }
    }

    pub(in crate) fn derive<R: ReactorInitializer>(&self, inst_name: &'static str) -> Self {
        Self {
            type_name: type_name::<R::Wrapped>(),
            inst_name,
            inst_path: format!("{}/{}", self.inst_path, inst_name),
        }
    }
}

impl Display for ReactorDebugInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.inst_path)
    }
}


/// Stores a mapping from global Id to debug label
#[derive(Default)]
pub(in crate) struct IdRegistry {
    debug_ids: HashMap<GlobalId, &'static str>,
    reactor_infos: IndexVec<ReactorId, ReactorDebugInfo>,
}

impl IdRegistry {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_debug_label(&self, id: GlobalId) -> Option<&'static str> {
        self.debug_ids.get(&id).map(|it| *it)
    }

    pub fn get_debug_info(&self, id: ReactorId) -> &ReactorDebugInfo {
        &self.reactor_infos[id]
    }

    #[inline]
    pub fn fmt_reaction(&self, id: GlobalReactionId) -> impl Display {
        let mut str = format!("{}:{}", &self.reactor_infos[id.0.container()], id.0.local());
        // reactions may have labels too
        if let Some(label) = self.get_debug_label(id.0) {
            str += "@";
            str += label;
        }
        str
    }

    pub(in super) fn record(&mut self, id: GlobalId, name: &'static str) {
        let existing = self.debug_ids.insert(id, name);
        debug_assert!(existing.is_none())
    }

    pub(in super) fn record_reactor(&mut self, id: ReactorId, debug: &ReactorDebugInfo) {
        let ix = self.reactor_infos.push(debug.clone());
        println!("reactor {}: {}", id, debug);
        assert_eq!(ix, id);
    }
}
