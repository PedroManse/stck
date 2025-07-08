use super::*;
#[cfg(not(test))]
use std::process::Command;

#[cfg(test)]
pub(super) fn sh(shell_cmd: &str) -> Result<isize, String> {
    eprintln!("[CMD] {shell_cmd}");
    Ok(0)
}

#[cfg(not(test))]
pub(super) fn sh(shell_cmd: &str) -> Result<isize, String> {
    Command::new("sh")
        .arg("-c")
        .arg(shell_cmd)
        .status()
        .map(|s| s.code().unwrap_or(256) as isize)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
pub(super) fn write_to(cont: &str, file: &str) -> Result<isize, String> {
    eprintln!("Write {} bytes to {file}", cont.len());
    Ok(cont.len() as isize)
}

#[cfg(not(test))]
pub(super) fn write_to(cont: &str, file: &str) -> Result<isize, String> {
    use std::io::prelude::Write;
    let mut file = std::fs::File::create(file).map_err(|e| e.to_string())?;
    match file.write_all(cont.as_bytes()) {
        Ok(()) => Ok(cont.len() as isize),
        Err(e) => Err(e.to_string()),
    }
}

enum FmtError {
    MissingValue(char),
    UnknownStringFormat(char),
    WrongVariableForFormat(Value, char),
}

fn fmt_internal(cont: &str, stack: &mut Stack) -> Result<String, FmtError> {
    enum State {
        Nothing,
        OnFmt,
    }
    let mut out = String::with_capacity(cont.len());
    let mut state = State::Nothing;
    for ch in cont.chars() {
        state = match (state, ch) {
            (State::Nothing, '%') => State::OnFmt,
            (State::Nothing, ch) => {
                out.push(ch);
                State::Nothing
            }
            (State::OnFmt, '%') => {
                out.push('%');
                State::Nothing
            }
            (State::OnFmt, 's') => {
                let add_str = stack_pop!(=(stack) -> str? as "%s" for "%%")
                    .ok_or(FmtError::MissingValue('s'))?
                    .map_err(|v| FmtError::WrongVariableForFormat(v, 's'))?;
                out.push_str(&add_str);
                State::Nothing
            }
            (State::OnFmt, 'd') => {
                let add_num = stack_pop!(=(stack) -> num? as "%d" for "%%")
                    .ok_or(FmtError::MissingValue('d'))?
                    .map_err(|v| FmtError::WrongVariableForFormat(v, 'd'))?;
                out.push_str(&add_num.to_string());
                State::Nothing
            }
            (State::OnFmt, 'v') => {
                let fmt = match stack.pop() {
                    Some(x) => format!("{x:?}"),
                    None => "<Nothing in stack>".to_string(),
                };
                out.push_str(&fmt);
                State::Nothing
            }
            (State::OnFmt, 'V') => {
                let fmt = match stack.pop() {
                    Some(x) => format!("{x}"),
                    None => "<Nothing in stack>".to_string(),
                };
                out.push_str(&fmt);
                State::Nothing
            }
            (State::OnFmt, 'b') => {
                let add_bool = stack_pop!(=(stack) -> bool? as "%b" for "%%")
                    .ok_or(FmtError::MissingValue('b'))?
                    .map_err(|v| FmtError::WrongVariableForFormat(v, 'b'))?;
                out.push_str(&add_bool.to_string());
                State::Nothing
            }
            (State::OnFmt, x) => {
                return Err(FmtError::UnknownStringFormat(x));
            }
        }
    }
    Ok(out)
}

pub(super) fn fmt(cont: &str, stack: &mut Stack) -> Result<String, RuntimeErrorKind> {
    fmt_internal(cont, stack).map_err(|e| {
        let fmt_str = cont.to_string();
        match e {
            FmtError::MissingValue(c) => RuntimeErrorKind::MissingValue(fmt_str, c),
            FmtError::UnknownStringFormat(c) => RuntimeErrorKind::UnknownStringFormat(fmt_str, c),
            FmtError::WrongVariableForFormat(v, c) => {
                RuntimeErrorKind::WrongValueType(fmt_str, v, c)
            }
        }
    })
}
