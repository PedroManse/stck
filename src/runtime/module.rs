//! # Host-defined stck functions
//!
//! Modules are named collections of functions that can be registered on a runtime environment for
//! users to execute.
//!
//! They can be created with [`Module::empty`], named [Hooks](Hook) can be added with
//! [`Module::add_fn`] and can be registered on a runtime with
//! [`RuntimeContext::register_modules`](crate::RuntimeContext::register_modules), then all the
//! functions from the module will be avaliable to the user by their name as if they where builtin.
//!
//! After registering a module it's impossible to remove it's functions from a runtime context.
//!
//! By default runtime is instantiated with the modules: [ [`debug`], [`io`] and [`map`] ]. But it
//! is possible to create a runtime with no loaded modules with
//! [`RuntimeContext::new_raw`](crate::RuntimeContext::new_raw)
//!

use super::Hook;
use std::collections::HashMap;

/// # A named collection of functions
#[derive(Clone)]
pub struct Module {
    pub(crate) name: String,
    pub(crate) funcs: HashMap<crate::FnName, Hook>,
}

impl Module {
    /// Create an empty module
    #[must_use]
    pub fn empty(name: String) -> Module {
        Module {
            name,
            funcs: HashMap::new(),
        }
    }
    /// Add a function to a module
    pub fn add_fn(&mut self, name: impl Into<String>, fnc: Hook) -> Option<Hook> {
        self.funcs.insert(name.into(), fnc)
    }
}

impl IntoIterator for Module {
    type Item = Module;
    type IntoIter = std::iter::Once<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self)
    }
}

pub mod debug;
pub mod io;
pub mod map;

#[must_use]
pub(crate) fn get_builtin_modules() -> impl IntoIterator<Item = Module> {
    [debug::make(), io::make(), map::make()]
}
