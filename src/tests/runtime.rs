use std::path::PathBuf;

use super::*;
use crate::{
    ErrCtx, RuntimeErrorCtx, RuntimeErrorKind, api,
    cache::Isolated,
    error::Error,
    internals::{self, RuntimeContext, RustStckFn, Value},
};

fn execute_string(cont: &str, test_name: &str) -> Result<RuntimeContext, Error> {
    let mut file_cacher = Isolated::new();
    let tokens = api::get_tokens_str(cont, test_name, &mut file_cacher)?;
    let code = api::parse_raw_tokens(tokens)?;
    let mut runtime = RuntimeContext::new();
    runtime.execute_entire_code(&code)?;
    Ok(runtime)
}

#[test]
fn rust_hook() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let tokens = api::get_tokens_str("\"7 3 -\n\" eval\n", "test rust hook", &mut file_cacher)?;
    let code = api::parse_raw_tokens(tokens)?;
    let mut runtime = RuntimeContext::new();
    let hook = RustStckFn::new("eval".to_string(), |ctx, source| {
        let st = ctx.stack.pop_this(Value::get_str).unwrap().unwrap();
        let tokens =
            api::get_tokens_str(&st, format!("Eval at {source:?}"), &mut Isolated::new()).unwrap();
        let code = api::parse_raw_tokens(tokens).unwrap();
        ctx.execute_entire_code(&code).unwrap();
    });
    runtime.add_rust_hook(hook);
    runtime.execute_entire_code(&code)?;
    let stack = runtime.get_stack();
    let expected_stack = [Value::Num(4)];
    test_eq!(got: stack, expected: expected_stack);
    Ok(())
}

#[test]
fn closure_parent_args() -> Result<(), Error> {
    let ctx = execute_string(
        "
(fn) [ i<num> ] [ <num> ] double { i 2 * }
(fn) [ first<fn> seccond<fn> ] [ joint<fn> ] join {
    [ v ]{ first seccond v @ @ }
}

(@double) (@double) join 2 @


[ a ] {
    [ b ] {
        [ _ ] {
            a b -
        }
    }
}

3 @ 4 @ '_' @
",
        "Test nested arguments",
    )?;
    let stack = ctx.get_stack();
    let expected_stack = [Value::Num(8), Value::Num(-1)];
    test_eq!(got: stack, expected: expected_stack);
    Ok(())
}

#[test]
fn structure_usage_copy() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    let code = r#"
(fn) [a b] flip {b a}
(fn) [a b c] rot3 { c a b }
(struct) Person [
  name<str>
  age<num>
  job<option<str>>
]

"Manse" 19 "developer" some Person$new
Person$copy$age

"Ravi" 2 none Person$new
Person$copy$age

flip rot3 -
"#;
    let code = api::get_tokens_str(code, "structure_usage_copy code", &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;
    runtime.execute_entire_code(&code)?;
    let out = runtime.stack.pop_this(Value::get_num);
    assert_eq!(out, Some(Ok(17)));
    Ok(())
}

#[test]
fn structure_usage_swap() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    let code = r#"
(struct) Person [
  name<str>
  age<num>
  job<option<str>>
]

"Manse" 19 "developer" some Person$new
20 Person$swap$age

"#;
    let code = api::get_tokens_str(code, "structure_usage_swap code", &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;
    runtime.execute_entire_code(&code)?;
    let out = runtime.stack.pop_this(Value::get_num);
    assert_eq!(out, Some(Ok(19)));
    Ok(())
}

#[test]
fn structure_usage_set() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    let code = r#"
(struct) Person [
  name<str>
  age<num>
  job<option<str>>
]

"Manse" 19 "developer" some Person$new
20 Person$set$age
Person$copy$age

"#;
    let code = api::get_tokens_str(code, "structure_usage_set code", &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;
    runtime.execute_entire_code(&code)?;
    let out = runtime.stack.pop_this(Value::get_num);
    assert_eq!(out, Some(Ok(20)));
    Ok(())
}

#[test]
fn structure_usage_explode() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    let code = r#"
(struct) Person [
  name<str>
  age<num>
  job<option<str>>
]

"Manse" 19 "developer" some Person$new Person$explode

"#;
    let code = api::get_tokens_str(code, "structure_usage_set code", &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;
    runtime.execute_entire_code(&code)?;
    let out = runtime.stack.pop_this(Value::get_option);
    assert_eq!(
        out,
        Some(Ok(Some(Box::new(Value::Str("developer".to_string())))))
    );
    Ok(())
}

