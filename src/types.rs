use super::*;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypedFnPart {
    Typed(Vec<TypeTester>),
    Any,
}

/// # A defined generic type for the [TRC](crate::types::TypeResolutionContext)
///
/// ```stck
/// (TRC* Printable num str bool)
/// (TRC* Nil)
///
/// (fn) [ a<Printable> ] [ result<str><Nil> ] { ... }
/// ```
#[derive(Debug, Default, Clone)]
pub struct DefinedGeneric {
    allow: HashSet<TypeTester>,
    viral: bool,
}

impl DefinedGeneric {
    fn new(viral: bool, allow: HashSet<TypeTester>) -> Self {
        Self { allow, viral }
    }
}

/// # Builder of generic type
///
/// Stored in [Type resolution builder](TypeResolutionBuilder)
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct DefinedGenericBuilder {
    pub(crate) name: String,
    pub(crate) viral: bool,
    pub(crate) allow: HashSet<TypeTester>,
}

impl From<DefinedGenericBuilder> for RawKeyword {
    fn from(v: DefinedGenericBuilder) -> RawKeyword {
        RawKeyword::TRC(v)
    }
}

impl FromStr for DefinedGenericBuilder {
    type Err = StckError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (viral, cont) = match s.strip_prefix("*") {
            Some(cont) => (false, cont.trim()),
            None => (true, s),
        };
        let mut conts = cont.split(' ');
        let name = conts
            .next()
            .ok_or(StckError::TRCMissingName(s.to_string()))?;
        let allow = conts.map(TypeTester::from_str).collect::<Result<_, _>>()?;
        Ok(Self {
            name: name.to_string(),
            viral,
            allow,
        })
    }
}

/// # Storage for defined generic types
#[derive(Debug, Default, Clone)]
pub(crate) struct TypeResolutionBuilder {
    defined: HashMap<String, DefinedGeneric>,
}

impl TypeResolutionBuilder {
    pub fn new() -> Self {
        Self {
            defined: HashMap::new(),
        }
    }
    /// # Store a [defined generic](DefinedGenericBuilder)
    pub fn add_generic(
        &mut self,
        DefinedGenericBuilder { name, viral, allow }: DefinedGenericBuilder,
    ) {
        self.defined.insert(name, DefinedGeneric::new(viral, allow));
    }
}

/// # Type resolution context
///
/// This structure allow type testers to register and check `Generic types`
///
/// The structure is held by the [runtime context](crate::runtime::Context) and instanciated by the
/// `pre-processor`, which can be used to allow multiple types independently
#[derive(Clone, Debug)]
pub struct TypeResolutionContext {
    defined: HashMap<String, DefinedGeneric>,
    current: HashMap<String, TypeTester>,
}

impl From<TypeResolutionBuilder> for TypeResolutionContext {
    fn from(TypeResolutionBuilder { defined }: TypeResolutionBuilder) -> Self {
        Self {
            defined,
            current: HashMap::new(),
        }
    }
}

#[derive(PartialEq)]
pub enum TypedFnPartEq {
    Typed(Vec<TypeTesterEq>),
    Any,
}

impl TypedFnPart {
    #[must_use]
    fn as_eq(&self) -> TypedFnPartEq {
        match self {
            Self::Any => TypedFnPartEq::Any,
            Self::Typed(ts) => TypedFnPartEq::Typed(ts.iter().map(TypeTester::as_eq).collect()),
        }
    }
}

pub enum TypeTesterEq {
    Structure(Rc<UserStructDef>),
    Float,
    Generic,
    Any,
    Char,
    Str,
    Num,
    Bool,
    ArrayAny,
    MapAny,
    ResultAny,
    OptionAny,
    ClosureAny,
    Array(Box<Self>),
    Map(Box<Self>),
    Result(Box<(Self, Self)>),
    Option(Box<Self>),
    Closure(Box<(TypedFnPartEq, TypedFnPartEq)>),
}

