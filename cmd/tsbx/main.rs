use anyhow::Result;
use std::env;

#[path = "../../internal/config/mod.rs"]
mod config;
#[path = "../../internal/configure/mod.rs"]
mod configure;
#[path = "../../internal/runtime/mod.rs"]
mod runtime;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.len() <= 1 {
        print_usage();
        std::process::exit(1);
    }

    let command = args[1].to_lowercase();
    match command.as_str() {
        "start" => {
            if let Err(err) = cmd_start() {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
        }
        "configure" => {
            if let Err(err) = cmd_configure() {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
        }
        "version" | "-v" | "--version" => {
            println!("tsbx {}", env!("CARGO_PKG_VERSION"));
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            println!();
            print_usage();
            std::process::exit(1);
        }
    }
}

fn cmd_start() -> Result<()> {
    let cfg = config::load_or_default()?;
    runtime::start(&cfg)
}

fn cmd_configure() -> Result<()> {
    let cfg = config::load_or_default()?;
    configure::run(cfg)?;
    Ok(())
}

fn print_usage() {
    println!("TSBX CLI (Linux)");
    println!();
    println!("Usage:");
    println!("  tsbx start       Start a sandbox session");
    println!("  tsbx configure   Configure inference credentials");
    println!("  tsbx version     Show CLI version");
}
