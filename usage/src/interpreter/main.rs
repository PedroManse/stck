use stck::prelude::*;

fn main() -> Result<(), stck::Error> {
    let file_path = std::env::args().nth(1).unwrap();
    let mut file_cacher = CacheHelper::new();
    let mut exec_ctx = RuntimeContext::new();
    exec_ctx.register_module(internals::module::io::make());
    let code = get_project_code(file_path, &mut file_cacher)?;
    if let Err(e) = exec_ctx.execute_entire_code(&code) {
        println!("{e}");
    }
    Ok(())
}
