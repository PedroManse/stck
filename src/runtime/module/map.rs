use crate::runtime::{sget, stack_pop};
use crate::{
    RuntimeContext, RuntimeErrorKind, Value,
    runtime::{Hook, Module},
};
use std::collections::HashMap;
use std::path::Path;

fn map_new(ctx: &mut RuntimeContext, _: &Path) {
    ctx.stack.push_this(HashMap::new());
}

fn map_insert(ctx: &mut RuntimeContext, _: &Path) -> Result<(), RuntimeErrorKind> {
    let value = stack_pop!(
        (ctx.stack) -> * as "value" for "map-insert-kv"
    )?;
    let key = stack_pop!(
        (ctx.stack) -> str as "key" for "map-insert-kv"
    )?;
    let mut map = stack_pop!(
        (ctx.stack) -> map as "map" for "map-insert-kv"
    )?;
    map.insert(key, value);
    ctx.stack.push_this(map);
    Ok(())
}

fn map_get(ctx: &mut RuntimeContext, _: &Path) -> Result<(), RuntimeErrorKind> {
    let key = stack_pop!(
        (ctx.stack) -> str as "key" for "map$get"
    )?;
    let got = stack_pop!((ctx.stack) -> &map as "map" for "map$get")?
        .get(&key)
        .cloned();
    ctx.stack.push_this(got);
    Ok(())
}

pub fn make() -> Module {
    let mut map_mod = Module::empty("map".to_string());
    map_mod.add_fn("map$new", Hook::Raw(map_new));
    map_mod.add_fn("map$insert", Hook::WithError(map_insert));
    map_mod.add_fn("map$insert-kv", Hook::WithError(map_insert));
    map_mod.add_fn("map$get", Hook::WithError(map_get));
    map_mod
}
