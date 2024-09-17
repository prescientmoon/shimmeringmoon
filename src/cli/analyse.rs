// {{{ Imports
use std::path::PathBuf;

use crate::cli::context::CliContext;
use crate::commands::score::magic_impl;
use crate::context::{Error, UserContext};
// }}}

#[derive(clap::Args)]
pub struct Args {
	files: Vec<PathBuf>,
}

pub async fn run(args: Args) -> Result<(), Error> {
	let mut ctx = CliContext::new(UserContext::new().await?);
	magic_impl(&mut ctx, &args.files).await?;
	Ok(())
}
