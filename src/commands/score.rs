use crate::arcaea::play::{CreatePlay, Play};
use crate::arcaea::score::Score;
use crate::context::{Context, Error};
use crate::recognition::recognize::{ImageAnalyzer, ScoreKind};
use crate::user::{discord_id_to_discord_user, User};
use crate::{get_user, timed};
use image::DynamicImage;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::CreateMessage;

use super::discord::MessageContext;

// {{{ Score
/// Score management
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("magic", "delete", "show"),
	subcommand_required
)]
pub async fn score(_ctx: Context<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Score magic
// {{{ Implementation
async fn magic_impl<C: MessageContext>(
	ctx: &mut C,
	files: Vec<C::Attachment>,
) -> Result<(), Error> {
	let user = get_user!(ctx);
	let files = ctx.download_images(&files).await?;

	if files.len() == 0 {
		ctx.reply("No images found attached to message").await?;
		return Ok(());
	}

	let mut embeds = Vec::with_capacity(files.len());
	let mut attachments = Vec::with_capacity(files.len());
	let mut analyzer = ImageAnalyzer::default();

	for (i, (attachment, bytes)) in files.into_iter().enumerate() {
		// {{{ Preapare image
		let mut image = timed!("decode image", { image::load_from_memory(&bytes)? });
		let mut grayscale_image = timed!("grayscale image", {
			DynamicImage::ImageLuma8(image.to_luma8())
		});
		// image = image.resize(1024, 1024, FilterType::Nearest);
		// }}}

		let result: Result<(), Error> = try {
			// {{{ Detection

			// edit_reply!(ctx, handle, "Image {}: reading kind", i + 1).await?;
			let kind = timed!("read_score_kind", {
				analyzer.read_score_kind(ctx.data(), &grayscale_image)?
			});

			// edit_reply!(ctx, handle, "Image {}: reading difficulty", i + 1).await?;
			// Do not use `ocr_image` because this reads the colors
			let difficulty = timed!("read_difficulty", {
				analyzer.read_difficulty(ctx.data(), &image, &grayscale_image, kind)?
			});

			// edit_reply!(ctx, handle, "Image {}: reading jacket", i + 1).await?;
			let (song, chart) = timed!("read_jacket", {
				analyzer.read_jacket(ctx.data(), &mut image, kind, difficulty)?
			});

			let max_recall = match kind {
				ScoreKind::ScoreScreen => {
					// edit_reply!(ctx, handle, "Image {}: reading max recall", i + 1).await?;
					Some(analyzer.read_max_recall(ctx.data(), &grayscale_image)?)
				}
				ScoreKind::SongSelect => None,
			};

			grayscale_image.invert();
			let note_distribution = match kind {
				ScoreKind::ScoreScreen => {
					// edit_reply!(ctx, handle, "Image {}: reading distribution", i + 1).await?;
					Some(analyzer.read_distribution(ctx.data(), &grayscale_image)?)
				}
				ScoreKind::SongSelect => None,
			};

			// edit_reply!(ctx, handle, "Image {}: reading score", i + 1).await?;
			let score = timed!("read_score", {
				analyzer
					.read_score(ctx.data(), Some(chart.note_count), &grayscale_image, kind)
					.map_err(|err| {
						format!(
							"Could not read score for chart {} [{:?}]: {err}",
							song.title, chart.difficulty
						)
					})?
			});

			// {{{ Build play
			let maybe_fars =
				Score::resolve_distibution_ambiguities(score, note_distribution, chart.note_count);

			let play = CreatePlay::new(score)
				.with_attachment(C::attachment_id(attachment))
				.with_fars(maybe_fars)
				.with_max_recall(max_recall)
				.save(&ctx.data(), &user, &chart)?;
			// }}}
			// }}}
			// {{{ Deliver embed

			let (embed, attachment) = timed!("to embed", {
				play.to_embed(ctx.data(), &user, &song, &chart, i, None)?
			});

			embeds.push(embed);
			attachments.extend(attachment);
			// }}}
		};

		if let Err(err) = result {
			analyzer
				.send_discord_error(ctx, &image, C::filename(&attachment), err)
				.await?;
		}
	}

	if embeds.len() > 0 {
		ctx.send_files(attachments, CreateMessage::new().embeds(embeds))
			.await?;
	}

	Ok(())
}
// }}}
// {{{ Tests
#[cfg(test)]
mod magic_tests {
	use std::{path::PathBuf, process::Command, str::FromStr};

	use r2d2_sqlite::SqliteConnectionManager;

	use crate::{
		commands::discord::mock::MockContext,
		context::{connect_db, get_shared_context},
	};

	use super::*;

	macro_rules! with_ctx {
		($test_path:expr, $f:expr) => {{
			let mut data = (*get_shared_context().await).clone();
			let dir = tempfile::tempdir()?;
			let path = dir.path().join("db.sqlite");
			println!("path {path:?}");
			data.db = connect_db(SqliteConnectionManager::file(path));

			Command::new("scripts/import-charts.py")
				.env("SHIMMERING_DATA_DIR", dir.path().to_str().unwrap())
				.output()
				.unwrap();

			let mut ctx = MockContext::new(data);
			User::create_from_context(&ctx)?;

			let res: Result<(), Error> = $f(&mut ctx).await;
			res?;

			ctx.write_to(&PathBuf::from_str($test_path)?)?;
			Ok(())
		}};
	}

	#[tokio::test]
	async fn no_pics() -> Result<(), Error> {
		with_ctx!("test/commands/score/magic/no_pics", async |ctx| {
			magic_impl(ctx, vec![]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn basic_pic() -> Result<(), Error> {
		with_ctx!("test/commands/score/magic/single_pic", async |ctx| {
			magic_impl(
				ctx,
				vec![PathBuf::from_str("test/screenshots/alter_ego.jpg")?],
			)
			.await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn weird_kerning() -> Result<(), Error> {
		with_ctx!("test/commands/score/magic/weird_kerning", async |ctx| {
			magic_impl(
				ctx,
				vec![
					PathBuf::from_str("test/screenshots/antithese_74_kerning.jpg")?,
					PathBuf::from_str("test/screenshots/genocider_24_kerning.jpg")?,
				],
			)
			.await?;
			Ok(())
		})
	}
}
// }}}

/// Identify scores from attached images.
#[poise::command(prefix_command, slash_command)]
pub async fn magic(
	mut ctx: Context<'_>,
	#[description = "Images containing scores"] files: Vec<serenity::Attachment>,
) -> Result<(), Error> {
	magic_impl(&mut ctx, files).await?;

	Ok(())
}
// }}}
// {{{ Score delete
/// Delete scores, given their IDs.
#[poise::command(prefix_command, slash_command)]
pub async fn delete(
	mut ctx: Context<'_>,
	#[description = "Id of score to delete"] ids: Vec<u32>,
) -> Result<(), Error> {
	let user = get_user!(&mut ctx);

	if ids.len() == 0 {
		ctx.reply("Empty ID list provided").await?;
		return Ok(());
	}

	let mut count = 0;

	for id in ids {
		let res = ctx
			.data()
			.db
			.get()?
			.prepare_cached("DELETE FROM plays WHERE id=? AND user_id=?")?
			.execute((id, user.id))?;

		if res == 0 {
			ctx.reply(format!("No play with id {} found", id)).await?;
		} else {
			count += 1;
		}
	}

	if count > 0 {
		ctx.reply(format!("Deleted {} play(s) successfully!", count))
			.await?;
	}

	Ok(())
}
// }}}
// {{{ Score show
/// Show scores given their ides
#[poise::command(prefix_command, slash_command)]
pub async fn show(
	ctx: Context<'_>,
	#[description = "Ids of score to show"] ids: Vec<u32>,
) -> Result<(), Error> {
	if ids.len() == 0 {
		ctx.reply("Empty ID list provided").await?;
		return Ok(());
	}

	let mut embeds = Vec::with_capacity(ids.len());
	let mut attachments = Vec::with_capacity(ids.len());
	let conn = ctx.data().db.get()?;
	for (i, id) in ids.iter().enumerate() {
		let (song, chart, play, discord_id) = conn
			.prepare_cached(
				"
          SELECT 
            p.id, p.chart_id, p.user_id, p.created_at,
            p.max_recall, p.far_notes, s.score,
            u.discord_id
          FROM plays p
          JOIN scores s ON s.play_id = p.id
          JOIN users u ON p.user_id = u.id
          WHERE s.scoring_system='standard'
          AND p.id=?
          ORDER BY s.score DESC
          LIMIT 1
        ",
			)?
			.query_and_then([id], |row| -> Result<_, Error> {
				let (song, chart) = ctx.data().song_cache.lookup_chart(row.get("chart_id")?)?;
				let play = Play::from_sql(chart, row)?;
				let discord_id = row.get::<_, String>("discord_id")?;
				Ok((song, chart, play, discord_id))
			})?
			.next()
			.ok_or_else(|| format!("Could not find play with id {}", id))??;

		let author = discord_id_to_discord_user(&ctx, &discord_id).await?;
		let user = User::by_id(ctx.data(), play.user_id)?;

		let (embed, attachment) =
			play.to_embed(ctx.data(), &user, song, chart, i, Some(&author))?;

		embeds.push(embed);
		attachments.extend(attachment);
	}

	ctx.channel_id()
		.send_files(ctx.http(), attachments, CreateMessage::new().embeds(embeds))
		.await?;

	Ok(())
}
// }}}
