mod builtins;
pub mod module;
mod stack;
use stack::*;

use crate::*;
use std::boxed::Box;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::path::Path;
use std::rc::Rc;

use self::module::Module;

#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools)]
struct ExecAllowOptions {
    include: bool,
    closure: bool,
    func: bool,
    while_loop: bool,
}

impl Default for ExecAllowOptions {
    fn default() -> Self {
        Self {
            include: true,
            closure: true,
            func: true,
            while_loop: true,
        }
    }
}

#[derive(Debug)]
enum RuntimeError {
    RuntimeCtx(RuntimeErrorCtx),
    RuntimeRaw(RuntimeErrorKind),
}

impl From<RuntimeErrorKind> for RuntimeError {
    fn from(value: RuntimeErrorKind) -> Self {
        Self::RuntimeRaw(value)
    }
}

impl From<RuntimeErrorCtx> for RuntimeError {
    fn from(value: RuntimeErrorCtx) -> Self {
        Self::RuntimeCtx(value)
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::RuntimeCtx(c) => c.fmt(f),
            RuntimeError::RuntimeRaw(r) => r.fmt(f),
        }
    }
}

impl RuntimeError {
    fn add_ctx(self, ctx: ErrCtx) -> RuntimeErrorCtx {
        match self {
            RuntimeError::RuntimeRaw(e) => RuntimeErrorCtx::new(ctx, e),
            RuntimeError::RuntimeCtx(c) => c.append_stack(ctx),
        }
    }
}

type Rtk = crate::error::RuntimeErrorKind;
type CResult<T> = std::result::Result<T, error::RuntimeErrorCtx>;
type MixedResult<T> = std::result::Result<T, RuntimeError>;

#[derive(Clone, Debug)]
pub enum Hook {
    Raw(for<'p> fn(&mut Context<'p>, &Path)),
    WithError(for<'p> fn(&mut Context, &Path) -> Result<(), RuntimeErrorKind>),
}

impl Hook {
    fn call(&self, ctx: &mut Context, source: &Path) -> Result<(), RuntimeError> {
        match self {
            Hook::Raw(c) => {
                c(ctx, source);
                Ok(())
            }
            Hook::WithError(c) => c(ctx, source).map_err(RuntimeError::RuntimeRaw),
        }
    }
}

impl From<fn(&mut Context, &Path)> for Hook {
    fn from(value: fn(&mut Context, &Path)) -> Self {
        Hook::Raw(value)
    }
}
impl From<fn(&mut Context, &Path) -> Result<(), RuntimeErrorKind>> for Hook {
    fn from(value: fn(&mut Context, &Path) -> Result<(), RuntimeErrorKind>) -> Self {
        Hook::WithError(value)
    }
}

#[derive(Default, Debug)]
#[doc(alias = "Runtime")]
pub struct Context<'p> {
    parent: Option<&'p Context<'p>>,
    options: ExecAllowOptions,
    vars: HashMap<String, Value>,
    global_vars: HashMap<String, Value>,
    fns: HashMap<FnName, FnDef>,
    pub stack: Stack,
    args: Option<HashMap<ArgName, FnArg>>,
    rust_fns: HashMap<FnName, Hook>,
    trc: TypeResolutionBuilder,
    enabled_modules: HashSet<String>,
    user_structures: HashSet<Rc<UserStructDef>>,
}

impl Context<'static> {
    #[must_use]
    pub fn new() -> Self {
        let mut ctx = Self::default();
        ctx.register_modules(module::get_builtin_modules());
        ctx
    }
}

impl<'p> Context<'p> {
    #[must_use]
    pub fn new_raw() -> Self {
        Self::default()
    }

    pub fn set_all_options(&mut self, opt: bool) {
        self.options.include = opt;
        self.options.closure = opt;
        self.options.func = opt;
        self.options.while_loop = opt;
    }
    pub fn set_option_include(&mut self, opt: bool) -> &mut Self {
        self.options.include = opt;
        self
    }
    pub fn set_option_closure(&mut self, opt: bool) -> &mut Self {
        self.options.closure = opt;
        self
    }
    pub fn set_option_func(&mut self, opt: bool) -> &mut Self {
        self.options.func = opt;
        self
    }
    pub fn set_option_while_loop(&mut self, opt: bool) -> &mut Self {
        self.options.while_loop = opt;
        self
    }

    /// Gets all enabled modules
    ///
    /// All modules registered by [`register_modules`](Self::register_modules)
    #[must_use]
    pub fn get_enabled_modules(&self) -> &HashSet<String> {
        &self.enabled_modules
    }

