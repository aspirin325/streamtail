use std::{env, process};

use streamtail::cli::{self, CliCommand};

fn main() {
    let command = cli::parse_args(env::args().skip(1));

    match command {
        Ok(CliCommand::Help) => {
            println!("{}", cli::help_text("streamtail"));
        }
        Ok(CliCommand::Version) => {
            println!("streamtail {}", env!("CARGO_PKG_VERSION"));
        }
        Ok(CliCommand::Run(config)) => {
            if let Err(err) = streamtail::run(config) {
                eprintln!("streamtail: {err}");
                process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("streamtail: {err}");
            eprintln!("try 'streamtail --help'");
            process::exit(2);
        }
    }
}
