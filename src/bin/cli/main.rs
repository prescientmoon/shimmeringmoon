use clap::Parser;
use command::{Cli, Command};
use shimmeringmoon::context::Error;

mod command;
mod commands;
mod context;

#[tokio::main]
async fn main() -> Result<(), Error> {
	let cli = Cli::parse();
	match cli.command {
		Command::Analyse(args) => {
			commands::analyse::run(args).await?;
		}
	}

	Ok(())
}
