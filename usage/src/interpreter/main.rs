use clap::{Parser, ValueEnum};
use colored::Colorize;
use stck::prelude::*;
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum StckMode {
    /// Normal execution
    Normal,
    /// Normal execution but dump variables and stack on error
    Debug,
    /// Check valdity of tokens
    TokenCheck,
    /// Check vailidity of expressions and pre-processor commands
    SyntaxCheck,
    /// Dump code after pre-processing
    PrintCode,
}

fn print_code(code: &stck::internals::Code, import_stack: usize) {
    for expr in code {
        if import_stack != 0 {
            println!(
                "{} {} @ {}",
                ">".repeat(import_stack).blue(),
                expr.cont,
                expr.span
            );
        } else {
            println!("{} @ {}", expr.cont, expr.span);
        }
        if let stck::internals::ExprCont::IncludedCode(code) = &expr.cont {
            print_code(code, import_stack + 1);
        }
    }
}

fn execute(
    mode: StckMode,
    file_path: PathBuf,
    file_cache: &mut impl cache::FileCacher,
    exec_ctx: &mut RuntimeContext,
) -> Result<(), stck::Error> {
    use StckMode as M;
    match mode {
        M::Normal | M::Debug => {
            let code = get_project_code(file_path, file_cache)?;
            exec_ctx.execute_entire_code(&code)?;
        }
        M::SyntaxCheck => {
            get_project_code(file_path, file_cache)?;
        }
        M::PrintCode => {
            let code = get_project_code(file_path, file_cache)?;
            print_code(&code, 0);
        }
        M::TokenCheck => {
            get_tokens(file_path, file_cache)?;
        }
    }
    Ok(())
}

#[derive(Parser, Debug)]
struct Cli {
    file: PathBuf,

    #[arg(short, long, value_name = "Mode", default_value = "normal")]
    mode: StckMode,
}

fn main() {
    let Cli { file, mode } = Cli::parse();
    let mut file_cacher = CacheHelper::new();
    let mut ctx = RuntimeContext::new();

    if let Err(e) = execute(mode, file, &mut file_cacher, &mut ctx) {
        eprintln!("\n{e}");
        if let stck::Error::RuntimeError(e) = e {
            let spans: stck::error::ErrorSpans = e.into();
            let sources = spans.try_into_sources(&mut file_cacher).unwrap();
            for source in sources {
                println!("{source}");
            }
        }

        if mode == StckMode::Debug {
            let vars = ctx.get_vars().clone();
            println!("===[ Stack ]===");
            for (n, v) in ctx.get_stack().iter().rev().enumerate() {
                println!("#{n}: {v}");
            }
            println!("---------------");
            println!("Vars: {vars:?}");
        }
        std::process::exit(1);
    }
}
