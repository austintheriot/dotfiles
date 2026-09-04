use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("config-manifest {VERSION}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: config-manifest --version");
            ExitCode::from(2)
        }
    }
}
