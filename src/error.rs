//! # Error handeling module

use super::*;
use crate::cache::FileCacher;
use colored::Colorize;
use std::fmt::Display;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// # The error of highest order of the stck lib
///
/// Keeps either a [`Runtime error`](RuntimeErrorCtx) of [another type of error](StckError)
#[derive(Debug)]
pub enum Error {
    Anoter(StckError),
    RuntimeError(RuntimeErrorCtx),
}

impl From<StckError> for Error {
    fn from(value: StckError) -> Self {
        Self::Anoter(value)
    }
}

impl From<RuntimeErrorCtx> for Error {
    fn from(value: RuntimeErrorCtx) -> Self {
        Self::RuntimeError(value)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeError(r) => r.fmt(f),
            Self::Anoter(a) => a.fmt(f),
        }
    }
}

/// # The context of a runtime error
///
/// Error Context, informing the source file's path, the expression
/// that caused the error and it's [`span`](LineRange)
///
/// Useful to [get the source code of the error](ErrorSource)
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ErrCtx {
    pub(crate) source: PathBuf,
    pub(crate) expr: Box<Expr>,
    pub(crate) lines: LineRange,
}

impl ErrCtx {
    #[must_use]
    pub fn new(source: &Path, expr: &Expr) -> Self {
        Self {
            source: source.to_path_buf(),
            expr: Box::new(expr.clone()),
            lines: expr.span.clone(),
        }
    }
    pub fn get_lines(&self, eh: &mut impl FileCacher) -> Result<String, StckError> {
        eh.get_span(&self.source, &self.lines)
            .map_err(StckError::from)
    }
}

/// # A viewable slice of a file
///
/// made in bulk from the a [stack trace](ErrorSpans) with [try into sources](ErrorSpans::try_into_sources)
///
/// Implemends Display by default to show:
/// ```md
/// ===[ {file}:{slice_start}:+{slice_size} ]===
/// {file content}
/// --------------------------------------------
///
/// ```
pub struct ErrorSource {
    pub(crate) range: LineRange,
    pub(crate) source: PathBuf,
    pub(crate) lines: String,
}

/// # The entire call stack of an [error](RuntimeErrorCtx)
///
/// Used to create viewable [sources](ErrorSource) of the error with [try into sources](ErrorSpans::try_into_sources)
pub struct ErrorSpans {
    head: ErrCtx,
    stack: Vec<ErrCtx>,
}

impl ErrorSpans {
    /// # Get code from an [`error context`](ErrCtx)
    ///
    /// Read the source files with [File cacher](FileCacher) and make [Error source](ErrorSource)
    /// for each [Error context](ErrCtx) entry
    pub fn try_into_sources(
        self,
        error_helper: &mut impl FileCacher,
    ) -> Result<Vec<ErrorSource>, StckError> {
        std::iter::once(self.head)
            .chain(self.stack)
            .map(|a| {
                Ok(ErrorSource {
                    lines: a.get_lines(error_helper)?,
                    range: a.lines,
                    source: a.source,
                })
            })
            .collect()
    }
}

/// # This should not be used
///
/// Implicitly convert a runtime error into [`ErrorSpans`].
///
/// The explicit [`into_error_spans`](RuntimeErrorCtx::into_error_spans) should be used
impl From<RuntimeErrorCtx> for ErrorSpans {
    fn from(value: RuntimeErrorCtx) -> Self {
        Self {
            head: value.ctx,
            stack: value.stack,
        }
    }
}

/// # An error with context
///
/// A runtime [`error`](RuntimeErrorKind) with the faulty expression's [context](ErrCtx)
/// and the [stack trace](RuntimeErrorCtx::get_call_stack)
///
/// This can be made into a stack trace of file slices containig the original expressions with
/// [`RuntimeErrorCtx::into_error_spans`].
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct RuntimeErrorCtx {
    pub(crate) ctx: ErrCtx,
    pub(crate) kind: Box<RuntimeErrorKind>,
    pub(crate) stack: Vec<ErrCtx>,
}

impl RuntimeErrorCtx {
    pub(crate) fn new(ctx: ErrCtx, kind: RuntimeErrorKind) -> Self {
        Self {
            ctx,
            kind: Box::new(kind),
            stack: vec![],
        }
    }
    #[must_use]
    pub(crate) fn append_stack(mut self, ctx: ErrCtx) -> Self {
        self.stack.push(ctx);
        self
    }
    #[must_use]
    pub fn get_call_stack(&self) -> &[ErrCtx] {
        &self.stack
    }
    #[must_use]
    pub fn into_error_spans(self) -> ErrorSpans {
        ErrorSpans {
            head: self.ctx,
            stack: self.stack,
        }
    }
}

impl std::error::Error for RuntimeErrorCtx {}

