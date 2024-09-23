// {{{ Imports
use std::path::PathBuf;

use crate::context::CliContext;
use shimmeringmoon::commands::discord::MessageContext;
use shimmeringmoon::commands::score::magic_impl;
use shimmeringmoon::context::{Error, UserContext};
// }}}

#[derive(clap::Args)]
pub struct Args {
	files: Vec<PathBuf>,
}

pub async fn run(args: Args) -> Result<(), Error> {
	let mut ctx = CliContext::new(UserContext::new().await?);
	let res = magic_impl(&mut ctx, &args.files).await;
	ctx.handle_error(res).await?;
	Ok(())
}
