use crate::arcaea::play::{CreatePlay, Play};
use crate::arcaea::score::Score;
use crate::context::{Context, Error};
use crate::recognition::recognize::{ImageAnalyzer, ScoreKind};
use crate::user::{discord_it_to_discord_user, User};
use crate::{edit_reply, get_user};
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
	} else {
		let mut embeds = Vec::with_capacity(files.len());
		let mut attachments = Vec::with_capacity(files.len());
		let handle = ctx
			.reply(format!("Processed 0/{} scores", files.len()))
			.await?;

		let mut analyzer = ImageAnalyzer::default();

		for (i, file) in files.iter().enumerate() {
			if let Some(_) = file.dimensions() {
				let bytes = file.download().await?;
				let mut image = image::load_from_memory(&bytes)?;
				// image = image.resize(1024, 1024, FilterType::Nearest);

				let result: Result<(), Error> = try {
					// {{{ Detection
					// This makes OCR more likely to work
					let mut ocr_image = image.grayscale().blur(1.);

					edit_reply!(ctx, handle, "Image {}: reading kind", i + 1).await?;
					let kind = analyzer.read_score_kind(ctx.data(), &ocr_image)?;

					edit_reply!(ctx, handle, "Image {}: reading difficulty", i + 1).await?;
					// Do not use `ocr_image` because this reads the colors
					let difficulty = analyzer.read_difficulty(ctx.data(), &image, kind)?;

					edit_reply!(ctx, handle, "Image {}: reading jacket", i + 1).await?;
					let (song, chart) = analyzer
						.read_jacket(ctx.data(), &mut image, kind, difficulty)
						.await?;

					ocr_image.invert();

					let (note_distribution, max_recall) = match kind {
						ScoreKind::ScoreScreen => {
							edit_reply!(ctx, handle, "Image {}: reading distribution", i + 1)
								.await?;
							let note_distribution =
								Some(analyzer.read_distribution(ctx.data(), &image)?);

							edit_reply!(ctx, handle, "Image {}: reading max recall", i + 1).await?;
							let max_recall = Some(analyzer.read_max_recall(ctx.data(), &image)?);

							(note_distribution, max_recall)
						}
						ScoreKind::SongSelect => (None, None),
					};

					edit_reply!(ctx, handle, "Image {}: reading score", i + 1).await?;
					let score = analyzer.read_score(
						ctx.data(),
						Some(chart.note_count),
						&ocr_image,
						kind,
					)?;

					// {{{ Build play
					let maybe_fars = Score::resolve_distibution_ambiguities(
						score,
						note_distribution,
						chart.note_count,
					);

					let play = CreatePlay::new(score, &chart, &user)
						.with_attachment(file)
						.with_fars(maybe_fars)
						.with_max_recall(max_recall)
						.save(&ctx.data())
						.await?;
					// }}}
					// }}}
					// {{{ Deliver embed
					let (embed, attachment) = play
						.to_embed(&ctx.data().db, &user, &song, &chart, i, None)
						.await?;

					embeds.push(embed);
					attachments.extend(attachment);
					// }}}
				};

				if let Err(err) = result {
					analyzer
						.send_discord_error(ctx, &image, &file.filename, err)
						.await?;
				}
			} else {
				ctx.reply("One of the attached files is not an image!")
					.await?;
				continue;
			}

			edit_reply!(ctx, handle, "Processed {}/{} scores", i + 1, files.len()).await?;
		}

		handle.delete(ctx).await?;

		if embeds.len() > 0 {
			ctx.channel_id()
				.send_files(ctx.http(), attachments, CreateMessage::new().embeds(embeds))
				.await?;
		}
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
