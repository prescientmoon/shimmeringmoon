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
async fn main() {
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
						if message.content.trim().is_empty() {
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
				let ctx = UserContext::new().await?;

				if var("SHIMMERING_REGEN_SCORES").unwrap_or_default() == "1" {
					timed!("generate_missing_scores", {
						generate_missing_scores(&ctx).await?;
					});
				}

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