impl PartialEq for TypeTesterEq {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Any, _) => true,
            (Self::Char, Self::Char) => true,
            (Self::Str, Self::Str) => true,
            (Self::Num, Self::Num) => true,
            (Self::Bool, Self::Bool) => true,
            (Self::ArrayAny, Self::ArrayAny) => true,
            (Self::MapAny, Self::MapAny) => true,
            (Self::ResultAny, Self::ResultAny) => true,
            (Self::OptionAny, Self::OptionAny) => true,
            (Self::ClosureAny, Self::ClosureAny) => true,
            (Self::Array(t), Self::Array(to)) => t == to,
            (Self::Map(t), Self::Map(to)) => t == to,
            (Self::Result(tt), Self::Result(tto)) => tt == tto,
            (Self::Option(t), Self::Option(to)) => t == to,
            (Self::Closure(tt), Self::Closure(tto)) => tt == tto,
            (Self::ArrayAny, Self::Array(_)) => true,
            (Self::MapAny, Self::Map(_)) => true,
            (Self::ResultAny, Self::Result(_)) => true,
            (Self::OptionAny, Self::Option(_)) => true,
            (Self::ClosureAny, Self::Closure(_)) => true,
            (_, _) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeTester {
    Generic(String),
    Float,
    Any,
    Char,
    Str,
    Num,
    Bool,
    ArrayAny,
    MapAny,
    ResultAny,
    OptionAny,
    ClosureAny,
    Array(Box<TypeTester>),
    Map(Box<TypeTester>),
    Result(Box<(TypeTester, TypeTester)>),
    Option(Box<TypeTester>),
    Closure(TypedFnPart, TypedFnPart),
    Structure(Rc<UserStructDef>),
}

impl FromStr for TypeTester {
    type Err = StckError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "?" => Self::Any,
            "char" => Self::Char,
            "string" | "str" => Self::Str,
            "num" => Self::Num,
            "float" => Self::Float,
            "bool" => Self::Bool,
            "list" | "array" => Self::ArrayAny,
            "map" => Self::MapAny,
            "result" => Self::ResultAny,
            "option" => Self::OptionAny,
            "fn" | "closure" => Self::ClosureAny,
            otherwise => {
                return None
                    .or(try_parse_generic(otherwise))
                    .or(try_parse_fn(otherwise))
                    .or(try_parse_result(otherwise))
                    .or(try_parse_simple(otherwise))
                    .ok_or(StckError::UnknownType(s.to_string()));
            }
        })
    }
}

fn try_parse_generic(cont: &str) -> Option<TypeTester> {
    let first_is_upper = cont.chars().next().is_some_and(char::is_uppercase);
    if first_is_upper {
        Some(TypeTester::Generic(cont.to_string()))
    } else {
        None
    }
}

fn try_parse_simple(cont: &str) -> Option<TypeTester> {
    let cont = cont.strip_suffix('>')?;
    let (t, cont) = cont.split_once('<')?;
    let t_internal: TypeTester = cont.parse().ok()?;
    let make_t = match t {
        "option" => TypeTester::Option,
        "map" => TypeTester::Map,
        "array" => TypeTester::Array,
        _ => return None,
    };
    Some(make_t(Box::new(t_internal)))
}

// result<ok><err>
fn try_parse_result(cont: &str) -> Option<TypeTester> {
    let cont = cont.strip_prefix("result<")?;
    let cont = cont.strip_suffix('>')?;
    let (l, r) = cont.split_once("><")?;
    Some(TypeTester::Result(Box::new((
        l.parse().ok()?,
        r.parse().ok()?,
    ))))
}

fn parse_type_list(cont: &str) -> Result<TypedFnPart, StckError> {
    Ok(match cont {
        "*" => TypedFnPart::Any,
        cont => {
            let types = cont
                .split_whitespace()
                .map(TypeTester::from_str)
                .collect::<Result<Vec<_>, _>>()?;
            TypedFnPart::Typed(types)
        }
    })
}

fn try_parse_fn(txt: &str) -> Option<TypeTester> {
    txt.strip_prefix("fn<")
        .and_then(|tx| tx.strip_suffix('>'))
        .filter(|tx| tx.contains('>'))
        .and_then(|tx| {
            let (ins, outs) = tx.split_once('>')?;
            let outs = outs.strip_prefix('<')?;
            let Ok(ins) = parse_type_list(ins) else {
                return None;
            };
            let Ok(outs) = parse_type_list(outs) else {
                return None;
            };
            Some(TypeTester::Closure(ins, outs))
        })
}

impl TypeResolutionContext {
    pub fn check_defined(t: &DefinedGeneric, v: &TypeTester) -> bool {
        t.allow.contains(v)
    }

    pub fn check_outputs(
        &mut self,
        t: &TypedOutputs,
        values: &[Value],
    ) -> Result<(), TypedOutputError> {
        if t.len() != values.len() {
            return Err(TypedOutputError::OutputCountError {
                expected: t.len(),
                got: values.len(),
            });
        }
        for (v, maybe_tt) in values.iter().zip(t.iter()) {
            if let Some(Err(t)) = maybe_tt.as_ref().map(|tt| self.check(tt, v)) {
                return Err(TypedOutputError::TypeError(t, v.clone()));
            }
        }
        Ok(())
    }