#[test]
fn ifs() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    let code = r#"
(fn) [ a<T> ] [ <T> <T> ] dup { a a }

(ifs) { dup 10 = } {
    "It's ten"
} { dup 12 = } {
    "It's twelve"
} { dup 14 = } {
    "It's fourteen"
} {
    "IDK"
}
"#;
    let code = api::get_tokens_str(code, "ifs code", &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;

    let tests = [
        (10, "It's ten"),
        (12, "It's twelve"),
        (14, "It's fourteen"),
        (4, "IDK"),
    ];
    for (input, expected_output) in tests {
        runtime.stack.push_this(input);
        runtime.execute_entire_code(&code)?;
        let out = runtime.stack.pop_this(Value::get_str);
        assert_eq!(out, Some(Ok(expected_output.to_string())));
    }
    Ok(())
}

#[test]
fn if_else() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    let code = r#"
(ifs) { 10 10 = } {
    "if code path"
} {
    "else code path"
}
"#;
    let code = api::get_tokens_str(code, "if_else code", &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;
    runtime.execute_entire_code(&code)?;
    let out = runtime.stack.pop_this(Value::get_str);
    assert_eq!(out, Some(Ok("if code path".to_string())));
    Ok(())
}

#[test]
fn runtime_stack() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    let code = "
(fn) [ a b ] func-b { }
(fn) [ v ] func-a { v func-b }

10 func-a
";
    let source = PathBuf::from("error stack code");
    let code = api::get_tokens_str(code, source.clone(), &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;
    let error = runtime.execute_entire_code(&code);
    assert_eq!(
        Err(RuntimeErrorCtx {
            ctx: ErrCtx::new(
                &source,
                &crate::Expr {
                    span: crate::LineRange { start: 3, end: 3 },
                    cont: crate::ExprCont::FnCall("func-b".to_string())
                }
            ),
            kind: Box::new(RuntimeErrorKind::UserFnMissingArgs {
                name: "func-b".to_string(),
                got: vec![Value::Num(10)],
                needs: vec!["a".to_string(), "b".to_string()]
            }),
            stack: vec![ErrCtx::new(
                &source,
                &crate::Expr {
                    span: crate::LineRange { start: 5, end: 5 },
                    cont: crate::ExprCont::FnCall("func-a".to_string())
                }
            )],
        }),
        error,
    );
    Ok(())
}

#[test]
fn exec_options() -> Result<(), Error> {
    let mut file_cacher = Isolated::new();
    let mut runtime = RuntimeContext::new();
    runtime
        .set_option_include(true)
        .set_option_while_loop(false);
    let code = r#"
(while) { 1 1 = } {
    "yes" print
}
"#;
    let code = api::get_tokens_str(code, "while rt_option", &mut file_cacher)?;
    let code = api::parse_raw_tokens(code)?;
    let e = runtime.execute_entire_code(&code);
    let ex_e = RuntimeErrorCtx {
        ctx: ErrCtx {
            source: PathBuf::from("while rt_option"),
            expr: Box::new(internals::Expr {
                span: crate::error::LineRange { start: 2, end: 4 },
                cont: internals::ExprCont::Keyword(internals::KeywordKind::While {
                    check: vec![
                        internals::Expr {
                            span: crate::error::LineRange { start: 2, end: 2 },
                            cont: internals::ExprCont::Immediate(Value::Num(1)),
                        },
                        internals::Expr {
                            span: crate::error::LineRange { start: 2, end: 2 },
                            cont: internals::ExprCont::Immediate(Value::Num(1)),
                        },
                        internals::Expr {
                            span: crate::error::LineRange { start: 2, end: 2 },
                            cont: internals::ExprCont::FnCall("=".to_string()),
                        },
                    ],
                    code: vec![
                        internals::Expr {
                            span: crate::error::LineRange { start: 3, end: 3 },
                            cont: internals::ExprCont::Immediate(Value::Str("yes".to_string())),
                        },
                        internals::Expr {
                            span: crate::error::LineRange { start: 3, end: 3 },
                            cont: internals::ExprCont::FnCall("print".to_string()),
                        },
                    ],
                }),
            }),
            lines: crate::error::LineRange { start: 2, end: 4 },
        },
        kind: Box::new(crate::error::RuntimeErrorKind::DisallowedAction(crate::runtime::UnauthorizedAction::WhileLoop)),
        stack: vec![],
    };
    assert_eq!(e, Err(ex_e));
    Ok(())
}
