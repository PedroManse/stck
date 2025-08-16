use super::*;
use colored::Colorize;
use std::fmt::{Display, Formatter};

pub struct DisplayArgs<'a>(pub &'a [super::FnArgDef]);

impl Display for DisplayArgs<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ ")?;
        for arg in self.0 {
            if let Some(tt) = arg.get_type() {
                write!(f, "{arg}<{}> ", tt.to_string().underline().blue())?;
            } else {
                write!(f, "{arg} ")?;
            }
        }
        write!(f, "]")
    }
}

impl Display for FnArgs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllStack => write!(f, "the stack"),
            Self::Args(args) => write!(f, "{}", display::DisplayArgs(args)),
        }
    }
}

impl Display for FnScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FnScope::Local => Ok(()),
            FnScope::Global => f.write_str("*"),
            FnScope::Isolated => f.write_str("-"),
        }
    }
}

impl Display for TypedFnPart {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedFnPart::Typed(args) => {
                write!(f, " ")?;
                for arg in args {
                    write!(f, "{} ", arg.to_string().underline().blue())?;
                }
                Ok(())
            }
            TypedFnPart::Any => write!(f, "?"),
        }
    }
}

impl Display for TypeTester {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use TypeTester::*;
        match self {
            Structure(s) => write!(f, "structure {}", s.name),
            Float => write!(f, "float"),
            Any => write!(f, "?"),
            Char => write!(f, "char"),
            Str => write!(f, "str"),
            Num => write!(f, "num"),
            Bool => write!(f, "bool"),
            ArrayAny => write!(f, "array"),
            MapAny => write!(f, "map"),
            ResultAny => write!(f, "result"),
            OptionAny => write!(f, "option"),
            ClosureAny => write!(f, "fn"),
            Array(t) => write!(f, "array<{t}>"),
            Map(v) => write!(f, "map<{v}>"),
            Option(t) => write!(f, "option<{t}>"),
            Result(tt) => write!(f, "result<{}><{}>", tt.0, tt.1),
            Closure(tin, tout) => write!(f, "fn<{tin}><{tout}>"),
            Generic(a) => write!(f, "{a}"),
        }
    }
}

impl Display for ExprCont {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MakeClosure(cl) => {
                write!(f, "Instantiate Closure {cl:p}")
            }
            Self::Immediate(v) => {
                write!(f, "Push value {v:?}")
            }
            Self::FnCall(fn_name) => {
                write!(f, "Execute `{}`", fn_name.bright_yellow())
            }
            Self::IncludedCode(code) => {
                write!(
                    f,
                    "Included file {}",
                    code.source.display().to_string().green()
                )
            }
            Self::Keyword(k) => {
                write!(f, "Keyword: ")?;
                match k {
                    KeywordKind::TryClosure => write!(f, "Try executing closure"),
                    KeywordKind::Try { fn_name } => write!(f, "Try executing function {fn_name}"),
                    KeywordKind::Structure { name, .. } => write!(f, "Struct {name}"),
                    KeywordKind::Require(mn) => write!(f, "Require module {mn}"),
                    KeywordKind::DefinedGeneric(g) => write!(f, "Define generic {g:?}"),
                    KeywordKind::Break => write!(f, "Break"),
                    KeywordKind::Return => write!(f, "Return"),
                    KeywordKind::IntoClosure { fn_name } => write!(f, "`{fn_name}` into Closure"),
                    KeywordKind::Ifs { .. } => write!(f, "If"),
                    KeywordKind::BubbleError => write!(f, "Bubble error"),
                    KeywordKind::While { .. } => write!(f, "While"),
                    KeywordKind::FnDef {
                        name,
                        scope,
                        args,
                        out_args: Some(out_args),
                        ..
                    } => write!(
                        f,
                        "Define fn{scope} `{}` as {args} → {}",
                        name.bright_yellow(),
                        DisplayArgs(out_args)
                    ),
                    KeywordKind::FnDef {
                        name,
                        scope,
                        args,
                        out_args: None,
                        ..
                    } => write!(
                        f,
                        "Define fn{scope} `{}` consuming {args}",
                        name.bright_yellow()
                    ),
                    KeywordKind::Switch { .. } => write!(f, "Switch"),
                }
            }
        }
    }
}

impl Display for FnArgDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Display for ErrCtx {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} in {}:{}",
            self.expr.cont,
            self.source.display().to_string().green(),
            self.lines.to_string().bright_magenta().underline(),
        )
    }
}