    pub fn check(&mut self, t: &TypeTester, v: &Value) -> Result<(), TypeTester> {
        self.check_internal(t, v).map_err(|()| t.clone())
    }

    fn check_internal(&mut self, t: &TypeTester, v: &Value) -> Result<(), ()> {
        match (t, v) {
            (TypeTester::Any, _) => Ok(()),
            (TypeTester::Float, Value::Float(_)) => Ok(()),
            (TypeTester::Char, Value::Char(_)) => Ok(()),
            (TypeTester::Str, Value::Str(_)) => Ok(()),
            (TypeTester::Num, Value::Num(_)) => Ok(()),
            (TypeTester::Bool, Value::Bool(_)) => Ok(()),
            (TypeTester::ArrayAny, Value::Array(_)) => Ok(()),
            (TypeTester::MapAny, Value::Map(_)) => Ok(()),
            (TypeTester::ResultAny, Value::Result(_)) => Ok(()),
            (TypeTester::OptionAny, Value::Option(_)) => Ok(()),
            (TypeTester::ClosureAny, Value::Closure(_)) => Ok(()),
            (TypeTester::Array(tt), Value::Array(n)) => {
                n.iter()
                    .map(|v| self.check_internal(tt, v))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(())
            }
            (TypeTester::Map(tt_value), Value::Map(m)) => {
                for value in m.values() {
                    self.check_internal(tt_value, value)?;
                }
                Ok(())
            }
            (TypeTester::Result(tt), Value::Result(v)) => {
                let (tt_ok, tt_err) = tt.as_ref();
                match v.as_ref() {
                    Ok(v_ok) => self.check_internal(tt_ok, v_ok),
                    Err(v_err) => self.check_internal(tt_err, v_err),
                }
            }
            (TypeTester::Structure(sd), Value::Structure(si)) if Rc::ptr_eq(&si.def, sd) => Ok(()),
            (TypeTester::Structure(..), Value::Structure(..)) => Err(()),
            (TypeTester::Option(_), Value::Option(None)) => Ok(()),
            (TypeTester::Option(tt), Value::Option(Some(v))) => self.check_internal(tt, v),
            (TypeTester::Closure(ttinput, ttoutput), Value::Closure(cl)) => {
                if let TypedFnPart::Typed(ttinput) = ttinput {
                    if cl.get_unfilled_args_count() != ttinput.len() {
                        return Err(());
                    }
                    let outs = cl
                        .get_args()
                        .get_unfilled_args()
                        .iter()
                        .map(|arg_def| arg_def.get_type())
                        .zip(ttinput);
                    for (cl_req, tt_req) in outs {
                        // part to test VTC
                        let ok = cl_req.as_ref().is_none_or(|c| tt_req.check_type(c));
                        if !ok {
                            return Err(());
                        }
                    }
                }
                if let TypedFnPart::Typed(ttoutput) = ttoutput {
                    // part to test VTC
                    let Some(outputs) = cl.get_output_types() else {
                        return Ok(());
                    };
                    if outputs.len() != ttoutput.len() {
                        return Err(());
                    }
                    for (cl_in, tt_in) in outputs.iter().zip(ttoutput) {
                        let ok = cl_in.as_ref().is_none_or(|c| tt_in.check_type(c));
                        if !ok {
                            return Err(());
                        }
                    }
                }
                Ok(())
            }
            (TypeTester::Generic(name), v) => match self.check_generic(name) {
                GenericTypeCapture::Registered(t) => self.check_internal(&t, v),
                GenericTypeCapture::Unregistered => {
                    let t = TypeTester::from(v);
                    self.register_generic_usage(name.to_string(), t);
                    Ok(())
                }
                GenericTypeCapture::Defined(t) => {
                    let vt: TypeTester = v.into();
                    if TypeResolutionContext::check_defined(&t, &vt) {
                        if t.viral {
                            self.register_generic_usage(name.to_string(), vt);
                        }
                        Ok(())
                    } else {
                        Err(())
                    }
                }
            },
            (_, _) => Err(()),
        }
    }

    pub fn check_raw_closure_arg(&mut self, f: &FnArgDef, v: &Value) -> Result<(), TypeTester> {
        match f.type_check.as_ref() {
            Some(tt) => self.check(tt, v),
            None => Ok(()),
        }
    }

    pub fn check_closure_arg(&mut self, f: &FnArgDef, v: &FnArg) -> Result<(), TypeTester> {
        self.check_raw_closure_arg(f, &v.0)
    }

