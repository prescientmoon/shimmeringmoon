pub mod prepare_jackets;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
	#[command(subcommand)]
	pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
	/// Start the discord bot
	Discord {},
	PrepareJackets {},
}