    /// Register modules[^many-or-one]
    ///
    /// Make the modules' functions avaliable to the user and register their names as enabled
    ///
    /// The list of enabled modules can be gotten form [`get_enabled_modules`](Self::get_enabled_modules)
    ///
    /// [^many-or-one]: A single module implements [`IntoIterator`] for convenience
    pub fn register_modules(&mut self, module_group: impl IntoIterator<Item = Module>) {
        for Module { funcs, name } in module_group {
            self.rust_fns.extend(funcs);
            self.enabled_modules.insert(name);
        }
    }

    pub fn add_rust_hook(&mut self, RustStckFn { name, code }: RustStckFn) -> Option<Hook> {
        self.rust_fns.insert(name, Hook::Raw(code))
    }

    #[must_use]
    pub fn get_stack(&self) -> &[Value] {
        self.stack.as_slice()
    }

    #[must_use]
    pub fn get_vars(&self) -> &HashMap<String, Value> {
        &self.vars
    }

    #[must_use]
    pub fn take_stack(self) -> Stack {
        self.stack
    }

    fn frame_fn(
        ctx: &'p Context<'p>,
        vars: HashMap<String, Value>,
        args_ins: FnArgsInsCap,
    ) -> Context<'p> {
        let (stack, args) = match args_ins {
            FnArgsInsCap::AllStack(xs) => (Stack::new_with(xs), None),
            FnArgsInsCap::Args(args) => (Stack::new(), Some(args)),
        };
        Self {
            parent: Some(ctx),
            options: ctx.options,
            trc: ctx.trc.clone(),
            vars,
            stack,
            args,
            ..Context::default()
        }
    }

    fn frame_closure(
        ctx: &'p Context,
        vars: HashMap<String, Value>,
        args: HashMap<ArgName, FnArg>,
    ) -> Context<'p> {
        Self {
            parent: Some(ctx),
            options: ctx.options,
            trc: ctx.trc.clone(),
            vars,
            args: Some(args),
            stack: Stack::new(),
            ..Context::default()
        }
    }

    pub fn execute_entire_code(&mut self, Code { source, exprs }: &Code) -> CResult<ControlFlow> {
        for expr in exprs {
            match self.execute_expr(expr, source)? {
                ControlFlow::Continue => {}
                c => return Ok(c),
            }
        }
        Ok(ControlFlow::Continue)
    }

    fn execute_code(&mut self, code: &[Expr], source: &Path) -> CResult<ControlFlow> {
        for expr in code {
            match self.execute_expr(expr, source)? {
                ControlFlow::Continue => {}
                c => return Ok(c),
            }
        }
        Ok(ControlFlow::Continue)
    }

    fn execute_check(&mut self, code: &[Expr], source: &Path) -> MixedResult<bool> {
        let old_stack_size = self.stack.len();
        for expr in code {
            self.execute_expr(expr, source)?;
        }
        let new_stack_size = self.stack.len();
        let new_should_stack_size = old_stack_size + 1;
        let correct_size = new_should_stack_size == new_stack_size;
        let check = self.stack.pop();
        let check = match (check, correct_size) {
            (Some(c), true) => Ok(c),
            _ => Err(RuntimeErrorKind::WrongStackSizeDiffOnCheck {
                old_stack_size,
                new_stack_size,
                new_should_stack_size,
            }),
        }?;
        match check {
            Value::Bool(b) if correct_size => Ok(b),
            got => Err(RuntimeErrorKind::WrongTypeOnCheck { got }.into()),
        }
    }

    fn execute_expr(&mut self, expr: &Expr, source: &Path) -> CResult<ControlFlow> {
        self.execute_expr_internal(expr, source)
            .map_err(|e| e.add_ctx(ErrCtx::new(source, expr)))
    }

    fn execute_expr_internal(&mut self, expr: &Expr, source: &Path) -> MixedResult<ControlFlow> {
        match &expr.cont {
            ExprCont::FnCall(name) => self.execute_fn(name, source)?,
            ExprCont::Keyword(kw) => {
                return self.execute_kw(kw, source);
            }
            ExprCont::MakeClosure(cl) => {
                if !self.options.closure {
                    return Err(RuntimeErrorKind::DisallowedAction(
                        error::UnauthorizedAction::ExecuteClosure,
                    )
                    .into());
                }
                let make_cl = cl.clone();
                let cl = make_cl.into_closure(self.args.clone());
                self.stack.push(Value::Closure(Box::new(cl)));
            }
            ExprCont::Immediate(v) => self.stack.push(v.clone().into()),
            ExprCont::IncludedCode(Code { source, exprs }) => {
                if self.options.include {
                    self.execute_code(exprs, source)?;
                } else {
                    return Err(RuntimeErrorKind::DisallowedAction(
                        error::UnauthorizedAction::Include,
                    )
                    .into());
                }
            }
        }
        Ok(ControlFlow::Continue)
    }

    fn execute_kw(&mut self, kw: &KeywordKind, source: &Path) -> MixedResult<ControlFlow> {
        Ok(match kw {
            KeywordKind::Structure { name, vars } => {
                let stct = UserStructDef {
                    name: name.to_string(),
                    fields: vars.iter().cloned().map(UserStructField::from).collect(),
                };
                self.user_structures.insert(Rc::new(stct));
                ControlFlow::Continue
            }
            KeywordKind::Require(module_name) => {
                return if self.enabled_modules.contains(module_name) {
                    Ok(ControlFlow::Continue)
                } else {
                    Err(RuntimeErrorKind::MissingModule(module_name.to_owned()).into())
                };
            }
            KeywordKind::DefinedGeneric(trc) => {
                self.trc.add_generic(trc.clone());
                ControlFlow::Continue
            }
            KeywordKind::IntoClosure { fn_name } => {
                let fndef = self
                    .fns
                    .get(fn_name)
                    .ok_or(RuntimeErrorKind::MissingUserFunction(
                        fn_name.as_str().to_string(),
                    ))?;
                let closure = fndef
                    .clone()
                    .into_closure(fn_name.as_str(), self.trc.clone())?;
                self.stack.push_this(closure);
                ControlFlow::Continue
            }
            KeywordKind::BubbleError => {
                let e = stack_pop!((self.stack) -> result as "result" for "(!) keyword")?;
                match e {
                    Err(x) => {
                        self.stack.push_this(Err(x));
                        ControlFlow::Return
                    }
                    Ok(x) => {
                        self.stack.push_this(x);
                        ControlFlow::Continue
                    }
                }
            }
            KeywordKind::Return => ControlFlow::Return,
            KeywordKind::Break => ControlFlow::Break,
            KeywordKind::Switch { cases, default } => {
                let cmp = self
                    .stack
                    .pop()
                    .ok_or(RuntimeErrorKind::SwitchCaseWithNoValue)?;
                for case in cases {
                    if case.test == cmp {
                        return self
                            .execute_code(&case.code, source)
                            .map_err(RuntimeError::from);
                    }
                }
                match default {
                    Some(code) => self.execute_code(code, source)?,
                    None => ControlFlow::Continue,
                }
            }
            KeywordKind::Ifs {
                branches,
                otherwhise,
            } => {
                for branch in branches {
                    if self.execute_check(&branch.check, source)? {
                        return self
                            .execute_code(&branch.code, source)
                            .map_err(RuntimeError::from);
                    }
                }
                if let Some(else_code) = otherwhise {
                    return self
                        .execute_code(else_code, source)
                        .map_err(RuntimeError::from);
                }
                ControlFlow::Continue
            }
            KeywordKind::While { check, code } => {
                if !self.options.while_loop {
                    return Err(RuntimeErrorKind::DisallowedAction(
                        error::UnauthorizedAction::WhileLoop,
                    )
                    .into());
                }
                while self.execute_check(check, source)? {
                    match self.execute_code(code, source)? {
                        ControlFlow::Break => break,
                        ControlFlow::Return => return Ok(ControlFlow::Return),
                        ControlFlow::Continue => {}
                    }
                }
                ControlFlow::Continue
            }
            KeywordKind::FnDef {
                name,
                scope,
                code,
                args,
                out_args,
            } => {
                if !self.options.func {
                    return Err(RuntimeErrorKind::DisallowedAction(
                        error::UnauthorizedAction::DeclareFunction,
                    )
                    .into());
                }
                self.fns.insert(
                    name.clone(),
                    FnDef::new(
                        scope.clone(),
                        code.clone(),
                        args.clone(),
                        out_args.clone().map(TypedOutputs::from),
                        source.to_path_buf(),
                    ),
                );
                ControlFlow::Continue
            }
        })
    }

    fn execute_fn(&mut self, name: &FnName, source: &Path) -> MixedResult<()> {
        // builtin fn should handle stack pop and push
        // and are always given precedence
        match self.try_execute_builtin(name.as_str(), source) {
            Ok(Some(())) => return Ok(()),
            Ok(None) => {}
            Err(e) => return Err(e),
        }

        // find_F discovers *if* the action exists
        // execute_F executes the function
        if let Some(arg) = self.find_arg(name) {
            // There's no "execute_arg", since it just pushes and item in the stack
            self.stack.push(arg);
            Ok(())
        } else if let Some(user_fn) = self.find_user_fn(name) {
            // execute_user_fn creates a child context (with this `self` as parent), gets the
            // function's context
            let rets = self.execute_user_fn(name, user_fn)?;
            self.stack.pushn(rets);
            Ok(())
        } else if let Some(hook) = self.find_hook(name) {
            // There's also no "execute_hook" since the hook itself is the execution function
            hook.call(self, source)
        } else if let Some(method) = self.find_method(name) {
            let (method, stct) = method?;
            self.execute_method(method, &stct)
        } else {
            Err(Rtk::MissingIdent(name.clone()).into())
        }
    }

    fn execute_method(
        &mut self,
        method: UserStructMethod,
        stct: &Rc<UserStructDef>,
    ) -> Result<(), RuntimeError> {
        match method {
            UserStructMethod::New => {
                let values = self
                    .stack
                    .popn(stct.fields_count())
                    .ok_or(RuntimeErrorKind::NotEnoughArgsForNew(Rc::clone(stct)))?;
                let si = UserStructDef::make_instance(stct, values)?;
                self.stack.push_this(si);
            }
            UserStructMethod::Explode => {
                let si = stack_pop!((self.stack) -> struct as "struct" for "struct$explode")?;
                self.stack.pushn(si.fields);
            }
            meth @ UserStructMethod::Copy(field_index) => {
                let si = stack_pop!((self.stack) -> &struct as "struct" for "struct$copy")?;
                let value = si.fields.get(field_index).cloned().ok_or(
                    RuntimeErrorKind::DEVWrongIndexOnUserStructField(
                        Rc::clone(stct),
                        meth,
                        field_index,
                        Box::new(si.clone()),
                    ),
                )?;
                self.stack.push(value);
            }

            UserStructMethod::Swap(field_index, field_type) => {
                let mut trc: TypeResolutionContext = TypeResolutionBuilder::new().into();
                let val = stack_pop!((self.stack) -> * as "value" for "struct$swap")?;
                trc.check(&field_type, &val).map_err(|e| {
                    RuntimeErrorKind::WrongTypeForMethod(
                        Box::new(val.clone()),
                        Box::new(e),
                        UserStructMethod::Swap(field_index, Rc::clone(&field_type)),
                        Rc::clone(stct),
                    )
                })?;
                let mut instance = stack_pop!((self.stack) -> struct as "si" for "struct$swap")?;
                let val = instance.swap_field(field_index, val).ok_or(
                    RuntimeErrorKind::DEVWrongIndexOnUserStructField(
                        Rc::clone(stct),
                        UserStructMethod::Swap(field_index, field_type),
                        field_index,
                        Box::new(instance.clone()),
                    ),
                )?;
                self.stack.push_this(instance);
                self.stack.push(val);
            }
            UserStructMethod::Set(field_index, field_type) => {
                let mut trc: TypeResolutionContext = TypeResolutionBuilder::new().into();
                let val = stack_pop!((self.stack) -> * as "value" for "struct$set")?;
                trc.check(&field_type, &val).map_err(|e| {
                    RuntimeErrorKind::WrongTypeForMethod(
                        Box::new(val.clone()),
                        Box::new(e),
                        UserStructMethod::Set(field_index, Rc::clone(&field_type)),
                        Rc::clone(stct),
                    )
                })?;
                let mut instance = stack_pop!((self.stack) -> struct as "si" for "struct$set")?;
                instance.swap_field(field_index, val).ok_or(
                    RuntimeErrorKind::DEVWrongIndexOnUserStructField(
                        Rc::clone(stct),
                        UserStructMethod::Set(field_index, field_type),
                        field_index,
                        Box::new(instance.clone()),
                    ),
                )?;
                self.stack.push_this(instance);
            }
            UserStructMethod::Take(field_index) => {
                let instance = stack_pop!((self.stack) -> struct as "si" for "struct$set")?;
                let v = instance.fields.get(field_index).ok_or(
                    RuntimeErrorKind::DEVWrongIndexOnUserStructField(
                        Rc::clone(stct),
                        UserStructMethod::Take(field_index),
                        field_index,
                        Box::new(instance.clone()),
                    ),
                )?;
                self.stack.push_this(v.clone());
            }
        }
        Ok(())
    }

    fn try_execute_user_closure(
        &mut self,
        closure: FullClosure,
        source: &Path,
    ) -> MixedResult<Vec<Value>> {
        let mut cl_ctx = Context::frame_closure(self, self.vars.clone(), closure.request_args);
        cl_ctx.execute_code(&closure.code, source)?;
        let output = cl_ctx.take_stack().into_vec();
        // TODO: use TRC instance from closure
        let mut trc: TypeResolutionContext = self.trc.clone().into();
        if let Some(output_types) = closure.output_types {
            match trc.check_outputs(&output_types, &output) {
                Err(TypedOutputError::TypeError(expected, got)) => {
                    Err(RuntimeErrorKind::Type(expected, Box::new(got)))
                }
                Err(TypedOutputError::OutputCountError { expected, got }) => {
                    Err(RuntimeErrorKind::OutputClosureCount { expected, got })
                }
                Ok(()) => Ok(()),
            }?;
        }
        Ok(output)
    }

    fn execute_user_fn(&mut self, name: &str, user_fn: FnDef) -> MixedResult<Vec<Value>> {
        let mut trc: TypeResolutionContext = self.trc.clone().into();

        let vars = match user_fn.scope {
            FnScope::Isolated => HashMap::new(),
            _ => self.vars.clone(),
        };

        let args = user_fn.args.capture(self, name, &mut trc)?;
        let (fn_vars, fn_global_vars, fn_output) = {
            let mut fn_ctx = Context::frame_fn(self, vars, args);
            if let Err(e) = fn_ctx.execute_code(&user_fn.code, &user_fn.source) {
                return Err(e.into());
            }

            let output = fn_ctx.stack.into_vec();
            (fn_ctx.vars, fn_ctx.global_vars, output)
        };
        if let FnScope::Global = user_fn.scope {
            self.global_vars.extend(fn_vars);
        }
        self.global_vars.extend(fn_global_vars);

        if let Some(out_tt) = &user_fn.output_types {
            let err = match trc.check_outputs(out_tt, &fn_output) {
                Ok(()) => None,
                Err(TypedOutputError::TypeError(t, v)) => Some(Rtk::Type(t, Box::new(v))),
                Err(TypedOutputError::OutputCountError { expected, got }) => {
                    Some(Rtk::OutputCount {
                        fn_name: name.to_string(),
                        expected,
                        got,
                    })
                }
            };
            if let Some(err) = err {
                return Err(err.into());
            }
        }
        Ok(fn_output)
    }

    fn try_execute_builtin(&mut self, fn_name: &str, source: &Path) -> MixedResult<Option<()>> {
        match fn_name {
            // seq system
            "print" => {
                let cont = self
                    .stack
                    .pop_this(Value::get_str)
                    .expect("`print` needs [string]")
                    .expect("`print`'s [string] needs to be a string");
                print!("{cont}");
            }
            "sys$exit" => {
                let code = stack_pop!(
                    (self.stack) -> num as "exit_code" for fn_name
                )?;
                std::process::exit(code as i32);
            }
            "sys$argv" => {
                let args: Vec<_> = std::env::args().map(Value::Str).collect();
                self.stack.push_this(args);
            }
            "sh" => {
                let shell_cmd = stack_pop!(
                    (self.stack) -> str as "command" for fn_name
                )?;
                let out = builtins::sh(&shell_cmd).map(Value::Num).map_err(Value::Str);
                self.stack.push_this(out);
            }
            "write-to" => {
                let file = stack_pop!(
                    (self.stack) -> str as "file" for fn_name
                )?;
                let cont = stack_pop!(
                    (self.stack) -> str as "content" for fn_name
                )?;
                let out = builtins::write_to(&cont, &file)
                    .map(Value::Num)
                    .map_err(Value::Str);
                self.stack.push_this(out);
            }

            // seq math seq logic
            "-" => {
                let rhs = stack_pop!(
                    (self.stack) -> num as "rhs" for fn_name
                )?;
                let lhs = stack_pop!(
                    (self.stack) -> num as "lhs" for fn_name
                )?;
                self.stack.push_this(lhs - rhs);
            }
            ".-" => {
                let rhs = stack_pop!(
                    (self.stack) -> float as "rhs" for fn_name
                )?;
                let lhs = stack_pop!(
                    (self.stack) -> float as "lhs" for fn_name
                )?;
                self.stack.push_this(lhs - rhs);
            }
            "*" => {
                let rhs = stack_pop!(
                    (self.stack) -> num as "rhs" for fn_name
                )?;
                let lhs = stack_pop!(
                    (self.stack) -> num as "lhs" for fn_name
                )?;
                self.stack.push_this(lhs * rhs);
            }
            ".*" => {
                let rhs = stack_pop!(
                    (self.stack) -> float as "rhs" for fn_name
                )?;
                let lhs = stack_pop!(
                    (self.stack) -> float as "lhs" for fn_name
                )?;
                self.stack.push_this(lhs * rhs);
            }
            "≃" => {
                use Value::*;
                let rhs = stack_pop!((self.stack) -> * as "rhs" for fn_name)?;
                let lhs = stack_pop!((self.stack) -> * as "lhs" for fn_name)?;
                let eq = match (lhs, rhs) {
                    // if the user has a threshold they can check it them selves
                    #[allow(clippy::float_cmp)]
                    (Float(l), Float(r)) => Ok(l == r),
                    (Char(l), Char(r)) => Ok(l == r),
                    (Num(l), Num(r)) => Ok(l == r),
                    (Str(l), Str(r)) => Ok(l == r),
                    (Bool(l), Bool(r)) => Ok(l == r),
                    (r @ Array(_), l) | (l, r @ Array(_)) => Err(Rtk::Compare { this: l, that: r }),
                    (m @ Map(_), l) | (l, m @ Map(_)) => Err(Rtk::Compare { this: l, that: m }),
                    (_, _) => Ok(false),
                }?;
                self.stack.push_this(eq);
            }
            "=" => {
                use Value::*;
                let rhs = stack_pop!((self.stack) -> * as "rhs" for fn_name)?;
                let lhs = stack_pop!((self.stack) -> * as "lhs" for fn_name)?;
                let eq = match (lhs, rhs) {
                    // if the user has a threshold they can check it them selves
                    #[allow(clippy::float_cmp)]
                    (Float(l), Float(r)) => l == r,
                    (Char(l), Char(r)) => l == r,
                    (Num(l), Num(r)) => l == r,
                    (Str(l), Str(r)) => l == r,
                    (Bool(l), Bool(r)) => l == r,
                    (l, r) => {
                        return Err(Rtk::Compare { this: l, that: r }.into());
                    }
                };
                self.stack.push_this(eq);
            }
            ">" => {
                use Value::*;
                let rhs = stack_pop!((self.stack) -> * as "rhs" for fn_name)?;
                let lhs = stack_pop!((self.stack) -> * as "lhs" for fn_name)?;
                let eq = match (lhs, rhs) {
                    (Float(l), Float(r)) => l > r,
                    (Num(l), Num(r)) => l > r,
                    (Str(l), Str(r)) => l > r,
                    (Bool(l), Bool(r)) => l && !r,
                    (l, r) => {
                        return Err(Rtk::Compare { this: l, that: r }.into());
                    }
                };
                self.stack.push_this(eq);
            }
            "%" => {
                let rhs = stack_pop!(
                    (self.stack) -> num as "rhs" for fn_name
                )?;
                let lhs = stack_pop!(
                    (self.stack) -> num as "lhs" for fn_name
                )?;
                self.stack.push_this(lhs % rhs);
            }
            "%." => {
                let rhs = stack_pop!(
                    (self.stack) -> float as "rhs" for fn_name
                )?;
                let lhs = stack_pop!(
                    (self.stack) -> float as "lhs" for fn_name
                )?;
                self.stack.push_this(lhs % rhs);
            }
            "@" => {
                let v = stack_pop!((self.stack) -> * as "value" for fn_name)?;
                let cl = stack_pop!((self.stack) -> closure as "closure" for fn_name)?;
                match cl.fill(v)? {
                    ClosureCurry::Partial(cl) => {
                        self.stack.push_this(cl);
                    }
                    ClosureCurry::Full(cl) => {
                        let result = self.try_execute_user_closure(cl, source)?;
                        self.stack.pushn(result);
                    }
                }
            }

            // seq variables
            "stack$len" => {
                self.stack.push_this(self.stack.len() as isize);
            }
            "set" => {
                let name = stack_pop!(
                    (self.stack) -> str as "name" for fn_name
                )?;
                let value = stack_pop!(
                    (self.stack) -> * as "value" for fn_name
                )?;
                self.vars.insert(name, value);
            }
            "get" => {
                let name = stack_pop!(
                    (self.stack) -> str as "name" for fn_name
                )?;
                match self.find_var(&name) {
                    None => {
                        return Err(Rtk::NoSuchVariable(name).into());
                    }
                    Some(v) => {
                        self.stack.push(v.clone());
                    }
                }
            }
            "try-get" => {
                let name = stack_pop!(
                    (self.stack) -> str as "name" for fn_name
                )?;
                self.stack.push_this(self.find_var(&name).cloned());
            }

            // seq error handeling
            "!" => {
                let may = stack_pop!((self.stack) -> * as "Monad" for fn_name)?;
                match may {
                    Value::Result(r) => match *r {
                        Err(error) => {
                            return Err(Rtk::UnwrapResultBuiltinFailed { error }.into());
                        }
                        Ok(o) => self.stack.push_this(o),
                    },
                    Value::Option(o) => match o {
                        None => return Err(Rtk::UnwrapOptionBuiltinFailed.into()),
                        Some(s) => self.stack.push_this(*s),
                    },
                    e => {
                        return Err(Rtk::WrongTypeForBuiltin {
                            for_fn: fn_name.to_string(),
                            args: "[Monad]",
                            this_arg: "Monad",
                            got: Box::new(e),
                            expected: "Result or Option",
                        }
                        .into());
                    }
                }
            }
            "ok" => {
                let v = self.stack.pop().expect("`ok` needs [value]");
                self.stack.push_this(Ok(v));
            }
            "err" => {
                let v = self.stack.pop().expect("`err` needs [value]");
                self.stack.push_this(Err(v));
            }
            "none" => {
                self.stack.push_this(None);
            }
            "some" => {
                let v = stack_pop!((self.stack) -> * as "v" for fn_name)?;
                self.stack.push_this(Some(v));
            }
            "&result$is-ok" => {
                let is_ok = stack_pop!((self.stack) -> &result as "result" for fn_name)?.is_ok();
                self.stack.push_this(is_ok);
            }
            "&option$is-some" => {
                let is_some =
                    stack_pop!((self.stack) -> &option as "option" for fn_name)?.is_some();
                self.stack.push_this(is_some);
            }

            // seq string
            "%%" => {
                let fmt = self
                    .stack
                    .pop_this(Value::get_str)
                    .expect("`%` needs at least [string]")
                    .expect("`%` [string] must be a string");
                let out = builtins::fmt(&fmt, &mut self.stack);
                self.stack.push_this(out?);
            }
            "&str$has-prefix" => {
                let prefix = stack_pop!(
                    (self.stack) -> str as "prefix" for fn_name
                )?;
                let s = stack_pop!((self.stack) -> &str as "string" for fn_name)?;
                let has = s.starts_with(&prefix);
                self.stack.push_this(has);
            }
            "str$trim" => {
                let v = stack_pop!(
                    (self.stack) -> str as "string" for fn_name
                )?;
                self.stack.push_this(v.trim().to_owned());
            }
            "str$remove-prefix" => {
                let prefix = self
                    .stack
                    .pop_this(Value::get_str)
                    .expect("`str$remove-prefix` needs [string prefix]")
                    .expect("`str$remove-prefix` needs [prefix] to be a string");
                let st = self
                    .stack
                    .pop_this(Value::get_str)
                    .expect("`str$remove-prefix` needs [string prefix]")
                    .expect("`str$remove-prefix` needs [string] to be a string");
                let out = st.strip_prefix(&prefix).map(String::from).unwrap_or(st);
                self.stack.push_this(out);
            }
            "str$into-arr" => {
                let string = stack_pop!((self.stack) -> str as "string" for fn_name)?;
                let chars: Vec<_> = string.chars().map(Value::from).collect();
                self.stack.push_this(chars);
            }

            // seq array
            "&arr$len" => {
                let arr_len = stack_pop!((self.stack) -> &arr as "array" for fn_name)?.len();
                self.stack.push_this(arr_len as isize);
            }
            "arr$reverse" => {
                let mut arr = stack_pop!((self.stack) -> arr as "arr" for fn_name)?;
                arr.reverse();
                self.stack.push_this(arr);
            }
            "arr$unpack" => {
                let arr = self
                    .stack
                    .pop_this(Value::get_arr)
                    .expect("arr$unpack` needs [arr]")
                    .expect("arr$unpack` [arr] must be an array");
                let len = arr.len();
                self.stack.pushn(arr);
                self.stack.push_this(len as isize);
            }
            "arr$pack-n" => {
                let count = stack_pop!((self.stack) -> num as "count" for fn_name)?;
                let xs = self.stack.popn(count as usize).ok_or_else(|| {
                    let got = self.stack.len() as isize;
                    let missing = count - got;
                    Rtk::MissingValuesForBuiltin {
                        for_fn: fn_name.to_string(),
                        args: "[n, [n]]",
                        missing,
                    }
                })?;
                self.stack.push_this(xs);
            }
            "arr$new" => {
                self.stack.push_this(Vec::new());
            }
            "arr$append" => {
                let mut arr = self
                    .stack
                    .pop_this(Value::get_arr)
                    .expect("arr$append` needs [value array]")
                    .expect("arr$append` [array] must be an array");
                let any = self.stack.pop().expect("`arr$append` needs [value array]");
                arr.push(any);
                self.stack.push_this(arr);
            }
            "arr$join" => {
                let joiner = stack_pop!((self.stack) -> str as "joiner" for fn_name)?;
                let arr = stack_pop!((self.stack) -> arr as "array" for fn_name)?;
                let arr = arr
                    .into_iter()
                    .map(super::Value::get_str)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|got| Rtk::WrongTypeForBuiltin {
                        for_fn: fn_name.to_string(),
                        args: "[array joiner]",
                        this_arg: "array",
                        expected: "String array",
                        got: Box::new(got),
                    })?;
                self.stack.push_this(arr.join(&joiner));
            }
            "arr$pop" => {
                let mut arr = stack_pop!((self.stack) -> arr as "array" for fn_name)?;
                let v = arr.pop();
                self.stack.push_this(arr);
                self.stack.push_this(v);
            }

            // seq type
            "type$is-str" => {
                let is_type = stack_pop!((self.stack) -> str as "value" for fn_name).is_ok();
                self.stack.push_this(is_type);
            }
            "type$is-num" => {
                let is_type = stack_pop!((self.stack) -> num as "value" for fn_name).is_ok();
                self.stack.push_this(is_type);
            }
            "type$is-bool" => {
                let is_type = stack_pop!((self.stack) -> bool as "value" for fn_name).is_ok();
                self.stack.push_this(is_type);
            }
            "type$is-array" => {
                let is_type = stack_pop!((self.stack) -> arr as "value" for fn_name).is_ok();
                self.stack.push_this(is_type);
            }
            "type$is-map" => {
                let is_type = stack_pop!((self.stack) -> map as "value" for fn_name).is_ok();
                self.stack.push_this(is_type);
            }
            "type$is-result" => {
                let is_type = stack_pop!((self.stack) -> result as "value" for fn_name)?.is_ok();
                self.stack.push_this(is_type);
            }
            "type$is-option" => {
                let is_type = stack_pop!((self.stack) -> option as "value" for fn_name).is_ok();
                self.stack.push_this(is_type);
            }

            _ => {
                return Ok(None);
            }
        }
        Ok(Some(()))
    }
}