/// # A range of lines
///
/// The [`LineRange`] can be used with [`FileCacher`]'s [`get_span`](FileCacher::get_span) to select specific lines to read from
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LineRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl From<Range<usize>> for LineRange {
    fn from(value: Range<usize>) -> Self {
        LineRange {
            start: value.start,
            end: value.end,
        }
    }
}

impl LineRange {
    pub(crate) fn delta(&self) -> usize {
        self.end - self.start
    }
    pub(crate) fn from_points(last: usize, current: usize) -> Self {
        Self {
            start: last,
            end: current,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum UnauthorizedAction {
    ExecuteClosure,
    DeclareFunction,
    WhileLoop,
    Include,
}

/// # An error from `stck`
///
/// A failure that doesn't occour during the runtime of the stck script, but at some other time
#[derive(thiserror::Error, Debug)]
#[cfg_attr(not(feature = "exhaustive-errors"), non_exhaustive)]
pub enum StckError {
    #[error("Can't read file {0:?}")]
    CantReadFile(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
    #[error(transparent)]
    ParseFloat(#[from] std::num::ParseFloatError),
    #[error("No pragma section to (end if), on span {0}")]
    NoSectionToClose(LineRange),
    #[error("Can't start pragma (else) section on {1:?} (span {0:?})")]
    CantElseCurrentSection(LineRange, Option<crate::preproc::ProcCommand>),
    #[error("Invalid pragma command: {0}")]
    InvalidPragma(String),
    #[error("Unexpected end of file while building token {0:?}")]
    UnexpectedEOF(token::State),
    #[error("Tokenizer: No impl for {0:?} with {1:?}")]
    CantTokenizerChar(token::State, char),
    #[error(
        "Parser in file {path}: State ({0:?}): {state} doesn't accept token: {1:?}",
        path=.2.display().to_string().green(),
        state=.0.to_string().yellow()
    )]
    CantParseToken(parse::State, Box<TokenCont>, PathBuf),
    #[error("Unknown keyword: {0}")]
    UnknownKeyword(String),
    #[error("Can't make closure with zero arguments, it's code spans these bytes: {span}")]
    CantInstanceClosureZeroArgs { span: LineRange },
    #[error("Parser in file {path}: Can only user param list or '*' as function arguments, not {0}", path=.1.display())]
    WrongParamList(String, PathBuf),
    #[error("Type `{0}` doesn't exist")]
    UnknownType(String),
    #[error("Can't parse TRC `{0}`, missing name")]
    TRCMissingName(String),
    #[error("Hosts can't make modules with the # prefix (sign of builtin module)")]
    UserModuleWithBang(String),
}

/// # A runtime error
///
/// An error that can be caught during a failure while trying to execute a stck script
///
/// This is usually wrapped by a [context](RuntimeErrorCtx) to display more information
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(not(feature = "exhaustive-errors"), non_exhaustive)]
pub enum RuntimeErrorKind {
    UserFnMissingArgs {
        name: String,
        got: Vec<Value>,
        needs: Vec<String>,
    },
    UnwrapResultBuiltinFailed {
        error: Value,
    },
    UnwrapOptionBuiltinFailed,
    Compare {
        this: Value,
        that: Value,
    },
    SwitchCaseWithNoValue,
    UnknownStringFormat(String, char),
    MissingValue(String, char),
    WrongValueType(String, Value, char),
    Type(TypeTester, Box<Value>),
    TypeType(TypeTester, TypeTester),
    OutputCount {
        fn_name: String,
        expected: usize,
        got: usize,
    },
    OutputClosureCount {
        expected: usize,
        got: usize,
    },
    MissingUserFunction(String),
    WrongStackSizeDiffOnCheck {
        old_stack_size: usize,
        new_stack_size: usize,
        new_should_stack_size: usize,
    },
    WrongTypeOnCheck {
        got: Value,
    },
    MissingValueForBuiltin {
        for_fn: String,
        args: String,
        this_arg: &'static str,
    },
    MissingValuesForBuiltin {
        for_fn: String,
        args: &'static str,
        missing: isize,
    },
    WrongTypeForBuiltin {
        for_fn: String,
        args: &'static str,
        this_arg: &'static str,
        got: Box<Value>,
        expected: &'static str,
    },
    NoSuchVariable(String),
    CantMakeFnIntoClosureZeroArgs {
        fn_name: String,
    },
    CantMakeFnIntoClosureAllStack {
        fn_name: String,
    },
    CantInstanceClosureZeroArgs {
        span: Range<usize>,
    },
    DEVFillFullClosure {
        closure_args: ClosurePartialArgs,
    },
    DEVWrongIndexOnUserStructField(
        Rc<UserStructDef>,
        UserStructMethod,
        usize,
        Box<UserStructInstance>,
    ),
    MissingIdent(String),
    MissingModule(String),
    NoSuchFieldlessAction(String),
    NoSuchField(String),
    NoSuchAction(String),
    NotEnoughArgsForNew(Rc<UserStructDef>),
    WrongTypeForMethod(
        Box<Value>,
        Box<TypeTester>,
        UserStructMethod,
        Rc<UserStructDef>,
    ),
    DisallowedAction(UnauthorizedAction),
}

impl Display for RuntimeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::DEVWrongIndexOnUserStructField(user_struct, method, index, instance) => format!(
                "DEV ERROR, this error should never appear to you:\nThe struct {user_struct}, with method {method:?}, tried using field with index {index}; {instance}"
            ),
            Self::DEVFillFullClosure { closure_args } => {
                format!(
                    "Closure's arguments ({closure_args:?}) are filled, but still tried to add more",
                )
            }
            Self::CantInstanceClosureZeroArgs { span } => format!(
                "Can't make closure with zero arguments, it's code spans these bytes: {span:?}"
            ),
            Self::CantMakeFnIntoClosureAllStack { fn_name } => format!(
                "Can't make function ({fn_name}) that takes entire stack into closure, since it would never be executed"
            ),
            Self::CantMakeFnIntoClosureZeroArgs { fn_name } => format!(
                "Can't make function ({fn_name}) that takes no arguments into closure, since that would never be executed"
            ),
            Self::WrongValueType(s, v, c) => {
                format!("`%%` ({s}) The provided value, {v}, can't be formatted with `{c}`")
            }
            Self::MissingValue(s, _) => {
                format!("`%%` ({s}) Can't capture any value, the stack is empty")
            }
            Self::UnknownStringFormat(s, c) => {
                format!(
                    "`%%` ({s}) doesn't recognise the format directive `{c}`, only '%', 'd', 's', 'v', 'V' and 'b' are avaliable"
                )
            }
            Self::UserFnMissingArgs { name, got, needs } => {
                format!("Not enough arguments to execute {name}, got {got:?} needs {needs:?}")
            }
            Self::UnwrapResultBuiltinFailed { error } => format!(
                "Found {} while executing `!` on a Result: {error}",
                "Error".bright_yellow()
            ),
            Self::UnwrapOptionBuiltinFailed => {
                format!("Found missing value while exeuting `!` on an Option")
            }
            Self::Compare { this, that } => format!("Can't compare {this} with {that}"),
            Self::SwitchCaseWithNoValue => format!("Switch case with no value"),
            Self::Type(t, v) => format!(
                "Expected type: {t} got value {v}: {ty}",
                ty = TypeTester::from(v.as_ref())
            ),
            Self::TypeType(t1, t2) => format!("Expected type: {t1} got {t2}"),
            Self::OutputCount {
                fn_name,
                expected,
                got,
            } => format!("Output of function `{fn_name}` error, Expected {expected:?} got {got:?}"),
            Self::OutputClosureCount { expected, got } => {
                format!("Output of closure error, Expected {expected:?} got {got:?}")
            }
            Self::MissingUserFunction(name) => format!("No such user-defined function `{name}`"),
            Self::WrongStackSizeDiffOnCheck {
                old_stack_size,
                new_stack_size,
                new_should_stack_size,
            } => format!(
                "WrongStackSizeDiffOnCheck {old_stack_size} -> {new_stack_size}, (should be {new_should_stack_size})"
            ),
            Self::WrongTypeOnCheck { got } => {
                format!("check blocks must recieve one boolean, recieved {got}")
            }
            Self::MissingValueForBuiltin {
                for_fn,
                args,
                this_arg,
            } => {
                format!("Function {for_fn} accepts [{args}]. But {this_arg} is missing")
            }
            Self::MissingValuesForBuiltin {
                for_fn,
                args,
                missing,
            } => {
                format!("Function {for_fn} accepts {args}. But {missing} args are missing")
            }
            Self::WrongTypeForBuiltin {
                for_fn,
                args,
                this_arg,
                got,
                expected,
            } => format!(
                "Function {for_fn} accepts {args}. But [{this_arg}] must be a {expected} and got {got}"
            ),
            Self::NoSuchVariable(name) => format!("The variable {name} is not defined"),
            Self::MissingIdent(name) => {
                format!("No such function or function argument called `{name}`")
            }
            Self::MissingModule(name) => format!("Module `{name}` is required but was not loaded"),
            Self::NoSuchFieldlessAction(name) => {
                format!("Method {name}'s fieldless action doesn't exist")
            }
            Self::NoSuchField(name) => format!("Method {name}'s field doesn't exist"),
            Self::NoSuchAction(name) => format!("Method {name}'s action doesn't exist"),
            Self::NotEnoughArgsForNew(user_struct) => {
                format!("Not enough arguments to make {user_struct}")
            }
            Self::WrongTypeForMethod(val, typ, meth, user_struct) => {
                format!("Wrong value, {val}, for type {typ}, while doing {meth:?} on {user_struct}")
            }
            Self::DisallowedAction(act) => format!("Tried to execute dissalowed action: {act}"),
        };
        f.write_str(&s)
    }
}