    fn check_generic(&mut self, generic: &str) -> GenericTypeCapture {
        use GenericTypeCapture::{Defined, Registered, Unregistered};
        let registered = self.current.get(generic).cloned().map(Registered);
        let defined = self.defined.get(generic).cloned().map(Defined);
        registered.or(defined).unwrap_or(Unregistered)
    }

    fn register_generic_usage(&mut self, generic: String, instance: TypeTester) {
        self.current.insert(generic, instance);
    }
}

#[derive(Debug)]
enum GenericTypeCapture {
    Registered(TypeTester),
    Defined(DefinedGeneric),
    Unregistered,
}

impl TypeTester {
    #[must_use]
    pub fn as_eq(&self) -> TypeTesterEq {
        match self {
            Self::Structure(e) => TypeTesterEq::Structure(Rc::clone(e)),
            Self::Float => TypeTesterEq::Float,
            Self::Any => TypeTesterEq::Any,
            Self::Char => TypeTesterEq::Char,
            Self::Str => TypeTesterEq::Str,
            Self::Num => TypeTesterEq::Num,
            Self::Bool => TypeTesterEq::Bool,
            Self::ArrayAny => TypeTesterEq::ArrayAny,
            Self::MapAny => TypeTesterEq::MapAny,
            Self::ResultAny => TypeTesterEq::ResultAny,
            Self::OptionAny => TypeTesterEq::OptionAny,
            Self::ClosureAny => TypeTesterEq::ClosureAny,
            Self::Generic(..) => TypeTesterEq::Generic,
            Self::Array(a) => TypeTesterEq::Array(Box::new(a.as_eq())),
            Self::Map(a) => TypeTesterEq::Map(Box::new(a.as_eq())),
            Self::Result(a) => TypeTesterEq::Result(Box::new((a.0.as_eq(), a.1.as_eq()))),
            Self::Option(a) => TypeTesterEq::Option(Box::new(a.as_eq())),
            Self::Closure(a, b) => TypeTesterEq::Closure(Box::new((a.as_eq(), b.as_eq()))),
        }
    }
    #[must_use]
    pub fn check_type(&self, v: &TypeTester) -> bool {
        self.as_eq() == v.as_eq()
    }
}

#[derive(Clone, Debug)]
pub struct TypedOutputs {
    outputs: Vec<Option<TypeTester>>,
}

pub enum TypedOutputError {
    TypeError(TypeTester, Value),
    OutputCountError { expected: usize, got: usize },
}

impl TypedOutputs {
    #[must_use]
    pub fn new(v: Vec<FnArgDef>) -> Self {
        Self {
            outputs: v.into_iter().map(super::FnArgDef::take_type).collect(),
        }
    }
    fn iter(&self) -> impl Iterator<Item = &Option<TypeTester>> {
        self.outputs.iter()
    }
    fn len(&self) -> usize {
        self.outputs.len()
    }
}

impl From<Vec<FnArgDef>> for TypedOutputs {
    fn from(value: Vec<FnArgDef>) -> Self {
        Self::new(value)
    }
}

impl From<&Value> for TypeTester {
    fn from(value: &internals::Value) -> Self {
        match value {
            Value::Structure(e) => Self::Structure(Rc::clone(&e.def)),
            Value::Float(_) => Self::Float,
            Value::Char(_) => Self::Char,
            Value::Str(_) => Self::Str,
            Value::Num(_) => Self::Num,
            Value::Bool(_) => Self::Bool,
            Value::Closure(cl) => {
                let ipts: Vec<_> = cl
                    .request_args
                    .next
                    .clone()
                    .into_iter()
                    .map(|v| v.take_type().unwrap_or(TypeTester::Any))
                    .collect();
                let out = if let Some(out) = &cl.output_types {
                    TypedFnPart::Typed(
                        out.outputs
                            .clone()
                            .into_iter()
                            .map(|v| v.unwrap_or(TypeTester::Any))
                            .collect::<Vec<_>>(),
                    )
                } else {
                    TypedFnPart::Any
                };
                TypeTester::Closure(TypedFnPart::Typed(ipts), out)
            }
            Value::Map(_) => todo!("map"),
            Value::Array(_) => todo!("array"),
            Value::Result(_) => todo!("result"),
            Value::Option(a) => a
                .clone()
                .map(|tt| TypeTester::from(tt.as_ref()))
                .map_or(TypeTester::OptionAny, |tt| TypeTester::Option(Box::new(tt))),
        }
    }
}
