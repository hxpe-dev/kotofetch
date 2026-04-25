mod anki;
mod cli;
mod config;
mod display;
mod quotes;

use crate::cli::{Cli, Commands, InitSource};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Init {
            source: InitSource::Anki(args),
        }) => {
            if let Err(e) = anki::run_init(args) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        None => {
            let user_cfg = config::load_user_config(cli.config.clone());
            let runtime = config::make_runtime_config(user_cfg, &cli);
            display::render(&runtime, &cli);
        }
    }
}
