use std::fmt::Display;

use crate::context::{Context, Error};
use crate::score::{CreatePlay, ImageCropper, Play, Score, ScoreKind};
use crate::user::{discord_it_to_discord_user, User};
use image::imageops::FilterType;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed, CreateMessage};
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
// {{{ Send error embed with image
async fn error_with_image(
	ctx: Context<'_>,
	bytes: &[u8],
	filename: &str,
	message: &str,
	err: impl Display,
) -> Result<(), Error> {
	let error_attachement = CreateAttachment::bytes(bytes, filename);
	let msg = CreateMessage::default().embed(
		CreateEmbed::default()
			.title(message)
			.attachment(filename)
			.description(format!("{}", err)),
	);

	ctx.channel_id()
		.send_files(ctx.http(), [error_attachement], msg)
		.await?;

	Ok(())
}
// }}}

/// Identify scores from attached images.
#[poise::command(prefix_command, slash_command)]
pub async fn magic(
	ctx: Context<'_>,
	#[description = "Images containing scores"] files: Vec<serenity::Attachment>,
) -> Result<(), Error> {
	let user = match User::from_context(&ctx).await {
		Ok(user) => user,
		Err(_) => {
			ctx.say("You are not an user in my database, sorry!")
				.await?;
			return Ok(());
		}
	};

	println!("Handling command from user {:?}", user.discord_id);

	if files.len() == 0 {
		ctx.reply("No images found attached to message").await?;
	} else {
		let mut embeds = Vec::with_capacity(files.len());
		let mut attachments = Vec::with_capacity(files.len());
		let handle = ctx
			.reply(format!("Processed 0/{} scores", files.len()))
			.await?;

		for (i, file) in files.iter().enumerate() {
			if let Some(_) = file.dimensions() {
				// {{{ Image pre-processing
				let bytes = file.download().await?;
				let image = image::load_from_memory(&bytes)?;
				let mut image = image.resize(1024, 1024, FilterType::Nearest);
				// }}}
				// {{{ Detection
				// Create cropper and run OCR
				let mut cropper = ImageCropper::default();

				let edited = CreateReply::default()
					.reply(true)
					.content(format!("Image {}: reading jacket", i + 1));
				handle.edit(ctx, edited).await?;

				// This makes OCR more likely to work
				let mut ocr_image = image.grayscale().blur(1.);

				// {{{ Kind
				let edited = CreateReply::default()
					.reply(true)
					.content(format!("Image {}: reading kind", i + 1));
				handle.edit(ctx, edited).await?;

				let kind = match cropper.read_score_kind(&ocr_image) {
					// {{{ OCR error handling
					Err(err) => {
						error_with_image(
							ctx,
							&cropper.bytes,
							&file.filename,
							"Could not read kind from picture",
							&err,
						)
						.await?;

						continue;
					}
					// }}}
					Ok(k) => k,
				};
				// }}}
				// {{{ Difficulty
				let edited = CreateReply::default()
					.reply(true)
					.content(format!("Image {}: reading difficulty", i + 1));
				handle.edit(ctx, edited).await?;

				// Do not use `ocr_image` because this reads the colors
				let difficulty = match cropper.read_difficulty(&image, kind) {
					// {{{ OCR error handling
					Err(err) => {
						error_with_image(
							ctx,
							&cropper.bytes,
							&file.filename,
							"Could not read difficulty from picture",
							&err,
						)
						.await?;

						continue;
					}
					// }}}
					Ok(d) => d,
				};

				println!("{difficulty:?}");
				// }}}
				// {{{ Jacket & distribution
				let mut jacket_rect = None;
				let song_by_jacket = cropper
					.read_jacket(ctx.data(), &mut image, kind, difficulty, &mut jacket_rect)
					.await;
				let note_distribution = cropper.read_distribution(&image)?;
				// }}}
				ocr_image.invert();
				// {{{ Title
				let edited = CreateReply::default()
					.reply(true)
					.content(format!("Image {}: reading title", i + 1));
				handle.edit(ctx, edited).await?;

				let song_by_name = match kind {
					ScoreKind::SongSelect => None,
					ScoreKind::ScoreScreen => {
						Some(cropper.read_song(&ocr_image, &ctx.data().song_cache, difficulty))
					}
				};

				let (song, chart) = match (song_by_jacket, song_by_name) {
					// {{{ Only name succeeded
					(Err(err_jacket), Some(Ok(by_name))) => {
						println!("Could not recognise jacket with error: {}", err_jacket);
						by_name
					}
					// }}}
					// {{{ Both succeeded
					(Ok(by_jacket), Some(Ok(by_name))) => {
						if by_name.0.id != by_jacket.0.id {
							println!(
								"Got diverging choices between '{}' and '{}'",
								by_jacket.0.title, by_name.0.title
							);
						};

						by_jacket
					} // }}}
					// {{{ Only jacket succeeded
					(Ok(by_jacket), err_name) => {
						if let Some(err) = err_name {
							println!("Could not read name with error: {:?}", err.unwrap_err());
						}

						by_jacket
					}
					// }}}
					// {{{ Both errors
					(Err(err_jacket), err_name) => {
						if let Some(rect) = jacket_rect {
							cropper.crop_image_to_bytes(&image, rect)?;
							error_with_image(
							ctx,
							&cropper.bytes,
							&file.filename,
							"Hey! I could not read the score in the provided picture.",
							&format!(
                                "This can mean one of three things:
1. The image you provided is *not that of an Arcaea score
2. The image you provided contains a newly added chart that is not in my database yet
3. The image you provided contains character art that covers the chart name. When this happens, I try to make use of the jacket art in order to determine the chart. Contact `@prescientmoon` on discord to try and resolve the issue!

Nerdy info:
```
Jacket error: {}
Title error: {:?}
```" ,
								err_jacket, err_name
							),
						)
						.await?;
						} else {
							ctx.reply(format!(
								"This is a weird error that should never happen...
Nerdy info:
```
Jacket error: {}
Title error: {:?}
```",
								err_jacket, err_name
							))
							.await?;
						}
						continue;
					} // }}}
				};

				println!("{}", song.title);
				// }}}
				// {{{ Score
				let edited = CreateReply::default()
					.reply(true)
					.content(format!("Image {}: reading score", i + 1));
				handle.edit(ctx, edited).await?;

				let score_possibilities =
					match cropper.read_score(Some(chart.note_count), &ocr_image, kind) {
						// {{{ OCR error handling
						Err(err) => {
							error_with_image(
								ctx,
								&cropper.bytes,
								&file.filename,
								"Could not read score from picture",
								&err,
							)
							.await?;

							continue;
						}
						// }}}
						Ok(scores) => scores,
					};
				// }}}
				// {{{ Build play
				let (score, maybe_fars, score_warning) = Score::resolve_ambiguities(
					score_possibilities,
					Some(note_distribution),
					chart.note_count,
				)
				.map_err(|err| {
					format!(
						"Error occurred when disambiguating scores for '{}' [{:?}] by {}: {}",
						song.title, difficulty, song.artist, err
					)
				})?;
				println!(
					"Maybe fars {:?}, distribution {:?}",
					maybe_fars, note_distribution
				);
				let play = CreatePlay::new(score, &chart, &user)
					.with_attachment(file)
					.with_fars(maybe_fars)
					.save(&ctx.data())
					.await?;
				// }}}
				// }}}
				// {{{ Deliver embed
				let (mut embed, attachment) = play
					.to_embed(&ctx.data().db, &user, &song, &chart, i, None)
					.await?;
				if let Some(warning) = score_warning {
					embed = embed.description(warning);
				}

				embeds.push(embed);
				attachments.extend(attachment);
			// }}}
			} else {
				ctx.reply("One of the attached files is not an image!")
					.await?;
				continue;
			}

			let edited = CreateReply::default().reply(true).content(format!(
				"Processed {}/{} scores",
				i + 1,
				files.len()
			));

			handle.edit(ctx, edited).await?;
		}

		handle.delete(ctx).await?;

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
	let user = match User::from_context(&ctx).await {
		Ok(user) => user,
		Err(_) => {
			ctx.say("You are not an user in my database, sorry!")
				.await?;
			return Ok(());
		}
	};

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
