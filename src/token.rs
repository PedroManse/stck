use std::path::PathBuf;
use std::str::FromStr;

use crate::{
    DefinedGenericBuilder, FnArgDef, FnScope, LineRange, RawKeyword, StckError, Token, TokenBlock,
    TokenCont,
};

type Result<T> = std::result::Result<T, StckError>;

pub struct Context {
    changed_line: bool,
    current_line: usize,
    point: usize,
    chars: Vec<char>,
}

#[derive(Debug)]
pub enum State {
    Nothing,
    OnComment,
    MakeIdent(String),
    MakeString(String, usize),
    MakeStringEsc(String, usize), // found \ on string
    Minus(String),
    MakeNumber(String),
    MakeFloat(String),
    MakeKeyword(String, usize),
    MakeFnArgs(Vec<FnArgDef>, String, usize),
    MakeFnArgType {
        args: Vec<FnArgDef>,
        arg_name: String,
        type_buf: String,
        tag_count: usize,
        line_start: usize,
    },
    MakeChar,
    MakeCharEnd(char),
    MakeCharEndEsc(char),
}

macro_rules! matches {
    (arg_ident) => {
        'a'..='z' | 'A'..='Z' | '_' | '-' | '&'
    };
    (ident) => {
        (matches!(start_ident) | matches!(digit) | '\'' | '-')
    };
    (arg_type) => {
        (matches!(letter) | matches!(space) | '?' | '*')
    };
    (letter) => {
        'a'..='z' | 'A'..='Z'
    };
    (start_ident) => {
        'a'..='z' | 'A'..='Z' | '+' | '_' | '%' | '!' | '?' | '$' | '=' | '/' | '*' | '&' | '<' | '>' | '≃' | ',' | ':' | '~' | '@' | '.'
    };
    (word_edge) => {
        '(' | ')' | '{' | '}' | '[' | ']'
    };
    (space) => {
        ' ' | '\n' | '\t'
    };
    (digit) => {
        ('0'..='9')
    }
}

impl Context {
    fn push_token(&mut self, out: &mut Vec<Token>, token: TokenCont) {
        let span = LineRange::from_points(self.current_line, self.current_line);
        out.push(Token { cont: token, span });
    }
    fn push_multiline_token(&mut self, out: &mut Vec<Token>, token: TokenCont, line_start: usize) {
        let span = LineRange::from_points(line_start, self.current_line);
        out.push(Token { cont: token, span });
    }
    pub fn tokenize(mut self, source: PathBuf) -> Result<TokenBlock> {
        let tokens = self.tokenize_block()?;
        Ok(TokenBlock { source, tokens })
    }

