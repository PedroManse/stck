use crate::{FnName, StckError};
use std::collections::HashMap;
use super::Hook;

macro_rules! register {
    ($mod:expr, $name:ident as |$ctx:ident| $fn:block) => {
        fn $name($ctx: &mut RuntimeContext, _: &Path) -> Result<(), RuntimeErrorKind> $fn
        $mod.add_fn(format!("io${}", stringify!($name)), Hook::WithError($name));
    };
    ($mod:expr, $name:ident as |$ctx:ident, $path: ident| $fn:block) => {
        fn $name($ctx: &mut RuntimeContext, $path: &Path) -> Result<(), RuntimeErrorKind> $fn
        $mod.add_fn(format!("io${}", stringify!($name)), Hook::WithError($name));
    };
}
pub(crate) use register;

#[derive(Clone)]
pub struct Module {
    pub(crate) name: String,
    pub(crate) funcs: HashMap<FnName, Hook>,
}

impl Module {
    pub fn new(name: String) -> Result<Module, StckError> {
        if name.starts_with('#') {
            Err(StckError::UserModuleWithBang(name))
        } else {
            Ok(Module {
                name,
                funcs: HashMap::new(),
            })
        }
    }
    fn new_protected(name: &'static str) -> Module {
        let name = format!("#{name}");
        Module {
            name,
            funcs: HashMap::new(),
        }
    }
    pub fn add_fn(&mut self, name: impl Into<String>, fnc: Hook) -> Option<Hook> {
        self.funcs.insert(name.into(), fnc)
    }
}

pub trait IntoModules {
    fn into_modules(self) -> impl Iterator<Item = Module>;
}

impl IntoModules for Module {
    fn into_modules(self) -> impl Iterator<Item = Module> {
        std::iter::once(self)
    }
}

impl<II> IntoModules for II
where
    II: IntoIterator<Item = Module>,
{
    fn into_modules(self) -> impl Iterator<Item = Module> {
        self.into_iter()
    }
}

pub mod debug;
pub mod io;
pub mod map;

#[deprecated]
pub mod oficial {
    pub use super::debug::make as debug;
    pub use super::io::make as io;
    #[allow(deprecated)]
    pub use super::io::io_module;
}

pub fn builtin_modules() -> impl IntoModules {
    [
        debug::make(),
        io::make(),
        map::make(),
    ]
}

