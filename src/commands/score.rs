use crate::arcaea::play::{CreatePlay, Play};
use crate::arcaea::score::Score;
use crate::context::{Context, Error};
use crate::recognition::recognize::{ImageAnalyzer, ScoreKind};
use crate::user::User;
use crate::{get_user, timed};
use anyhow::anyhow;
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
pub async fn magic_impl<C: MessageContext>(
	ctx: &mut C,
	files: &[C::Attachment],
) -> Result<Vec<Play>, Error> {
	let user = get_user!(ctx);
	let files = ctx.download_images(&files).await?;

	if files.len() == 0 {
		ctx.reply("No images found attached to message").await?;
		return Ok(vec![]);
	}

	let mut embeds = Vec::with_capacity(files.len());
	let mut attachments = Vec::with_capacity(files.len());
	let mut plays = Vec::with_capacity(files.len());
	let mut analyzer = ImageAnalyzer::default();

	for (i, (attachment, bytes)) in files.into_iter().enumerate() {
		// {{{ Preapare image
		let mut image = image::load_from_memory(&bytes)?;
		let mut grayscale_image = DynamicImage::ImageLuma8(image.to_luma8());
		// }}}

		let result: Result<(), Error> = try {
			// {{{ Detection

			let kind = timed!("read_score_kind", {
				analyzer.read_score_kind(ctx.data(), &grayscale_image)?
			});

			// Do not use `ocr_image` because this reads the colors
			let difficulty = timed!("read_difficulty", {
				analyzer.read_difficulty(ctx.data(), &image, &grayscale_image, kind)?
			});

			let (song, chart) = timed!("read_jacket", {
				analyzer.read_jacket(ctx.data(), &mut image, kind, difficulty)?
			});

			let max_recall = match kind {
				ScoreKind::ScoreScreen => {
					// NOTE: are we ok with discarding errors like that?
					analyzer.read_max_recall(ctx.data(), &grayscale_image).ok()
				}
				ScoreKind::SongSelect => None,
			};

			grayscale_image.invert();
			let note_distribution = match kind {
				ScoreKind::ScoreScreen => {
					Some(analyzer.read_distribution(ctx.data(), &grayscale_image)?)
				}
				ScoreKind::SongSelect => None,
			};

			let score = timed!("read_score", {
				analyzer
					.read_score(ctx.data(), Some(chart.note_count), &grayscale_image, kind)
					.map_err(|err| {
						anyhow!(
							"Could not read score for chart {} [{:?}]: {err}",
							song.title,
							chart.difficulty
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

			plays.push(play);
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

	Ok(plays)
}
// }}}
// {{{ Tests
#[cfg(test)]
mod magic_tests {

	use std::path::PathBuf;

	use crate::{
		arcaea::score::ScoringSystem,
		commands::discord::{mock::MockContext, play_song_title},
		with_test_ctx,
	};

	use super::*;

	#[tokio::test]
	async fn no_pics() -> Result<(), Error> {
		with_test_ctx!("test/commands/score/magic/no_pics", async |ctx| {
			magic_impl(ctx, &[]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn simple_pic() -> Result<(), Error> {
		with_test_ctx!(
			"test/commands/score/magic/single_pic",
			async |ctx: &mut MockContext| {
				let plays =
					magic_impl(ctx, &[PathBuf::from_str("test/screenshots/alter_ego.jpg")?])
						.await?;
				assert_eq!(plays.len(), 1);
				assert_eq!(plays[0].score(ScoringSystem::Standard).0, 9926250);
				assert_eq!(play_song_title(ctx, &plays[0])?, "ALTER EGO");
				Ok(())
			}
		)
	}

	#[tokio::test]
	async fn weird_kerning() -> Result<(), Error> {
		with_test_ctx!(
			"test/commands/score/magic/weird_kerning",
			async |ctx: &mut MockContext| {
				let plays = magic_impl(
					ctx,
					&[
						PathBuf::from_str("test/screenshots/antithese_74_kerning.jpg")?,
						PathBuf::from_str("test/screenshots/genocider_24_kerning.jpg")?,
					],
				)
				.await?;

				assert_eq!(plays.len(), 2);
				assert_eq!(plays[0].score(ScoringSystem::Standard).0, 9983744);
				assert_eq!(play_song_title(ctx, &plays[0])?, "Antithese");
				assert_eq!(plays[1].score(ScoringSystem::Standard).0, 9724775);
				assert_eq!(play_song_title(ctx, &plays[1])?, "GENOCIDER");

				Ok(())
			}
		)
	}
}
// }}}
// {{{ Discord wrapper
/// Identify scores from attached images.
#[poise::command(prefix_command, slash_command)]
pub async fn magic(
	mut ctx: Context<'_>,
	#[description = "Images containing scores"] files: Vec<serenity::Attachment>,
) -> Result<(), Error> {
	magic_impl(&mut ctx, &files).await?;

	Ok(())
}
// }}}
// }}}
// {{{ Score show
// {{{ Implementation
pub async fn show_impl<C: MessageContext>(ctx: &mut C, ids: &[u32]) -> Result<Vec<Play>, Error> {
	if ids.len() == 0 {
		ctx.reply("Empty ID list provided").await?;
		return Ok(vec![]);
	}

	let mut embeds = Vec::with_capacity(ids.len());
	let mut attachments = Vec::with_capacity(ids.len());
	let mut plays = Vec::with_capacity(ids.len());
	let conn = ctx.data().db.get()?;
	for (i, id) in ids.iter().enumerate() {
		let result = conn
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
			.next();

		let (song, chart, play, discord_id) = match result {
			None => {
				ctx.send(
					CreateMessage::new().content(format!("Could not find play with id {}", id)),
				)
				.await?;
				continue;
			}
			Some(result) => result?,
		};

		let author = ctx.fetch_user(&discord_id).await?;
		let user = User::by_id(ctx.data(), play.user_id)?;

		let (embed, attachment) =
			play.to_embed(ctx.data(), &user, song, chart, i, Some(&author))?;

		embeds.push(embed);
		attachments.extend(attachment);
		plays.push(play);
	}

	if embeds.len() > 0 {
		ctx.send_files(attachments, CreateMessage::new().embeds(embeds))
			.await?;
	}

	Ok(plays)
}
/// }}}
// {{{ Tests
#[cfg(test)]
mod show_tests {
	use super::*;
	use crate::{commands::discord::mock::MockContext, with_test_ctx};
	use std::path::PathBuf;

	#[tokio::test]
	async fn no_ids() -> Result<(), Error> {
		with_test_ctx!("test/commands/score/show/no_ids", async |ctx| {
			show_impl(ctx, &[]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn nonexistent_id() -> Result<(), Error> {
		with_test_ctx!("test/commands/score/show/nonexistent_id", async |ctx| {
			show_impl(ctx, &[666]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn agrees_with_magic() -> Result<(), Error> {
		with_test_ctx!(
			"test/commands/score/show/agrees_with_magic",
			async |ctx: &mut MockContext| {
				let created_plays = magic_impl(
					ctx,
					&[
						PathBuf::from_str("test/screenshots/alter_ego.jpg")?,
						PathBuf::from_str("test/screenshots/antithese_74_kerning.jpg")?,
						PathBuf::from_str("test/screenshots/genocider_24_kerning.jpg")?,
					],
				)
				.await?;

				let ids = created_plays.iter().map(|p| p.id).collect::<Vec<_>>();
				let plays = show_impl(ctx, &ids).await?;

				assert_eq!(plays.len(), 3);
				assert_eq!(created_plays, plays);
				Ok(())
			}
		)
	}
}
// }}}
// {{{ Discord wrapper
/// Show scores given their ides
#[poise::command(prefix_command, slash_command)]
pub async fn show(
	mut ctx: Context<'_>,
	#[description = "Ids of score to show"] ids: Vec<u32>,
) -> Result<(), Error> {
	show_impl(&mut ctx, &ids).await?;

	Ok(())
}
// }}}
// }}}
// {{{ Score delete
// {{{ Implementation
pub async fn delete_impl<C: MessageContext>(ctx: &mut C, ids: &[u32]) -> Result<(), Error> {
	let user = get_user!(ctx);

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
			ctx.reply(&format!("No play with id {} found", id)).await?;
		} else {
			count += 1;
		}
	}

	if count > 0 {
		ctx.reply(&format!("Deleted {} play(s) successfully!", count))
			.await?;
	}

	Ok(())
}
/// }}}
// {{{ Tests
#[cfg(test)]
mod delete_tests {
	use super::*;
	use crate::{
		commands::discord::{mock::MockContext, play_song_title},
		with_test_ctx,
	};
	use std::path::PathBuf;

	#[tokio::test]
	async fn no_ids() -> Result<(), Error> {
		with_test_ctx!("test/commands/score/delete/no_ids", async |ctx| {
			delete_impl(ctx, &[]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn nonexistent_id() -> Result<(), Error> {
		with_test_ctx!("test/commands/score/delete/nonexistent_id", async |ctx| {
			delete_impl(ctx, &[666]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn delete_twice() -> Result<(), Error> {
		with_test_ctx!(
			"test/commands/score/delete/delete_twice",
			async |ctx: &mut MockContext| {
				let plays =
					magic_impl(ctx, &[PathBuf::from_str("test/screenshots/alter_ego.jpg")?])
						.await?;

				let id = plays[0].id;
				delete_impl(ctx, &[id, id]).await?;
				Ok(())
			}
		)
	}

	#[tokio::test]
	async fn no_show_after_delete() -> Result<(), Error> {
		with_test_ctx!(
			"test/commands/score/delete/no_show_after_delete",
			async |ctx: &mut MockContext| {
				let plays =
					magic_impl(ctx, &[PathBuf::from_str("test/screenshots/alter_ego.jpg")?])
						.await?;

				// Showcase proper usage
				let ids = [plays[0].id];
				delete_impl(ctx, &ids).await?;

				// This will tell the user the play doesn't exist
				let shown_plays = show_impl(ctx, &ids).await?;
				assert_eq!(shown_plays.len(), 0);

				Ok(())
			}
		)
	}

	#[tokio::test]
	async fn delete_multiple() -> Result<(), Error> {
		with_test_ctx!(
			"test/commands/score/delete/delete_multiple",
			async |ctx: &mut MockContext| {
				let plays = magic_impl(
					ctx,
					&[
						PathBuf::from_str("test/screenshots/antithese_74_kerning.jpg")?,
						PathBuf::from_str("test/screenshots/alter_ego.jpg")?,
						PathBuf::from_str("test/screenshots/genocider_24_kerning.jpg")?,
					],
				)
				.await?;

				delete_impl(ctx, &[plays[0].id, plays[2].id]).await?;

				// Ensure the second play still exists
				let shown_plays = show_impl(ctx, &[plays[1].id]).await?;
				assert_eq!(play_song_title(ctx, &shown_plays[0])?, "ALTER EGO");

				Ok(())
			}
		)
	}
}
// }}}
// {{{ Discord wrapper
/// Delete scores, given their IDs.
#[poise::command(prefix_command, slash_command)]
pub async fn delete(
	mut ctx: Context<'_>,
	#[description = "Id of score to delete"] ids: Vec<u32>,
) -> Result<(), Error> {
	delete_impl(&mut ctx, &ids).await?;

	Ok(())
}
// }}}
// }}}