    // just read a '{'
    fn tokenize_block(&mut self) -> Result<Vec<Token>> {
        use State::*;
        use TokenCont::*;
        let mut state = Nothing;
        let mut out = Vec::new();

        while let Some(ch) = self.next() {
            state = match (state, ch) {
                // code block
                (Nothing, '{') => {
                    let block_start = self.current_line;
                    let block = self.tokenize_block()?;
                    self.push_multiline_token(&mut out, Block(block), block_start);
                    Nothing
                }
                (Nothing, '}') => {
                    self.push_token(&mut out, EndOfBlock);
                    return Ok(out);
                }

                // char
                (Nothing, '\'') => MakeChar,
                (MakeChar, c) => MakeCharEnd(*c),
                (MakeCharEnd('\\'), c @ ('\\' | '\'')) => MakeCharEndEsc(*c),
                (MakeCharEnd(c), '\'') => {
                    self.push_token(&mut out, Char(c));
                    Nothing
                }
                (MakeCharEnd('\\'), 'n') => MakeCharEndEsc('\n'),
                (MakeCharEndEsc(c), '\'') => {
                    self.push_token(&mut out, Char(c));
                    Nothing
                }

                // minus (either number or identifier)
                (Nothing, c @ '-') => Minus(String::from(*c)),
                (Minus(mut buf), c @ matches!(digit)) => {
                    buf.push(*c);
                    MakeNumber(buf)
                }
                (Minus(buf), _) => {
                    self.unget();
                    MakeIdent(buf)
                }

                // identifier
                (Nothing, c @ matches!(start_ident)) => MakeIdent(String::from(*c)),
                (MakeIdent(mut buf), c @ matches!(ident)) => {
                    buf.push(*c);
                    MakeIdent(buf)
                }
                (MakeIdent(buf), matches!(space)) => {
                    self.push_token(&mut out, Ident(buf));
                    Nothing
                }
                (MakeIdent(buf), matches!(word_edge)) => {
                    self.push_token(&mut out, Ident(buf));
                    self.unget(); // re-read char with Nothing State
                    Nothing
                }

                // string
                (Nothing, '"') => MakeString(String::new(), self.current_line),
                (MakeString(buf, line_start), '"') => {
                    self.push_multiline_token(&mut out, Str(buf), line_start);
                    Nothing
                }
                (MakeString(buf, line_start), '\\') => MakeStringEsc(buf, line_start),
                (MakeString(mut buf, line_start), c) => {
                    buf.push(*c);
                    MakeString(buf, line_start)
                }
                (MakeStringEsc(mut buf, line_start), '\\') => {
                    buf.push('\\');
                    MakeString(buf, line_start)
                }
                (MakeStringEsc(mut buf, line_start), 'n') => {
                    buf.push('\n');
                    MakeString(buf, line_start)
                }

                // number
                (Nothing, c @ matches!(digit)) => MakeNumber(String::from(*c)),
                (MakeNumber(mut buf), c @ matches!(digit)) => {
                    buf.push(*c);
                    MakeNumber(buf)
                }
                (MakeNumber(buf), matches!(space)) => {
                    let num = buf.parse()?;
                    self.push_token(&mut out, Number(num));
                    Nothing
                }
                (MakeNumber(buf), matches!(word_edge)) => {
                    let num = buf.parse()?;
                    self.push_token(&mut out, Number(num));
                    self.unget(); // re-read char with Nothing State
                    Nothing
                }
                (MakeNumber(mut buf), c @ '.') => {
                    buf.push(*c);
                    MakeFloat(buf)
                }
                (MakeNumber(buf), ',') => {
                    let num = buf.parse()?;
                    self.push_token(&mut out, Number(num));
                    self.push_token(&mut out, Ident(",".to_string()));
                    Nothing
                }

                // Make float
                (MakeFloat(mut buf), c @ matches!(digit)) => {
                    buf.push(*c);
                    MakeFloat(buf)
                }
                (MakeFloat(buf), matches!(space)) => {
                    let num = buf.parse()?;
                    self.push_token(&mut out, Float(num));
                    Nothing
                }
                (MakeFloat(buf), matches!(word_edge)) => {
                    let num = buf.parse()?;
                    self.push_token(&mut out, Float(num));
                    self.unget(); // re-read char with Nothing State
                    Nothing
                }

                // keyword
                (Nothing, '(') => MakeKeyword(String::new(), self.current_line),
                (MakeKeyword(buf, line_start), ')') => {
                    let kw = match buf.as_str().trim() {
                        "!" => RawKeyword::BubbleError,
                        "struct" => RawKeyword::Structure,
                        "fn" => RawKeyword::Fn(FnScope::Local),
                        "fn*" => RawKeyword::Fn(FnScope::Global),
                        "fn-" => RawKeyword::Fn(FnScope::Isolated),
                        "while" => RawKeyword::While,
                        "return" => RawKeyword::Return,
                        "switch" => RawKeyword::Switch,
                        "break" => RawKeyword::Break,
                        "ifs" => RawKeyword::Ifs,
                        "try @" => RawKeyword::TryClosure,
                        otherwise => {
                            let include =
                                otherwise
                                    .strip_prefix("include ")
                                    .map(|p| RawKeyword::Include {
                                        path: p.trim().into(),
                                    });
                            let pragma = otherwise
                                .strip_prefix("pragma")
                                .map(str::trim)
                                .map(String::from)
                                .map(|command| RawKeyword::Pragma { command });
                            let fn_into_closure = otherwise
                                .strip_prefix("@")
                                .map(String::from)
                                .map(|fn_name| RawKeyword::FnIntoClosure { fn_name });
                            let trc = otherwise
                                .strip_prefix("TRC")
                                .map(str::trim)
                                .map(DefinedGenericBuilder::from_str)
                                .and_then(Result::ok)
                                .map(RawKeyword::from);
                            let require = otherwise
                                .strip_prefix("require")
                                .map(str::trim)
                                .map(String::from)
                                .map(RawKeyword::Require);
                            // In case of old (require) invocations regarding protected
                            // modules, simply remove the # prefix
                            let require_old_protected = otherwise
                                .strip_prefix("require #")
                                .map(str::trim)
                                .map(String::from)
                                .map(RawKeyword::Require);
                            let try_fn = otherwise
                                .strip_prefix("try ")
                                .map(str::trim)
                                .map(String::from)
                                .map(RawKeyword::Try);
                            include
                                .or(pragma)
                                .or(fn_into_closure)
                                .or(trc)
                                .or(require_old_protected)
                                .or(require)
                                .or(try_fn)
                                .ok_or(StckError::UnknownKeyword(otherwise.to_string()))?
                        }
                    };
                    self.push_multiline_token(&mut out, Keyword(kw), line_start);
                    Nothing
                }
                (MakeKeyword(mut buf, line_start), c) => {
                    buf.push(*c);
                    MakeKeyword(buf, line_start)
                }

                // arg list
                (Nothing, '[') => MakeFnArgs(Vec::new(), String::new(), self.current_line),
                (MakeFnArgs(mut xs, buf, line_start), matches!(space)) => {
                    if !buf.is_empty() {
                        xs.push(FnArgDef::new_untyped(buf));
                    }
                    MakeFnArgs(xs, String::new(), line_start)
                }
                (MakeFnArgs(args, arg_name, line_start), '<') => MakeFnArgType {
                    args,
                    arg_name,
                    type_buf: String::new(),
                    tag_count: 0,
                    line_start,
                },
                (
                    MakeFnArgType {
                        args,
                        arg_name,
                        mut type_buf,
                        tag_count,
                        line_start,
                    },
                    c @ matches!(arg_type),
                ) => {
                    type_buf.push(*c);
                    MakeFnArgType {
                        args,
                        arg_name,
                        type_buf,
                        tag_count,
                        line_start,
                    }
                }
                (
                    MakeFnArgType {
                        args,
                        arg_name,
                        mut type_buf,
                        tag_count,
                        line_start,
                    },
                    c @ '<',
                ) => {
                    type_buf.push(*c);
                    MakeFnArgType {
                        args,
                        arg_name,
                        type_buf,
                        tag_count: tag_count + 1,
                        line_start,
                    }
                }
                (
                    MakeFnArgType {
                        mut args,
                        arg_name,
                        type_buf,
                        tag_count: 0,
                        line_start,
                    },
                    '>',
                ) => {
                    let x = FnArgDef::new_typed(arg_name, type_buf.trim().parse()?);
                    args.push(x);
                    MakeFnArgs(args, String::new(), line_start)
                }
                (
                    MakeFnArgType {
                        args,
                        arg_name,
                        mut type_buf,
                        tag_count,
                        line_start,
                    },
                    c @ '>',
                ) => {
                    type_buf.push(*c);
                    MakeFnArgType {
                        args,
                        arg_name,
                        type_buf,
                        tag_count: tag_count - 1,
                        line_start,
                    }
                }
                (MakeFnArgs(xs, mut buf, line_start), c @ matches!(arg_ident)) => {
                    buf.push(*c);
                    MakeFnArgs(xs, buf, line_start)
                }
                (MakeFnArgs(mut xs, buf, line_start), ']') => {
                    if !buf.is_empty() {
                        xs.push(FnArgDef::new_untyped(buf));
                    }
                    self.push_multiline_token(&mut out, FnArgs(xs), line_start);
                    Nothing
                }

                // comment
                (Nothing, '#') => OnComment,
                (OnComment, '\n') => Nothing,
                (OnComment, _) => OnComment,
                (Nothing, matches!(space)) => Nothing,

                (s, c) => {
                    return Err(StckError::CantTokenizerChar(s, *c));
                }
            }
        }
        match state {
            Nothing | OnComment => {}
            MakeIdent(s) => {
                self.push_token(&mut out, Ident(s));
            }
            MakeNumber(buf) => {
                let num = buf.parse()?;
                self.push_token(&mut out, Number(num));
            }
            s => return Err(StckError::UnexpectedEOF(s)),
        }
        self.push_token(&mut out, EndOfBlock);
        Ok(out)
    }

    fn next(&mut self) -> Option<&char> {
        let ch = self.chars.get(self.point)?;
        if self.changed_line {
            self.current_line += 1;
            self.changed_line = false;
        }
        if ch == &'\n' {
            self.changed_line = true;
        }
        self.point += 1;
        Some(ch)
    }

    // to re-read char with differnt State
    fn unget(&mut self) {
        self.point -= 1;
        if self.chars.get(self.point) == Some(&'\n') {
            self.current_line -= 1;
        }
    }

    pub fn new(code: &str) -> Self {
        let chars: Vec<char> = code.chars().collect();
        Self {
            changed_line: false,
            point: 0,
            chars,
            current_line: 1,
        }
    }
}
