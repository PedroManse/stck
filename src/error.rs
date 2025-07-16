//! # Error handeling module

use super::*;
use crate::cache::FileCacher;
use colored::Colorize;
use std::collections::hash_map::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// # The error of highest order of the stck lib
///
/// Keeps either a [`Runtime error`](RuntimeErrorCtx) of [another type of error](StckError)
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
    /// Deprecated failure of reading file
    #[deprecated]
    #[error("Can't read file {0:?}")]
    CantReadFile(PathBuf),
    /// IO Failure
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Fail to parse string as integer
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
    /// Fail to parse string as float
    #[error(transparent)]
    ParseFloat(#[from] std::num::ParseFloatError),
    /// Couldn't find (pragma end if) on (pragma if) section
    #[error("No pragma section to (end if), on span {0}")]
    NoSectionToClose(LineRange),
    /// Can't use (pragma else) on current section (already execution else section)
    #[error("Can't start pragma (else) section on {1:?} (span {0:?})")]
    CantElseCurrentSection(LineRange, Option<crate::preproc::ProcCommand>),
    /// Invalid pragma command
    #[error("Invalid pragma command: {0}")]
    InvalidPragma(String),
    /// End of file while making token with definitive ending not yet found
    #[error("Unexpected end of file while building token {0:?}")]
    UnexpectedEOF(token::State),
    /// Invalid character with specific Tokenizer state
    #[error("Tokenizer: No impl for {0:?} with {1:?}")]
    CantTokenizerChar(token::State, char),
    /// Invalid token to parse with specific Parser state
    #[error(
        "Parser in file {path}: State ({0:?}): {state} doesn't accept token: {1:?}",
        path=.2.display().to_string().green(),
        state=.0.to_string().yellow()
    )]
    CantParseToken(parse::State, Box<TokenCont>, PathBuf),
    /// An unknown keyword tried to be tokenized
    #[error("Unknown keyword: {0}")]
    UnknownKeyword(String),
    #[deprecated]
    #[error("Missing char")]
    MissingChar,
    /// Can't create a closure with zero arguments
    #[error("Can't make closure with zero arguments, it's code spans these lines: {span}")]
    CantInstanceClosureZeroArgs { span: LineRange },
    /// Functions only accept an argument list or a '*' as their arguments
    #[error("Parser in file {path}: Can only user param list or '*' as function arguments, not {0}", path=.1.display())]
    WrongParamList(String, PathBuf),
    /// Type is unknown
    ///
    /// Generic types must start with capital letters
    #[error("Type `{0}` doesn't exist")]
    UnknownType(String),
    /// A Defined Generic must have a name
    #[error("Can't parse TRC `{0}`, missing name")]
    TRCMissingName(String),
    /// Deprecated User module starts with bang
    ///
    /// Bang `#` prefixes are now allowed, but not used by builtin module
    #[deprecated]
    #[error("Hosts can't make modules with the # prefix (sign of builtin module)")]
    UserModuleWithBang(String),
}

