use clap::Parser;
use command::{Cli, Command};
use shimmeringmoon::context::{Error, UserContext};

mod command;
mod commands;
mod context;

#[tokio::main]
async fn main() -> Result<(), Error> {
	let cli = Cli::parse();
	match cli.command {
		Command::PrepareJackets {} => {
			commands::prepare_jackets::run()?;
		}
		Command::Analyse(args) => {
			commands::analyse::run(args).await?;
		}
	}

	Ok(())
}