impl Context<'_> {
    fn find_arg(&self, name: &ArgName) -> Option<Value> {
        self.interal_find_arg(name)
            .or(self.parent.as_ref().and_then(|p| p.find_arg(name)))
    }
    fn find_user_fn(&self, name: &FnName) -> Option<FnDef> {
        self.interal_find_user_fn(name)
            .or(self.parent.as_ref().and_then(|p| p.find_user_fn(name)))
    }
    fn find_hook(&self, name: &FnName) -> Option<Hook> {
        self.interal_find_hook(name)
            .or(self.parent.as_ref().and_then(|p| p.find_hook(name)))
    }
    fn find_method(
        &self,
        name: &FnName,
    ) -> Option<Result<(UserStructMethod, Rc<UserStructDef>), RuntimeErrorKind>> {
        self.interal_find_method(name)
            .map(|r| r.map_err(|e| e.into_runtime_error_kind(name.to_string())))
            .or(self.parent.as_ref().and_then(|p| p.find_method(name)))
    }
    fn find_var(&self, name: &str) -> Option<&Value> {
        self.internal_find_var(name)
            .or(self.parent.as_ref().and_then(|p| p.find_var(name)))
    }

    fn internal_find_var(&self, name: &str) -> Option<&Value> {
        self.vars.get(name).or(self.global_vars.get(name))
    }
    fn interal_find_arg(&self, name: &ArgName) -> Option<Value> {
        if let Some(args) = &self.args {
            args.get(name).map(|arg| arg.0.clone())
        } else {
            None
        }
    }
    fn interal_find_user_fn(&self, name: &FnName) -> Option<FnDef> {
        self.fns.get(name).cloned()
    }
    fn interal_find_hook(&self, name: &FnName) -> Option<Hook> {
        self.rust_fns.get(name).cloned()
    }
    fn interal_find_method(
        &self,
        name: &FnName,
    ) -> Option<Result<(UserStructMethod, Rc<UserStructDef>), MethodErrorPart>> {
        let (method, stct) = self
            .user_structures
            .iter()
            .find_map(|stct| Some((stct.is_method_of(name)?, Rc::clone(stct))))?;
        match method {
            Ok(m) => Some(Ok((m, stct))),
            Err(e) => Some(Err(e)),
        }
    }
}
