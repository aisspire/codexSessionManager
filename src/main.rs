fn main() {
    if let Err(error) = codex_session_manager::cli::run() {
        eprintln!("error: {error:?}");
        std::process::exit(1);
    }
}
