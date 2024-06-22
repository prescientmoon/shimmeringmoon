#![warn(clippy::str_to_string)]
#![feature(iter_map_windows)]

mod chart;
mod commands;
mod context;
mod score;
mod user;

use chart::SongCache;
use context::{Error, UserContext};
use poise::serenity_prelude as serenity;
use score::score_to_zeta_score;
use sqlx::sqlite::SqlitePoolOptions;
use std::{env::var, sync::Arc, time::Duration};

// {{{ Error handler
async fn on_error(error: poise::FrameworkError<'_, UserContext, Error>) {
	match error {
		poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
		poise::FrameworkError::Command { error, ctx, .. } => {
			println!("Error in command `{}`: {:?}", ctx.command().name, error,);
		}
		error => {
			if let Err(e) = poise::builtins::on_error(error).await {
				println!("Error while handling error: {}", e)
			}
		}
	}
}
// }}}

#[tokio::main]
async fn main() {
	let data_dir = var("SHIMMERING_DATA_DIR")
		.expect("Missing `SHIMMERING_DATA_DIR` env var, see README for more information.");

	let pool = SqlitePoolOptions::new()
		.connect(&format!("sqlite://{}/db.sqlite", data_dir))
		.await
		.unwrap();

	println!("{:?}", score_to_zeta_score(9966677, 1303));
	println!("{:?}", score_to_zeta_score(9970525, 1303));

	// {{{ Poise options
	let options = poise::FrameworkOptions {
		commands: vec![commands::help(), commands::score()],
		prefix_options: poise::PrefixFrameworkOptions {
			prefix: Some("!".into()),
			edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
				Duration::from_secs(3600),
			))),
			..Default::default()
		},
		// The global error handler for all error cases that may occur
		on_error: |error| Box::pin(on_error(error)),
		..Default::default()
	};
	// }}}
	// {{{ Start poise
	let framework = poise::Framework::builder()
		.setup(move |ctx, _ready, framework| {
			Box::pin(async move {
				println!("Logged in as {}", _ready.user.name);
				poise::builtins::register_globally(ctx, &framework.options().commands).await?;
				let mut ctx = UserContext::new(pool);
				ctx.song_cache = SongCache::new(&ctx).await?;
				Ok(ctx)
			})
		})
		.options(options)
		.build();

	let token = var("SHIMMERING_DISCORD_TOKEN")
		.expect("Missing `SHIMMERING_DISCORD_TOKEN` env var, see README for more information.");
	let intents =
		serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

	let client = serenity::ClientBuilder::new(token, intents)
		.framework(framework)
		.await;

	client.unwrap().start().await.unwrap()
	// }}}
}
