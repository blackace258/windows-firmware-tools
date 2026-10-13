//! bios-tool binary entry point.

use bios_tool::cli::Cli;
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    if let Err(e) = cli.run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}