impl Display for LineRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.delta() == 0 {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}:{:+}", self.start, self.delta())
        }
    }
}

impl Display for ErrorSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let source = self.source.display().to_string();
        let range = self.range.to_string();
        let title_len = source.len() + range.len() + 11;
        writeln!(
            f,
            "===[ {}:{} ]===",
            source.green(),
            range.bright_magenta().underline()
        )?;
        writeln!(f, "{}", self.lines)?;
        writeln!(f, "{}", "-".repeat(title_len).dimmed())?;
        Ok(())
    }
}

impl Display for parse::State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Nothing => "Making nothing",

            Self::MakeIfs(..) => "Making ifs, awaiting conditional check or end",
            Self::MakeIfsCode { .. } => "Making ifs, awaiting code block to execute",

            Self::MakeFnArgs(..) => "Making function, awaiting args",
            Self::MakeFnNameOrOutArgs(..) => "Making function, awaiting name or output args",
            Self::MakeFnName(..) => "Making function, awaiting name",
            Self::MakeFnBlock(..) => "Make function, awaiting code block to execute",

            Self::MakeStructureName => "Make structure def, awaiting name",
            Self::MakeStructureBody { .. } => "Make struct def, awaiting values",

            Self::MakeSwitch(..) => "Making switch case, awaiting value to match",
            Self::MakeSwitchCode(..) => "Making switch case, awaiting code block to execute",

            Self::MakeWhile => "Making while loop, awaiting check code block",
            Self::MakeWhileCode(..) => "MakeWhile while lop, awaiting code block to execute",

            Self::MakeClosureBlockOrOutArgs(..) => {
                "Making closure, awaiting code block to execute or output args"
            }
            Self::MakeClosureBlock(..) => "Making closure, awaiting code block to execute",
        };
        write!(f, "{s}")
    }
}

impl Display for UserStructDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {{ ", self.name.italic())?;
        let last_index = self.fields.len() - 1;
        for (index, def) in self.fields.iter().enumerate() {
            if index == last_index {
                write!(f, "{}: {} ", def.name, def.type_check)?;
            } else {
                write!(f, "{}: {}, ", def.name, def.type_check)?;
            }
        }
        write!(f, "}}")
    }
}

impl Display for UserStructInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {{ ", self.def.name.italic())?;
        let last_index = self.def.fields.len() - 1;
        for (index, (def, val)) in self.def.fields.iter().zip(&self.fields).enumerate() {
            if index == last_index {
                write!(f, "{}: {} ", def.name, val)?;
            } else {
                write!(f, "{}: {}, ", def.name, val)?;
            }
        }
        write!(f, "}}")
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Structure(s) => {
                write!(f, "{s}")
            }
            Value::Result(r) => match r.as_ref() {
                Result::Ok(t) => write!(f, "{}<{}>", "Ok".bright_yellow(), t),
                Result::Err(e) => write!(f, "{}<{}>", "Error".bright_yellow(), e),
            },
            Value::Str(s) => write!(f, "\"{}\"", s.green()),
            Value::Num(n) => write!(f, "{}", n.to_string().bright_cyan()),
            Value::Float(n) => write!(f, "{}", n.to_string().bright_cyan()),
            Value::Char(c) => write!(f, "'{}'", c.to_string().green()),
            Value::Bool(b) => write!(f, "{}", b.to_string().purple()),
            Value::Option(o) => match o.as_ref() {
                Option::None => write!(f, "{}", "None".bright_yellow()),
                Option::Some(t) => write!(f, "{}<{t}>", "Some".bright_yellow()),
            },
            Value::Array(a) => {
                write!(f, "{}<", "Array".bright_yellow())?;
                let last_idx = a.len() - 1;
                for (idx, v) in a.iter().enumerate() {
                    if idx == last_idx {
                        write!(f, "{v}")?;
                    } else {
                        write!(f, "{v} ")?;
                    }
                }
                write!(f, ">")
            }
            Value::Map(m) => f.debug_map().entries(m).finish(),
            Value::Closure(c) => write!(f, "Closure <...> -> <...> @ {c:p}"),
        }
    }
}

impl Display for Stack {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "[")?;
        for v in self.as_slice() {
            writeln!(f, "{v}")?;
        }
        write!(f, "]")
    }
}

impl Display for error::UnauthorizedAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let x = match self {
            Self::Include => "Include",
            Self::WhileLoop => "While loop",
            Self::ExecuteClosure => "Execute closure",
            Self::DeclareFunction => "Declare function",
        };
        write!(f, "{x}")
    }
}
