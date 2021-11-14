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

use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use index_vec::Idx;

// private implementation types
pub(crate) type ReactionIdImpl = u16;
pub(crate) type ReactorIdImpl = u16;
/// note: this must always be wide enough to concatenate a
/// [ReactorIdImpl] and a [ReactionIdImpl].
/// If size changes, you need to update the [Hash] implementation
/// for [GlobalId].
pub(crate) type GlobalIdImpl = u32;
assert_eq_size!(GlobalIdImpl, (ReactorIdImpl, ReactionIdImpl));

macro_rules! simple_idx_type {
    ($(#[$($attrs:tt)*])* $id:ident($impl_t:ty)) => {

$(#[$($attrs)*])*
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct $id($impl_t);

impl $id {
    // a const fn to be able to use this in const context
    pub const fn new(u: $impl_t) -> Self {
        Self(u)
    }

    pub const fn raw(self) -> $impl_t {
        self.0
    }

    pub(crate) fn plus(&self, u: usize) -> Self {
        Self::from_usize(self.0 as usize + u)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    #[allow(unused)]
    pub(crate) fn get_and_incr(&mut self) -> Self {
        let id = *self;
        *self = Self(self.0 + 1);
        id
    }
}

impl Idx for $id {
    fn from_usize(idx: usize) -> Self {
        debug_assert!(idx <= <$impl_t>::MAX as usize);
        Self(idx as $impl_t)
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

impl Display for $id {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}
    };
}

simple_idx_type! {
    /// ID of a reaction local to its containing reactor.
    LocalReactionId(ReactionIdImpl)
}

simple_idx_type! {
    /// The unique identifier of a reactor instance during
    /// execution.
    ReactorId(ReactorIdImpl)
}

macro_rules! global_id_newtype {
    {$(#[$m:meta])* $id:ident} => {
        $(#[$m])*
        #[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Copy, Clone)]
        pub struct $id(pub(crate) GlobalId);

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

global_id_newtype! {
    /// Global identifier for a reaction.
    GlobalReactionId
}

/// Identifies a component of a reactor using the ID of its container
/// and a local component ID.
#[derive(Eq, Ord, PartialOrd, PartialEq, Copy, Clone)]
pub(crate) struct GlobalId {
    container: ReactorId,
    local: LocalReactionId,
}

impl GlobalId {
    pub fn new(container: ReactorId, local: LocalReactionId) -> Self {
        Self { container, local }
    }

    pub(crate) const fn container(&self) -> ReactorId {
        self.container
    }

    pub(crate) const fn local(&self) -> LocalReactionId {
        self.local
    }
}

impl FromStr for GlobalId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((container, local)) = s.split_once('/') {
            let container = container.parse::<ReactorIdImpl>().map_err(|_| "invalid reactor id")?;
            let local = local.parse::<ReactionIdImpl>().map_err(|_| "invalid local id")?;
            Ok(GlobalId::new(ReactorId::new(container), LocalReactionId::new(local)))
        } else {
            Err("Expected format {int}/{int}")
        }
    }
}

// hashing global ids is a very hot operation in the framework,
// therefore we give it an optimal implementation
impl Hash for GlobalId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // safety: this has the same size and the same layout basically,
        // since both ReactorId and LocalReactionId are declared #[repr(transparent)]
        state.write_u32(unsafe { std::mem::transmute(*self) })
    }
}

impl Debug for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Display>::fmt(self, f)
    }
}

impl Display for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}/{}", self.container(), self.local())
    }
}
