use std::time::Instant;

use crate::arcaea::play::{CreatePlay, Play};
use crate::arcaea::score::Score;
use crate::context::{Context, Error};
use crate::recognition::recognize::{ImageAnalyzer, ScoreKind};
use crate::user::{discord_it_to_discord_user, User};
use crate::{edit_reply, get_user, timed};
use image::DynamicImage;
use poise::serenity_prelude::futures::future::join_all;
use poise::serenity_prelude::CreateMessage;
use poise::{serenity_prelude as serenity, CreateReply};
use sqlx::query;

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

			let play = CreatePlay::new(score, &chart, &user)
				.with_attachment(file)
				.with_fars(maybe_fars)
				.with_max_recall(max_recall)
				.save(&ctx.data())
				.await?;
			// }}}
			// }}}
			// {{{ Deliver embed

			let (embed, attachment) = timed!("to embed", {
				play.to_embed(&ctx.data().db, &user, &song, &chart, i, None)
					.await?
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
		let res = query!("DELETE FROM plays WHERE id=? AND user_id=?", id, user.id)
			.execute(&ctx.data().db)
			.await?;

		if res.rows_affected() == 0 {
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
	for (i, id) in ids.iter().enumerate() {
		let res = query!(
			"
        SELECT 
          p.id,p.chart_id,p.user_id,p.score,p.zeta_score,
          p.max_recall,p.created_at,p.far_notes,
          u.discord_id
        FROM plays p 
        JOIN users u ON p.user_id = u.id
        WHERE p.id=?
      ",
			id
		)
		.fetch_one(&ctx.data().db)
		.await
		.map_err(|_| format!("Could not find play with id {}", id))?;

		let play = Play {
			id: res.id as u32,
			chart_id: res.chart_id as u32,
			user_id: res.user_id as u32,
			score: Score(res.score as u32),
			zeta_score: Score(res.zeta_score as u32),
			max_recall: res.max_recall.map(|r| r as u32),
			far_notes: res.far_notes.map(|r| r as u32),
			created_at: res.created_at,
			discord_attachment_id: None,
			creation_ptt: None,
			creation_zeta_ptt: None,
		};

		let author = discord_it_to_discord_user(&ctx, &res.discord_id).await?;
		let user = User::by_id(&ctx.data().db, play.user_id).await?;

		let (song, chart) = ctx.data().song_cache.lookup_chart(play.chart_id)?;
		let (embed, attachment) = play
			.to_embed(&ctx.data().db, &user, song, chart, i, Some(&author))
			.await?;

		embeds.push(embed);
		attachments.extend(attachment);
	}

	ctx.channel_id()
		.send_files(ctx.http(), attachments, CreateMessage::new().embeds(embeds))
		.await?;

	Ok(())
}
// }}}
