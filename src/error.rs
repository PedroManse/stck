//! # Error handeling module

use super::*;
use crate::cache::FileCacher;
use colored::Colorize;
use std::collections::hash_map::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Anoter(#[from] StckError),
    #[error(transparent)]
    RuntimeError(#[from] RuntimeErrorCtx),
}

/// # The context of a runtime error
///
/// Error Context, informing the source file's path, the expression
/// that caused the error and it's [`span`](LineRange)
///
/// Useful to [get the source code of the error](ErrorSource)
#[derive(Debug)]
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

/// # A single viewable source file
///
/// made in bulk from the [stack trace](ErrorSpans) with [try into sources](ErrorSpans::try_into_sources)
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
    /// # Get code from [error](ErrCtx)
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
/// An [error](RuntimeErrorKind) with the faulty expression's [context](ErrCtx)
/// and the [stack trace](RuntimeErrorCtx::get_call_stack)
#[derive(Debug)]
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

/// # An error from `stck`
///
/// A failure that doesn't occour during the runtime of the stck script, but at some other time
#[derive(thiserror::Error, Debug)]
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
    #[error("Missing char")]
    MissingChar,
    #[error("Can't make closure with zero arguments, it's code spans these bytes: {span}")]
    CantInstanceClosureZeroArgs { span: LineRange },
    #[error("Parser in file {path}: Can only user param list or '*' as function arguments, not {0}", path=.1.display())]
    WrongParamList(String, PathBuf),
    #[error("Type `{0}` doesn't exist")]
    UnknownType(String),
    #[error("Can't parse TRC `{0}`, missing name")]
    TRCMissingName(String),
    #[error("Tried making a builtin module without a # prefix")]
    BuiltinModuleWithoutBang(String),
    #[error("Hosts can't make modules with the # prefix (sign of builtin module)")]
    UserModuleWithBang(String),
}

/// # A runtime error
///
/// An error that can only be caught during a failure while trying to execute a stck script
///
/// This is usually wrapped by a [context](RuntimeErrorCtx) to display more information
#[derive(thiserror::Error, Debug)]
pub enum RuntimeErrorKind {
    #[error("Not enough arguments to execute {name}, got {got:?} needs {needs:?}")]
    UserFnMissingArgs {
        name: String,
        got: Vec<Value>,
        needs: Vec<String>,
    },
    #[error("Found {} while executing `!` on a Result: {error}", "Error".bright_yellow())]
    UnwrapResultBuiltinFailed { error: Value },
    #[error("Found missing value while exeuting `!` on an Option")]
    UnwrapOptionBuiltinFailed,
    #[error("Can't compare {this} with {that}")]
    Compare { this: Value, that: Value },
    #[error("Switch case with no value")]
    SwitchCaseWithNoValue,
    #[error(
        "`%%` ({0}) doesn't recognise the format directive `{1}`, only '%', 'd', 's', 'v', 'V' and 'b' are avaliable"
    )]
    UnknownStringFormat(String, char),
    #[error("`%%` ({0}) Can't capture any value, the stack is empty")]
    MissingValue(String, char),
    #[error("`%%` ({0}) The provided value, {1}, can't be formatted with `{2}`")]
    WrongValueType(String, Value, char),
    #[error("Expected type: {0} got value {1}: {ty}", ty=TypeTester::from(.1.as_ref()))]
    Type(TypeTester, Box<Value>),
    #[error("Expected type: {0} got {1}")]
    TypeType(TypeTester, TypeTester),
    #[error("Output of function `{fn_name}` error, Expected {expected:?} got {got:?}")]
    OutputCount {
        fn_name: String,
        expected: usize,
        got: usize,
    },
    #[error("Output of closure error, Expected {expected:?} got {got:?}")]
    OutputClosureCount { expected: usize, got: usize },
    #[error("No such user-defined function `{0}`")]
    MissingUserFunction(String),
    #[error("WrongStackSizeDiffOnCheck {old_stack_size} -> {new_stack_size}")]
    WrongStackSizeDiffOnCheck {
        old_stack_size: usize,
        new_stack_size: usize,
        new_should_stack_size: usize,
    },
    #[error("check blocks must recieve one boolean, recieved {got}")]
    WrongTypeOnCheck { got: Value },
    #[error("Function {for_fn} accepts [{args}]. But {this_arg} is missing")]
    MissingValueForBuiltin {
        for_fn: String,
        args: String,
        this_arg: &'static str,
    },
    #[error("Function {for_fn} accepts {args}. But {missing} args are missing")]
    MissingValuesForBuiltin {
        for_fn: String,
        args: &'static str,
        missing: isize,
    },
    #[error(
        "Function {for_fn} accepts {args}. But [{this_arg}] must be a {expected} and got {got}"
    )]
    WrongTypeForBuiltin {
        for_fn: String,
        args: &'static str,
        this_arg: &'static str,
        got: Box<Value>,
        expected: &'static str,
    },
    #[error("The variable {0} is not defined")]
    NoSuchVariable(String),
    #[error(
        "Can't make function ({fn_name}) that takes no arguments into closure, since that would never be executed"
    )]
    CantMakeFnIntoClosureZeroArgs { fn_name: String },
    #[error(
        "Can't make function ({fn_name}) that takes entire stack into closure, since it would never be executed"
    )]
    CantMakeFnIntoClosureAllStack { fn_name: String },
    #[error("Can't make closure with zero arguments, it's code spans these bytes: {span:?}")]
    CantInstanceClosureZeroArgs { span: Range<usize> },
    #[error(
        "Closure's arguments ({:?}) are filled, but still tried to add more",
        closure_args
    )]
    DEVFillFullClosure { closure_args: ClosurePartialArgs },
    #[error(
        "Closure's arguments ({closure_args:?})'s parent function values are beeing reset with {parent_args:?}"
    )]
    DEVResettingParentValuesForClosure {
        closure_args: Box<ClosurePartialArgs>,
        parent_args: HashMap<ArgName, FnArg>,
    },
    #[error(
        "DEV ERROR, this error should never appear to you:\nThe struct {0}, with method {1:?}, tried using field with index {2}; {3}"
    )]
    DEVWrongIndexOnUserStructField(
        Rc<UserStructDef>,
        UserStructMethod,
        usize,
        Box<UserStructInstance>,
    ),
    #[error("No such function or function argument called `{0}`")]
    MissingIdent(String),
    #[error("Module `{0}` is required but was not loaded")]
    MissingModule(String),
    #[error("Method {0}'s fieldless action doesn't exist")]
    NoSuchFieldlessAction(String),
    #[error("Method {0}'s field doesn't exist")]
    NoSuchField(String),
    #[error("Method {0}'s action doesn't exist")]
    NoSuchAction(String),
    #[error("Not enough arguments to make {0}")]
    NotEnoughArgsForNew(Rc<UserStructDef>),
    #[error("Wrong value, {0}, for type {1}, while doing {2:?} on {3}")]
    WrongTypeForMethod(
        Box<Value>,
        Box<TypeTester>,
        UserStructMethod,
        Rc<UserStructDef>,
    ),
}
