use std::time::Instant;

use crate::arcaea::play::{CreatePlay, Play};
use crate::arcaea::score::Score;
use crate::context::{Context, Error};
use crate::recognition::recognize::{ImageAnalyzer, ScoreKind};
use crate::user::{discord_id_to_discord_user, User};
use crate::{edit_reply, get_user, timed};
use image::DynamicImage;
use poise::serenity_prelude::futures::future::join_all;
use poise::serenity_prelude::CreateMessage;
use poise::{serenity_prelude as serenity, CreateReply};

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
/// Identify scores from attached images.
#[poise::command(prefix_command, slash_command)]
pub async fn magic(
	ctx: Context<'_>,
	#[description = "Images containing scores"] files: Vec<serenity::Attachment>,
) -> Result<(), Error> {
	let user = get_user!(&ctx);

	if files.len() == 0 {
		ctx.reply("No images found attached to message").await?;
		return Ok(());
	}

	let mut embeds = Vec::with_capacity(files.len());
	let mut attachments = Vec::with_capacity(files.len());
	let handle = ctx
		.reply(format!("Processed 0/{} scores", files.len()))
		.await?;

	let mut analyzer = ImageAnalyzer::default();

	// {{{ Download files
	let download_tasks = files
		.iter()
		.filter(|file| file.dimensions().is_some())
		.map(|file| async move { (file, file.download().await) });

	let downloaded = timed!("dowload_files", { join_all(download_tasks).await });

	if downloaded.len() < files.len() {
		ctx.reply("One or more of the attached files are not images!")
			.await?;
	}
	// }}}

	for (i, (file, bytes)) in downloaded.into_iter().enumerate() {
		let bytes = bytes?;

		let start = Instant::now();
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
				analyzer.read_score(ctx.data(), Some(chart.note_count), &grayscale_image, kind)?
			});

			// {{{ Build play
			let maybe_fars =
				Score::resolve_distibution_ambiguities(score, note_distribution, chart.note_count);

			let play = CreatePlay::new(score)
				.with_attachment(file)
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
				.send_discord_error(ctx, &image, &file.filename, err)
				.await?;
		}

		let took = start.elapsed();

		edit_reply!(
			ctx,
			handle,
			"Processed {}/{} scores. Last score took {took:?} to process.",
			i + 1,
			files.len()
		)
		.await?;
	}

	handle.delete(ctx).await?;

	if embeds.len() > 0 {
		ctx.channel_id()
			.send_files(ctx.http(), attachments, CreateMessage::new().embeds(embeds))
			.await?;
	}

	Ok(())
}
// }}}
// {{{ Score delete
/// Delete scores, given their IDs.
#[poise::command(prefix_command, slash_command)]
pub async fn delete(
	ctx: Context<'_>,
	#[description = "Id of score to delete"] ids: Vec<u32>,
) -> Result<(), Error> {
	let user = get_user!(&ctx);

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
