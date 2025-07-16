//! # IO Module
//!
//! Registered as `io`. It allows the user to read and write to files
//!
//! ## Functions
//! * `io$read-file` `[ <str> ][ <result<str><str>> ]`
//! * `io$write-file` `[ content<str> path<str> ][ <result<num><str>> ]`

use crate::{
    RuntimeContext, RuntimeErrorKind, Value,
    runtime::{Hook, module::Module, sget, stack_pop},
};
use std::{io::Write, path::Path};

fn read_file(ctx: &mut RuntimeContext, _: &Path) -> Result<(), RuntimeErrorKind> {
    let path = stack_pop!((ctx.stack) -> str as "file path" for "io$read-file")?;
    let content = std::fs::read_to_string(path);
    let r = match content {
        Ok(o) => Ok(Value::from(o)),
        Err(e) => Err(Value::from(e.to_string())),
    };
    ctx.stack.push_this(r);
    Ok(())
}

fn write_file(ctx: &mut RuntimeContext, _: &Path) -> Result<(), RuntimeErrorKind> {
    let path = stack_pop!((ctx.stack) -> str as "file path" for "io$read-file")?;
    let content = stack_pop!((ctx.stack) -> str as "file content" for "io$read-file")?;
    let file = std::fs::OpenOptions::new().append(true).open(path);
    let mut file = match file {
        Ok(o) => o,
        Err(e) => {
            ctx.stack.push_this(Err(e.to_string().into()));
            return Ok(());
        }
    };
    let res = match file.write_all(content.as_bytes()) {
        Err(e) => Err((e.to_string()).into()),
        Ok(()) => Ok((content.len() as isize).into()),
    };
    ctx.stack.push_this(res);
    Ok(())
}

pub fn make() -> Module {
    let mut io_mod = Module::empty("io".to_string());
    io_mod.add_fn("io$read-file", Hook::WithError(read_file));
    io_mod.add_fn("io$write-file", Hook::WithError(write_file));
    io_mod
}
