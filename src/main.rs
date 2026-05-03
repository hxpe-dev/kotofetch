mod anki;
mod cli;
mod config;
mod display;
mod quotes;

use crate::cli::{Cli, Commands, InitSource};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use std::io;

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
        Some(Commands::Completion { shell }) => {
            let mut cmd = Cli::command();
            let cmdname = cmd.get_name().to_string();
            generate(*shell, &mut cmd, cmdname, &mut io::stdout());
        }
        None => {
            let user_cfg = config::load_user_config(cli.config.clone());
            let runtime = config::make_runtime_config(user_cfg, &cli);
            display::render(&runtime, &cli);
        }
    }
}
