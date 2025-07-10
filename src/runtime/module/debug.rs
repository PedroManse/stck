use super::*;
use crate::RuntimeContext;
use std::path::Path;

fn debug_stack(ctx: &mut RuntimeContext, _: &Path) {
    println!("{:?}", ctx.stack);
}
fn debug_stack_pretty(ctx: &mut RuntimeContext, _: &Path) {
    println!("{}", ctx.stack);
}
fn debug_vars(ctx: &mut RuntimeContext, _: &Path) {
    println!("{:?}", ctx.vars);
}
fn debug_args(ctx: &mut RuntimeContext, _: &Path) {
    println!("{:?}", ctx.args);
}
fn debug_fns(ctx: &mut RuntimeContext, _: &Path) {
    println!("{:?}", ctx.fns);
}
fn debug_modules(ctx: &mut RuntimeContext, _: &Path) {
    println!("{:?}", ctx.enabled_modules);
}
fn debug_generics(ctx: &mut RuntimeContext, _: &Path) {
    println!("{:?}", ctx.trc);
}

pub fn make() -> Module {
    let mut debug_mod = Module::new_protected("debug");
    debug_mod.add_fn("debug$stack", Hook::Raw(debug_stack));
    debug_mod.add_fn("Debug$stack", Hook::Raw(debug_stack_pretty));
    debug_mod.add_fn("debug$vars", Hook::Raw(debug_vars));
    debug_mod.add_fn("debug$args", Hook::Raw(debug_args));
    debug_mod.add_fn("debug$fns", Hook::Raw(debug_fns));
    debug_mod.add_fn("debug$modules", Hook::Raw(debug_modules));
    debug_mod.add_fn("debug$generics", Hook::Raw(debug_generics));
    debug_mod
}
