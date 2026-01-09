fn main() {
    if let Err(err) = executors::executors::fake_agent::run_fake_agent() {
        eprintln!("fake-agent failed: {err}");
        std::process::exit(1);
    }
}
