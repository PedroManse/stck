use crate::{
    runtime::{Hook, module::Module, sget, stack_pop},
    RuntimeContext, RuntimeErrorKind, StckError, Value,
};
use std::{io::Write, path::Path};
use super::register;

pub fn io() -> Module {
    let mut io_mod = Module::new_protected("io");

    register!(io_mod, read_file as |ctx| {
        let path = stack_pop!((ctx.stack) -> str as "file path" for "io$read-file")?;
        let content = std::fs::read_to_string(path);
        let r = match content {
            Ok(o) => Ok(Value::from(o)),
            Err(e) => Err(Value::from(e.to_string())),
        };
        ctx.stack.push_this(r);
        Ok(())
    });

    register!(io_mod, write_file as |ctx| {
        let path = stack_pop!((ctx.stack) -> str as "file path" for "io$read-file")?;
        let content = stack_pop!((ctx.stack) -> str as "file content" for "io$read-file")?;
        let file = std::fs::File::create(path);
        let mut file = match file {
            Ok(o) => o,
            Err(e) => {
                ctx.stack.push_this(Err(e.to_string().into()));
                return Ok(());
            }
        };
        let res = match file.write_all(content.as_bytes()) {
            Err(e)=> Err((e.to_string()).into()),
            Ok(()) => Ok((content.len() as isize).into()),
        };
        ctx.stack.push_this(res);
        Ok(())
    });

    register!(io_mod, append_file as |ctx| {
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
            Err(e)=> Err((e.to_string()).into()),
            Ok(()) => Ok((content.len() as isize).into()),
        };
        ctx.stack.push_this(res);
        Ok(())
    });

    io_mod
}

#[deprecated]
pub fn io_module() -> Result<Module, StckError> {
    Ok(io())
}
