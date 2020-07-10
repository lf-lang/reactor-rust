use std::borrow::Borrow;
use std::cell::{Ref, RefCell};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::reactors::assembler::{GraphElement, Stamped};

use super::assembler::Assembler;

#[derive(Debug)]
pub struct InPort<T> {
    name: &'static str,

    /// The binding for an input port is the output port to
    /// which it is connected.
    ///
    /// RefCell<            // For internal mutability
    /// Option<             // The port may be unbound
    /// Rc<                 // The referenced output port may be referenced by several input ports
    /// OutPort<T>>>>       // Finally the value!
    ///
    binding: RefCell<Option<Rc<OutPort<T>>>>,
}

impl<'a, T> GraphElement<'a> for InPort<T> {}

impl<T> InPort<T> {
    pub fn new<'a>(assembler: &mut Assembler<'a>, name: &'static str) -> Stamped<'a, InPort<T>>
        where T: 'static {
        assembler.create_node(
            InPort {
                name,
                binding: RefCell::new(None),
            }
        )
    }

    pub fn bind(&self, binding: &Rc<OutPort<T>>) {
        // that it's important that the borrow here is dropped before borrow_mut is called
        //                                        vvvvvv
        if let Some(b) = self.binding.borrow().as_deref() {
            panic!("Input port {} already bound to {}", self.name, b.name)
        }
        *self.binding.borrow_mut() = Some(Rc::clone(binding))
    }

    pub fn borrow_or_panic(&self) -> Rc<OutPort<T>> {
        let x = self.binding.borrow();
        match &*x {
            Some(output) => Rc::clone(output),
            None => panic!("No binding for port {}", self.name)
        }
    }
}


/// An OutPort is an internally mutable container for a value
/// In reactors, ports should be wrapped inside an Rc; when
/// linked to an input port of another reactor, that Rc should
/// be cloned.
#[derive(Debug)]
pub struct OutPort<T> {
    name: &'static str,
    /// Mutable container for the value
    cell: RefCell<T>,
}


impl<T> OutPort<T> {
    pub fn new<'a>(assembler: &mut Assembler<'a>, name: &'static str, initial_val: T) -> Stamped<'a, OutPort<T>>
        where T: 'static {
        assembler.create_node(
            OutPort { name, cell: RefCell::new(initial_val) }
        )
    }

    pub fn set(&self, new_val: T) {
        *self.cell.borrow_mut() = new_val
    }

    pub fn get<'a>(&'a self) -> impl Deref<Target=T> + 'a {
        self.cell.borrow()
    }

    pub fn get_mut(&self) -> impl DerefMut<Target=T> + '_ {
        self.cell.borrow_mut()
    }
}

impl<'a, T> GraphElement<'a> for OutPort<T> {}