/// # A runtime error
///
/// An error that can be caught during a failure while trying to execute a stck script
///
/// This is usually wrapped by a [context](RuntimeErrorCtx) to display more information
#[derive(thiserror::Error, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(not(feature = "exhaustive-errors"), non_exhaustive)]
pub enum RuntimeErrorKind {
    /// Tried to execute a user-defined functions but there weren't enough arguments in the stack
    #[error("Not enough arguments to execute {name}, got {got:?} needs {needs:?}")]
    UserFnMissingArgs {
        name: String,
        got: Vec<Value>,
        needs: Vec<String>,
    },
    /// `!` (aka unwrap) builtin executed on Error
    ///
    /// The unwrap builtin works like [`Result::unwrap`],
    /// on a [`Value::Result`], but an Error variant was found
    #[error("Found {} while executing `!` on a Result: {error}", "Error".bright_yellow())]
    UnwrapResultBuiltinFailed { error: Value },
    /// `!` (aka unwrap) builtin executed on None
    ///
    /// The unwrap builtin works like [`Option::unwrap`],
    /// on a [`Value::Option`], but a None variant was found
    #[error("Found missing value while exeuting `!` on an Option")]
    UnwrapOptionBuiltinFailed,
    /// Tried to compare values of incompatible types
    #[error("Can't compare {this} with {that}")]
    Compare { this: Value, that: Value },
    /// Tried to execute `(switch)` without any value in the stack to be used as the case
    #[error("Switch case with no value")]
    SwitchCaseWithNoValue,
    /// Tried to format a string with the `%%` builtin but an unknown format directive was used
    #[error(
        "`%%` ({0}) doesn't recognise the format directive `{1}`, only '%', 'd', 's', 'v', 'V' and 'b' are avaliable"
    )]
    UnknownStringFormat(String, char),
    /// Tried to format a string with `%%` but there weren't enough values in the stack
    #[error("`%%` ({0}) Can't capture any value, the stack is empty")]
    MissingValue(String, char),
    /// Tried to format a string with `%%` but a value of the wrong type was popped
    #[error("`%%` ({0}) The provided value, {1}, can't be formatted with `{2}`")]
    WrongValueType(String, Value, char),
    /// Concrete type error
    ///
    /// A value was given but a function or closure expected another type
    #[error("Expected type: {0} got value {1}: {ty}", ty=TypeTester::from(.1.as_ref()))]
    Type(TypeTester, Box<Value>),
    /// Abstract type error
    ///
    /// An input or return type of a higher order closure didn't match the expected type from
    /// another function or closure
    #[error("Expected type: {0} got {1}")]
    TypeType(TypeTester, TypeTester),
    /// Wrong output count from function
    ///
    /// A function returned the wrong amount of values according to it's return argument
    /// list
    #[error("Output of function `{fn_name}` error, Expected {expected:?} got {got:?}")]
    OutputCount {
        fn_name: String,
        expected: usize,
        got: usize,
    },
    /// Wrong output count from a closure
    ///
    /// A closure returned the wrong amount of values according to it's return argument
    /// list
    #[error("Output of closure error, Expected {expected:?} got {got:?}")]
    OutputClosureCount { expected: usize, got: usize },
    /// Failed to find a user-defined function
    ///
    /// Different from [`RuntimeErrorKind::MissingIdent`] because this only applies to user-defined
    /// functions. This error may occour if the `(@<fn name>)` keyword was executed but the
    /// functions doesn't exist
    #[error("No such user-defined function `{0}`")]
    MissingUserFunction(String),
    /// A check code block was executed and didn't create exactly one value
    ///
    /// Maybe this type will be removed together with the restriction
    #[error("WrongStackSizeDiffOnCheck {old_stack_size} -> {new_stack_size}")]
    WrongStackSizeDiffOnCheck {
        old_stack_size: usize,
        new_stack_size: usize,
        new_should_stack_size: usize,
    },
    /// A check code block created a non-bool value
    #[error("check blocks must recieve one boolean, recieved {got}")]
    WrongTypeOnCheck { got: Value },
    /// Not enough values where provided to a builtin
    #[error("Function {for_fn} accepts [{args}]. But {this_arg} is missing")]
    MissingValueForBuiltin {
        for_fn: String,
        args: String,
        this_arg: &'static str,
    },
    /// Not enough values where provided to a builtin with dynamic capture size
    #[error("Function {for_fn} accepts {args}. But {missing} args are missing")]
    MissingValuesForBuiltin {
        for_fn: String,
        args: &'static str,
        missing: isize,
    },
    /// A value provided for a builtin was of incorrect type
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
    /// Tried to get an inexistant variable
    #[error("The variable {0} is not defined")]
    NoSuchVariable(String),
    /// User tried to convert a user function into a closure, but it had zero arguments
    #[error(
        "Can't make function ({fn_name}) that takes no arguments into closure, since that would never be executed"
    )]
    CantMakeFnIntoClosureZeroArgs { fn_name: String },
    /// User tried to convert a user function into a closure, but it had All Stack Capture
    #[error(
        "Can't make function ({fn_name}) that takes entire stack into closure, since it would never be executed"
    )]
    CantMakeFnIntoClosureAllStack { fn_name: String },
    /// User tried to create a closure, but it had zero arguments
    #[error("Can't make closure with zero arguments, it's code spans these bytes: {span:?}")]
    CantInstanceClosureZeroArgs { span: Range<usize> },
    /// DEVERROR, this error should never be triggered
    ///
    /// Tried to [`Closure::fill`] already full closure
    #[error(
        "Closure's arguments ({:?}) are filled, but still tried to add more",
        closure_args
    )]
    DEVFillFullClosure { closure_args: ClosurePartialArgs },
    /// DEVERROR, this error should never be triggered
    ///
    /// Tried to Immediate a closure that already had parent-scope arguments.
    ///
    /// This error could be removed with an enum for closures with and without scope arguments.
    #[error(
        "Closure's arguments ({closure_args:?})'s parent function values are beeing reset with {parent_args:?}"
    )]
    DEVResettingParentValuesForClosure {
        closure_args: Box<ClosurePartialArgs>,
        parent_args: HashMap<ArgName, FnArg>,
    },
    /// DEVERROR, this error should never be triggered
    ///
    /// Internl user structure workings resolved a field's index into an out-of-bounds index for
    /// the field list of the structure
    #[error(
        "DEV ERROR, this error should never appear to you:\nThe struct {0}, with method {1:?}, tried using field with index {2}; {3}"
    )]
    DEVWrongIndexOnUserStructField(
        Rc<UserStructDef>,
        UserStructMethod,
        usize,
        Box<UserStructInstance>,
    ),
    /// User tried to execute a function, builtin or hook that doesn't exist
    #[error("No such function or function argument called `{0}`")]
    MissingIdent(String),
    /// User requires a module that isn't loaded
    #[error("Module `{0}` is required but was not loaded")]
    MissingModule(String),
    /// User tried to execute an action on/about a structure, but the method doesn't exist
    #[error("Method {0}'s fieldless action doesn't exist")]
    NoSuchFieldlessAction(String),
    /// User tried to access a field that doesn't exist
    #[error("Method {0}'s field doesn't exist")]
    NoSuchField(String),
    /// User tried to use a structure action that doesn't exist
    #[error("Method {0}'s action doesn't exist")]
    NoSuchAction(String),
    /// User tried to create a structure but there weren't enough values in the stack
    #[error("Not enough arguments to make {0}")]
    NotEnoughArgsForNew(Rc<UserStructDef>),
    /// User tried to execute a method but the type of the provided value was wrong
    #[error("Wrong value, {0}, for type {1}, while doing {2:?} on {3}")]
    WrongTypeForMethod(
        Box<Value>,
        Box<TypeTester>,
        UserStructMethod,
        Rc<UserStructDef>,
    ),
    // TODO: better docs or overall option, maybe #217
    /// User tried to execute an action disabled
    #[error("Tried to execute dissalowed action: {0}")]
    DisallowedAction(UnauthorizedAction),
}
