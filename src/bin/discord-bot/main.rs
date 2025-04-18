use poise::serenity_prelude::{self as serenity};
use shimmeringmoon::arcaea::play::generate_missing_scores;
use shimmeringmoon::context::{Error, UserContext};
use shimmeringmoon::{commands, timed};
use std::{env::var, sync::Arc, time::Duration};

// {{{ Error handler
async fn on_error(error: poise::FrameworkError<'_, UserContext, Error>) {
	if let Err(e) = poise::builtins::on_error(error).await {
		println!("Error while handling error: {}", e)
	}
}
// }}}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// {{{ Poise options
	let options = poise::FrameworkOptions {
		commands: vec![
			commands::help(),
			commands::score::score(),
			commands::stats::stats(),
			commands::chart::chart(),
			commands::calc::calc(),
			commands::user::user(),
		],
		prefix_options: poise::PrefixFrameworkOptions {
			stripped_dynamic_prefix: Some(|_ctx, message, _user_ctx| {
				Box::pin(async {
					let global_prefix = std::env::var("SHIMMERING_GLOBAL_PREFIX");
					if message.author.bot || Into::<u64>::into(message.author.id) == 1 {
						return Ok(None);
					}

					if let Ok(global_prefix) = global_prefix {
						if message.content.starts_with(&global_prefix) {
							return Ok(Some(message.content.split_at(global_prefix.len())));
						}
					}

					if message.guild_id.is_none() {
						if message.content.trim().is_empty() {
							return Ok(Some(("", "score magic")));
						} else {
							return Ok(Some(("", &message.content[..])));
						}
					}

					Ok(None)
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
				println!("🔒 Logged in as {}", _ready.user.name);
				poise::builtins::register_globally(ctx, &framework.options().commands).await?;
				Ok(UserContext::new().unwrap())
			})
		})
		.options(options)
		.build();

	if var("SHIMMERING_REGEN_SCORES").unwrap_or_default() == "1" {
		timed!("generate_missing_scores", {
			generate_missing_scores(framework.user_data().await).await?;
		});
	}

	let token =
		var("SHIMMERING_DISCORD_TOKEN").expect("Missing `SHIMMERING_DISCORD_TOKEN` env var");
	let intents =
		serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

	let client = serenity::ClientBuilder::new(token, intents)
		.framework(framework)
		.await;

	client.unwrap().start().await?;
	// }}}

	Ok(())
}
