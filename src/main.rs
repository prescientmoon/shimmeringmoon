#![warn(clippy::str_to_string)]
#![feature(iter_map_windows)]
#![feature(let_chains)]
#![feature(array_try_map)]
#![feature(async_closure)]
#![feature(try_blocks)]

mod arcaea;
mod assets;
mod bitmap;
mod commands;
mod context;
mod levenshtein;
mod logs;
mod recognition;
mod time;
mod transform;
mod user;

use assets::get_data_dir;
use context::{Error, UserContext};
use poise::serenity_prelude::{self as serenity};
use sqlx::sqlite::SqlitePoolOptions;
use std::{env::var, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

// {{{ Error handler
async fn on_error(error: poise::FrameworkError<'_, UserContext, Error>) {
	match error {
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
	let data_dir = get_data_dir();
	let cache_dir = var("SHIMMERING_CACHE_DIR").expect("Missing `SHIMMERING_CACHE_DIR` env var");

	let pool = SqlitePoolOptions::new()
		.connect(&format!(
			"sqlite://{}/db.sqlite",
			data_dir.to_str().unwrap()
		))
		.await
		.unwrap();

	// {{{ Poise options
	let options = poise::FrameworkOptions {
		commands: vec![
			commands::help(),
			commands::score::score(),
			commands::stats::stats(),
			commands::chart::chart(),
		],
		prefix_options: poise::PrefixFrameworkOptions {
			stripped_dynamic_prefix: Some(|_ctx, message, _user_ctx| {
				Box::pin(async {
					if message.author.bot || Into::<u64>::into(message.author.id) == 1 {
						Ok(None)
					} else if message.content.starts_with("!") {
						Ok(Some(message.content.split_at(1)))
					} else if message.guild_id.is_none() {
						if message.content.trim().len() == 0 {
							Ok(Some(("", "score magic")))
						} else {
							Ok(Some(("", &message.content[..])))
						}
					} else {
						Ok(None)
					}
				})
			}),
			edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
				Duration::from_secs(3600),
			))),
			..Default::default()
		},
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
				let ctx = UserContext::new(data_dir, PathBuf::from_str(&cache_dir)?, pool).await?;

				Ok(ctx)
			})
		})
		.options(options)
		.build();

	let token =
		var("SHIMMERING_DISCORD_TOKEN").expect("Missing `SHIMMERING_DISCORD_TOKEN` env var");
	let intents =
		serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

	let client = serenity::ClientBuilder::new(token, intents)
		.framework(framework)
		.await;

	client.unwrap().start().await.unwrap()
	// }}}
}
