//! # Internal items of the crate
//!
//! Used for more precise control or error recovery

use super::*;

pub type HostContext = runtime::Context<'static>;
pub use runtime::Context as RuntimeContext;
pub use runtime::Hook as StckHook;
pub use runtime::module;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub struct Code {
    pub(crate) source: PathBuf,
    pub(crate) exprs: Vec<Expr>,
}

impl Code {
    #[must_use]
    pub fn new(source: PathBuf, exprs: Vec<Expr>) -> Self {
        Code { source, exprs }
    }
    #[must_use]
    pub fn expr_count(&self) -> usize {
        self.exprs.len()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Expr> {
        self.exprs.iter()
    }
}

impl<'p> IntoIterator for &'p Code {
    type Item = &'p Expr;
    type IntoIter = std::slice::Iter<'p, Expr>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct FnArgDef {
    pub(crate) name: String,
    pub(crate) type_check: Option<TypeTester>,
}

impl FnArgDef {
    pub(crate) fn new_untyped(name: String) -> Self {
        Self {
            name,
            type_check: None,
        }
    }
    pub(crate) fn new_typed(name: String, type_check: TypeTester) -> Self {
        Self {
            name,
            type_check: Some(type_check),
        }
    }
    #[must_use]
    pub fn new(name: String, type_check: Option<TypeTester>) -> Self {
        Self { name, type_check }
    }
    #[must_use]
    pub(crate) fn get_name(&self) -> &str {
        &self.name
    }
    pub(crate) fn get_type(&self) -> Option<&TypeTester> {
        self.type_check.as_ref()
    }
    pub(crate) fn take_type(self) -> Option<TypeTester> {
        self.type_check
    }
    fn take_name(self) -> String {
        self.name
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub enum FnArgs {
    Args(Vec<FnArgDef>),
    AllStack,
}

impl FnArgs {
    pub(crate) fn capture(
        self,
        ctx: &mut RuntimeContext,
        fn_name: &str,
        trc: &mut TypeResolutionContext,
    ) -> Result<FnArgsInsCap, error::RuntimeErrorKind> {
        match &self {
            FnArgs::Args(args) => {
                let Some(args_stack) = ctx.stack.popn(args.len()) else {
                    return Err(error::RuntimeErrorKind::UserFnMissingArgs {
                        name: fn_name.to_string(),
                        got: ctx.get_stack().to_vec(),
                        needs: self.into_needs(),
                    });
                };
                args.iter()
                    .zip(args_stack.into_iter().map(FnArg))
                    .map(|(cap, ins)| {
                        if let Err(type_check_error) = trc.check_closure_arg(cap, &ins) {
                            if TypeTesterEq::ClosureAny == type_check_error.as_eq() {
                                Err(RuntimeErrorKind::TypeType(
                                    type_check_error,
                                    TypeTester::from(&ins.0),
                                ))
                            } else {
                                Err(RuntimeErrorKind::Type(type_check_error, Box::new(ins.0)))
                            }
                        } else {
                            Ok((cap.get_name().to_string(), ins))
                        }
                    })
                    .collect::<Result<_, error::RuntimeErrorKind>>()
                    .map(FnArgsInsCap::Args)
            }
            FnArgs::AllStack => Ok(FnArgsInsCap::AllStack(ctx.stack.take())),
        }
    }
}

pub(crate) enum ClosureCurry {
    Full(FullClosure),
    Partial(Closure),
}

#[derive(Debug)]
enum ClosureFillError {
    OutOfBound,
    TypeError(TypeTester, Value),
}

#[derive(Clone, Debug)]
pub struct ClosurePartialArgs {
    pub(crate) next: Vec<FnArgDef>,
    pub(crate) filled: Vec<(ArgName, Value)>,
    parent: Option<HashMap<ArgName, FnArg>>,
}

#[derive(Clone, Debug)]
pub struct MakeClosurePartialArgs {
    pub(crate) next: Vec<FnArgDef>,
}

#[cfg(test)]
impl PartialEq for ClosurePartialArgs {
    fn eq(&self, other: &Self) -> bool {
        self.next == other.next && self.filled == other.filled
    }
}

impl MakeClosurePartialArgs {
    #[must_use]
    fn new(mut arg_list: Vec<FnArgDef>) -> Self {
        arg_list.reverse();
        MakeClosurePartialArgs { next: arg_list }
    }
    pub(crate) fn parse(arg_list: Vec<FnArgDef>, span: LineRange) -> Result<Self, StckError> {
        if arg_list.is_empty() {
            Err(StckError::CantInstanceClosureZeroArgs { span })
        } else {
            Ok(Self::new(arg_list))
        }
    }
    fn with_ctx(self, parent_args: Option<HashMap<ArgName, FnArg>>) -> ClosurePartialArgs {
        ClosurePartialArgs {
            filled: Vec::with_capacity(self.next.len()),
            next: self.next,
            parent: parent_args,
        }
    }
}

impl MakeClosure {
    pub(crate) fn into_closure(self, parent_args: Option<HashMap<ArgName, FnArg>>) -> Closure {
        Closure {
            trc: self.trc,
            code: self.code,
            request_args: self.request_args.with_ctx(parent_args),
            output_types: self.output_types,
        }
    }
}

impl ClosurePartialArgs {
    #[must_use]
    pub fn get_unfilled_args(&self) -> &[FnArgDef] {
        &self.next
    }
    #[must_use]
    pub fn take_unfilled_args(self) -> Vec<FnArgDef> {
        self.next
    }
    #[must_use]
    pub fn get_unfilled_args_count(&self) -> usize {
        self.next.len()
    }
    pub fn parse(arg_list: Vec<FnArgDef>, span: LineRange) -> Result<Self, StckError> {
        MakeClosurePartialArgs::parse(arg_list, span).map(|c| c.with_ctx(None))
    }
    #[must_use]
    fn new(mut arg_list: Vec<FnArgDef>) -> Self {
        arg_list.reverse();
        ClosurePartialArgs {
            filled: Vec::with_capacity(arg_list.len()),
            next: arg_list,
            parent: None,
        }
    }
    pub fn convert(arg_list: Vec<FnArgDef>, fn_name: &str) -> Result<Self, RuntimeErrorKind> {
        if arg_list.is_empty() {
            Err(RuntimeErrorKind::CantMakeFnIntoClosureZeroArgs {
                fn_name: fn_name.to_string(),
            })
        } else {
            Ok(Self::new(arg_list))
        }
    }
    fn fill(
        &mut self,
        value: Value,
        trc: &mut TypeResolutionContext,
    ) -> Result<(), ClosureFillError> {
        let next = self.next.pop().ok_or(ClosureFillError::OutOfBound)?;
        if let Err(tt) = trc.check_raw_closure_arg(&next, &value) {
            return Err(ClosureFillError::TypeError(tt, value));
        }
        self.filled.push((next.take_name(), value));
        Ok(())
    }
    fn is_full(&self) -> bool {
        self.next.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct MakeClosure {
    pub(crate) trc: TypeResolutionContext,
    pub(crate) code: Vec<Expr>,
    pub(crate) request_args: MakeClosurePartialArgs,
    pub(crate) output_types: Option<TypedOutputs>,
}

#[derive(Clone, Debug)]
pub struct Closure {
    pub(crate) trc: TypeResolutionContext,
    pub(crate) code: Vec<Expr>,
    pub(crate) request_args: ClosurePartialArgs,
    pub(crate) output_types: Option<TypedOutputs>,
}

pub(crate) struct FullClosure {
    pub(crate) code: Vec<Expr>,
    pub(crate) request_args: HashMap<ArgName, FnArg>,
    pub(crate) output_types: Option<TypedOutputs>,
}

impl Closure {
    pub(crate) fn get_unfilled_args_count(&self) -> usize {
        self.get_args().get_unfilled_args_count()
    }
    pub(crate) fn get_output_types(&self) -> Option<&TypedOutputs> {
        self.output_types.as_ref()
    }
    pub(crate) fn get_args(&self) -> &ClosurePartialArgs {
        &self.request_args
    }
    pub(crate) fn fill(mut self, value: Value) -> Result<ClosureCurry, RuntimeErrorKind> {
        if let Err(r) = self.request_args.fill(value, &mut self.trc) {
            return Err(match r {
                ClosureFillError::OutOfBound => RuntimeErrorKind::DEVFillFullClosure {
                    closure_args: self.request_args,
                },
                ClosureFillError::TypeError(tt, v) => RuntimeErrorKind::Type(tt, Box::new(v)),
            });
        }
        Ok(if self.request_args.is_full() {
            let args = if let Some(parent_args) = self.request_args.parent {
                let mut closure_args = parent_args.clone();
                for (k, v) in self.request_args.filled {
                    closure_args.insert(k, FnArg(v));
                }
                closure_args
            } else {
                self.request_args
                    .filled
                    .into_iter()
                    .map(|(k, v)| (k, FnArg(v)))
                    .collect()
            };
            ClosureCurry::Full(FullClosure {
                code: self.code,
                request_args: args,
                output_types: self.output_types,
            })
        } else {
            ClosureCurry::Partial(self)
        })
    }
}
impl PartialEq for Closure {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl PartialEq for MakeClosure {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl FnArgs {
    #[must_use]
    pub fn into_vec(self) -> Vec<FnArgDef> {
        match self {
            FnArgs::AllStack => vec![],
            FnArgs::Args(xs) => xs,
        }
    }

    #[must_use]
    pub fn into_needs(self) -> Vec<String> {
        match self {
            FnArgs::AllStack => vec![],
            FnArgs::Args(xs) => xs.into_iter().map(|x| x.name).collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum FnArgsInsCap {
    Args(HashMap<ArgName, FnArg>),
    AllStack(Vec<Value>),
}

#[derive(Debug, Default)]
pub struct Stack(Vec<Value>);

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FnArg(pub Value);

impl Stack {
    pub(crate) fn new_with(v: Vec<Value>) -> Self {
        Self(v)
    }
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }
    pub fn push(&mut self, v: Value) {
        self.0.push(v);
    }
    pub fn push_this(&mut self, v: impl Into<Value>) {
        self.0.push(v.into());
    }
    pub fn pushn(&mut self, mut vs: Vec<Value>) {
        self.0.append(&mut vs);
    }
    pub fn pop(&mut self) -> Option<Value> {
        self.0.pop()
    }
    pub fn peek(&mut self) -> Option<&Value> {
        self.0.get(self.len() - 1)
    }
    pub fn popn(&mut self, n: usize) -> Option<Vec<Value>> {
        if n > self.len() {
            return None;
        }
        Some(self.0.split_off(self.len() - n))
    }
    #[must_use]
    pub fn as_slice(&self) -> &[Value] {
        &self.0
    }
    #[must_use]
    pub fn into_vec(self) -> Vec<Value> {
        self.0
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn take(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.0)
    }
    pub fn pop_this<T, F>(&mut self, get_fn: F) -> Option<Result<T, Value>>
    where
        F: Fn(Value) -> Result<T, Value>,
    {
        self.pop().map(get_fn)
    }
    pub fn peek_this<T, F>(&mut self, get_fn: F) -> Option<Result<&T, &Value>>
    where
        F: Fn(&Value) -> Result<&T, &Value>,
    {
        self.peek().map(get_fn)
    }
}

pub(crate) type ArgName = String;
pub(crate) type FnName = String;

#[derive(Clone, Debug, PartialEq)]
pub enum FnScope {
    Global,   // read and writes to upper-scoped variables
    Local,    // reads upper-scoped variables
    Isolated, // fully isolated
}

#[derive(Clone, Debug)]
pub(crate) struct FnDef {
    pub(crate) source: PathBuf,
    pub(crate) scope: FnScope,
    pub(crate) code: Vec<Expr>,
    pub(crate) args: FnArgs,
    pub(crate) output_types: Option<TypedOutputs>,
}

impl FnDef {
    pub(crate) fn new(
        scope: FnScope,
        code: Vec<Expr>,
        args: FnArgs,
        output_types: Option<TypedOutputs>,
        source: PathBuf,
    ) -> Self {
        FnDef {
            source,
            scope,
            code,
            args,
            output_types,
        }
    }
    pub fn into_closure(
        self,
        name: &str,
        trc: TypeResolutionBuilder,
    ) -> Result<Closure, RuntimeErrorKind> {
        let args = match self.args {
            FnArgs::AllStack => Err(RuntimeErrorKind::CantMakeFnIntoClosureAllStack {
                fn_name: name.to_string(),
            }),
            FnArgs::Args(a) => Ok(a),
        }?;
        Ok(Closure {
            trc: trc.into(),
            code: self.code,
            request_args: ClosurePartialArgs::convert(args, name)?,
            output_types: self.output_types,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Char(char),
    Str(String),
    Num(isize),
    Bool(bool),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Result(Box<Result<Value, Value>>),
    Option(Option<Box<Value>>),
    Closure(Box<Closure>),
    Float(f64),
    Structure(UserStructInstance),
}

#[derive(Clone, Debug)]
pub struct UserStructInstance {
    pub(crate) def: Rc<UserStructDef>,
    pub(crate) fields: Vec<Value>,
}

impl UserStructInstance {
    pub(crate) fn swap_field(&mut self, index: usize, value: Value) -> Option<Value> {
        let x = self.fields.get_mut(index)?;
        Some(std::mem::replace(x, value))
    }
}

impl PartialEq for UserStructInstance {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.def, &other.def)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct UserStructDef {
    pub(crate) name: String,
    pub(crate) fields: Vec<UserStructField>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum UserStructMethod {
    New,
    Copy(usize),
    Take(usize),
    Set(usize, Rc<TypeTester>),
    Swap(usize, Rc<TypeTester>),
    Explode,
}

pub enum MethodErrorPart {
    /// Field doesn't exist
    NoSuchField,
    /// Action doesn't exist
    NoSuchAction,
    /// Fieldless action doesn't exist
    NoSuchFieldlessAction,
}

impl MethodErrorPart {
    #[must_use]
    pub fn into_runtime_error_kind(self, original_call: String) -> RuntimeErrorKind {
        (match self {
            Self::NoSuchField => RuntimeErrorKind::NoSuchField,
            Self::NoSuchAction => RuntimeErrorKind::NoSuchAction,
            Self::NoSuchFieldlessAction => RuntimeErrorKind::NoSuchFieldlessAction,
        })(original_call)
    }
}

impl UserStructDef {
    pub fn make_instance(
        def: &Rc<UserStructDef>,
        values: Vec<Value>,
    ) -> Result<UserStructInstance, RuntimeErrorKind> {
        let mut trc: TypeResolutionContext = TypeResolutionBuilder::new().into();
        for (field_def, field_value) in def.fields.iter().zip(&values) {
            trc.check(&field_def.type_check, field_value).map_err(|_| {
                RuntimeErrorKind::WrongTypeForMethod(
                    Box::new(field_value.clone()),
                    Box::new(field_def.type_check.as_ref().clone()),
                    UserStructMethod::New,
                    Rc::clone(def),
                )
            })?;
        }
        Ok(UserStructInstance {
            def: Rc::clone(def),
            fields: values,
        })
    }
    #[must_use]
    pub fn fields_count(&self) -> usize {
        self.fields.len()
    }
    #[must_use]
    pub fn is_method_of(
        &self,
        method_name: &str,
    ) -> Option<Result<UserStructMethod, MethodErrorPart>> {
        let action_field = method_name
            .strip_prefix(&self.name)
            .and_then(|s| s.strip_prefix('$'))?;
        Some(self.internal_is_method_of(action_field))
    }
    fn internal_is_method_of(
        &self,
        action_field: &str,
    ) -> Result<UserStructMethod, MethodErrorPart> {
        if let Some((action, field)) = action_field.split_once('$') {
            let (field_index, field_info) = self
                .get_field_info(field)
                .ok_or(MethodErrorPart::NoSuchField)?;
            Ok(match action {
                "copy" => UserStructMethod::Copy(field_index),
                "take" => UserStructMethod::Take(field_index),
                "set" => UserStructMethod::Set(field_index, Rc::clone(&field_info.type_check)),
                "swap" => UserStructMethod::Swap(field_index, Rc::clone(&field_info.type_check)),
                _ => return Err(MethodErrorPart::NoSuchAction),
            })
        } else {
            Ok(match action_field {
                "new" => UserStructMethod::New,
                "explode" => UserStructMethod::Explode,
                _ => return Err(MethodErrorPart::NoSuchFieldlessAction),
            })
        }
    }
    fn get_field_info(&self, field_name: &str) -> Option<(usize, &UserStructField)> {
        self.fields
            .iter()
            .enumerate()
            .find(|(_, f)| f.name == field_name)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct UserStructField {
    pub(crate) name: String,
    pub(crate) type_check: Rc<TypeTester>,
}

impl From<FnArgDef> for UserStructField {
    fn from(value: FnArgDef) -> Self {
        UserStructField {
            name: value.name,
            type_check: Rc::new(value.type_check.unwrap_or(TypeTester::Any)),
        }
    }
}

impl Value {
    pub fn get_struct(self) -> Result<UserStructInstance, Value> {
        match self {
            Value::Structure(s) => Ok(s),
            e => Err(e),
        }
    }
    pub fn get_float(self) -> Result<f64, Value> {
        match self {
            Value::Float(n) => Ok(n),
            e => Err(e),
        }
    }
    pub fn get_option(self) -> Result<Option<Box<Value>>, Value> {
        match self {
            Value::Option(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_result(self) -> Result<Result<Value, Value>, Value> {
        match self {
            Value::Result(x) => Ok(*x),
            o => Err(o),
        }
    }
    pub fn get_closure(self) -> Result<Closure, Value> {
        match self {
            Value::Closure(x) => Ok(*x),
            o => Err(o),
        }
    }
    pub fn get_str(self) -> Result<String, Value> {
        match self {
            Value::Str(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_num(self) -> Result<isize, Value> {
        match self {
            Value::Num(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_bool(self) -> Result<bool, Value> {
        match self {
            Value::Bool(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_arr(self) -> Result<Vec<Value>, Value> {
        match self {
            Value::Array(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_map(self) -> Result<HashMap<String, Value>, Value> {
        match self {
            Value::Map(x) => Ok(x),
            o => Err(o),
        }
    }

    pub fn get_ref_structure(&self) -> Result<&UserStructInstance, &Value> {
        match self {
            Value::Structure(n) => Ok(n),
            o => Err(o),
        }
    }
    pub fn get_ref_float(&self) -> Result<&f64, &Value> {
        match self {
            Value::Float(n) => Ok(n),
            o => Err(o),
        }
    }
    pub fn get_ref_option(&self) -> Result<&Option<Box<Value>>, &Value> {
        match self {
            Value::Option(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_ref_result(&self) -> Result<&Result<Value, Value>, &Value> {
        match self {
            Value::Result(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_ref_closure(&self) -> Result<&Closure, &Value> {
        match self {
            Value::Closure(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_ref_str(&self) -> Result<&String, &Value> {
        match self {
            Value::Str(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_ref_num(&self) -> Result<&isize, &Value> {
        match self {
            Value::Num(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_ref_bool(&self) -> Result<&bool, &Value> {
        match self {
            Value::Bool(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_ref_arr(&self) -> Result<&Vec<Value>, &Value> {
        match self {
            Value::Array(x) => Ok(x),
            o => Err(o),
        }
    }
    pub fn get_ref_map(&self) -> Result<&HashMap<String, Value>, &Value> {
        match self {
            Value::Map(x) => Ok(x),
            o => Err(o),
        }
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<char> for Value {
    fn from(value: char) -> Self {
        Value::Char(value)
    }
}

impl From<Option<Value>> for Value {
    fn from(value: Option<Value>) -> Self {
        Value::Option(value.map(Box::new))
    }
}

impl From<UserStructInstance> for Value {
    fn from(value: UserStructInstance) -> Self {
        Value::Structure(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Str(value)
    }
}
impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Value::Num(value)
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}
impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Value::Array(value)
    }
}
impl From<HashMap<String, Value>> for Value {
    fn from(value: HashMap<String, Value>) -> Self {
        Value::Map(value)
    }
}
impl From<Result<Value, Value>> for Value {
    fn from(value: Result<Value, Value>) -> Self {
        Value::Result(Box::new(value))
    }
}
impl From<Closure> for Value {
    fn from(value: Closure) -> Self {
        Value::Closure(Box::new(value))
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub struct CondBranch {
    pub(crate) check: Vec<Expr>,
    pub(crate) code: Vec<Expr>,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub enum KeywordKind {
    Structure {
        name: String,
        vars: Vec<FnArgDef>,
    },
    IntoClosure {
        fn_name: FnName,
    },
    Break,
    Return,
    BubbleError,
    Ifs {
        branches: Vec<CondBranch>,
        otherwhise: Option<Vec<Expr>>,
    },
    While {
        check: Vec<Expr>,
        code: Vec<Expr>,
    },
    FnDef {
        name: FnName,
        scope: FnScope,
        code: Vec<Expr>,
        args: FnArgs,
        out_args: Option<Vec<FnArgDef>>,
    },
    Switch {
        cases: Vec<SwitchCase>,
        default: Option<Vec<Expr>>,
    },
    DefinedGeneric(DefinedGenericBuilder),
    Require(String),
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub struct SwitchCase {
    pub(crate) test: Value,
    pub(crate) code: Vec<Expr>,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub struct Expr {
    pub span: LineRange,
    pub cont: ExprCont,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub enum ImmdValue {
    Str(String),
    Num(isize),
    Float(f64),
    Char(char),
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub enum ExprCont {
    MakeClosure(MakeClosure),
    Immediate(ImmdValue),
    FnCall(FnName),
    Keyword(KeywordKind),
    IncludedCode(Code),
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub enum ControlFlow {
    Continue,
    Break,
    Return,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub enum RawKeyword {
    Structure,
    FnIntoClosure { fn_name: FnName },
    BubbleError,
    Return,
    Fn(FnScope),
    Ifs,
    While,
    Include { path: PathBuf },
    TRC(DefinedGenericBuilder),
    Pragma { command: String },
    Switch,
    Break,
    Require(String),
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub struct Token {
    pub(crate) cont: TokenCont,
    pub(crate) span: LineRange,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub enum TokenCont {
    Float(f64),
    Char(char),
    Ident(String),
    Str(String),
    Number(isize),
    Keyword(RawKeyword),
    FnArgs(Vec<FnArgDef>),
    Block(Vec<Token>),
    IncludedBlock(TokenBlock),
    EndOfBlock,
}

/// # Array of tokens and their source
///
/// Usually created by [`api::get_tokens`] for files or [`api::get_tokens_str`] for raw strings.
/// The token array ends with a [`TokenCont::EndOfBlock`] token, to indicate either the end of the
/// source string or a `}` that closed the code block
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub struct TokenBlock {
    pub(crate) source: PathBuf,
    pub(crate) tokens: Vec<Token>,
}

impl<'p> IntoIterator for &'p TokenBlock {
    type Item = &'p Token;
    type IntoIter = std::slice::Iter<'p, Token>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl TokenBlock {
    #[must_use]
    pub fn token_count(&self) -> usize {
        self.tokens.len() - usize::from(self.last_is_eof())
    }
    #[must_use]
    pub fn last_is_eof(&self) -> bool {
        self.tokens
            .last()
            .is_some_and(|e| matches!(e.cont, TokenCont::EndOfBlock))
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Token> {
        self.tokens.iter()
    }
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Token> {
        self.tokens.get(index)
    }
}

type RustStckFnRaw = fn(&mut runtime::Context, &Path);
#[derive(Clone)]
pub struct RustStckFn {
    pub(crate) name: String,
    pub(crate) code: RustStckFnRaw,
}

// TODO ::new test if name is valid (for tokenizer)
impl RustStckFn {
    #[must_use]
    pub fn new(name: String, code: RustStckFnRaw) -> Self {
        RustStckFn { name, code }
    }
}

impl std::fmt::Debug for RustStckFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rust function {}", self.name)
    }
}

impl From<ImmdValue> for Value {
    fn from(value: ImmdValue) -> Self {
        match value {
            ImmdValue::Str(v) => Value::Str(v),
            ImmdValue::Num(v) => Value::Num(v),
            ImmdValue::Char(v) => Value::Char(v),
            ImmdValue::Float(v) => Value::Float(v),
        }
    }
}
