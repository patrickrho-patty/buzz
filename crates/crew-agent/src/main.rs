fn main() {
    if let Err(e) = crew_agent::run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
