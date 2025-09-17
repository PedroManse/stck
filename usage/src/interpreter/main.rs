use stck::prelude::*;

fn exec_file(
    ctx: &mut stck::internals::HostContext,
    file_path: String,
    file_cacher: &mut CacheHelper,
) -> Result<(), stck::Error> {
    ctx.execute_entire_code(&get_project_code(file_path, file_cacher)?)?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    let file_path = std::env::args().nth(1).unwrap();
    let mut file_cacher = CacheHelper::new();
    let mut exec_ctx = RuntimeContext::new();
    if let Err(e) = exec_file(&mut exec_ctx, file_path, &mut file_cacher) {
        println!("{e}");
        std::process::ExitCode::from(1)
    } else {
        std::process::ExitCode::from(0)
    }
}
