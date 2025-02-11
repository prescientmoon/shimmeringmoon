// {{{ Imports
use crate::arcaea::play::{CreatePlay, Play};
use crate::arcaea::score::Score;
use crate::context::{Error, ErrorKind, PoiseContext, TagError, TaggedError};
use crate::recognition::recognize::{ImageAnalyzer, ScoreKind};
use crate::user::User;
use crate::{get_user_error, timed};
use anyhow::anyhow;
use image::DynamicImage;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed};
use poise::{serenity_prelude as serenity, CreateReply};

use super::discord::{CreateReplyExtra, MessageContext};
// }}}

// {{{ Score
/// Score management
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("magic", "delete", "show"),
	subcommand_required
)]
pub async fn score(_ctx: PoiseContext<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Score magic
// {{{ Implementation
#[allow(clippy::too_many_arguments)]
async fn magic_detect_one<C: MessageContext>(
	ctx: &mut C,
	user: &User,
	embeds: &mut Vec<CreateEmbed>,
	attachments: &mut Vec<CreateAttachment>,
	plays: &mut Vec<Play>,
	analyzer: &mut ImageAnalyzer,
	attachment: &C::Attachment,
	index: usize,
	image: &mut DynamicImage,
	grayscale_image: &mut DynamicImage,
) -> Result<(), TaggedError> {
	// {{{ Detection
	let kind = timed!("read_score_kind", {
		analyzer.read_score_kind(ctx.data(), grayscale_image)?
	});

	let difficulty = timed!("read_difficulty", {
		analyzer.read_difficulty(ctx.data(), image, grayscale_image, kind)?
	});

	let (song, chart) = timed!("read_jacket", {
		analyzer.read_jacket(ctx.data(), image, kind, difficulty)?
	});

	let max_recall = match kind {
		ScoreKind::ScoreScreen => {
			// NOTE: are we ok with discarding errors like that?
			analyzer.read_max_recall(ctx.data(), grayscale_image).ok()
		}
		ScoreKind::SongSelect => None,
	};

	grayscale_image.invert();
	let note_distribution = match kind {
		ScoreKind::ScoreScreen => Some(analyzer.read_distribution(ctx.data(), grayscale_image)?),
		ScoreKind::SongSelect => None,
	};

	let score = timed!("read_score", {
		analyzer
			.read_score(ctx.data(), Some(chart.note_count), grayscale_image, kind)
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
		.save(ctx.data(), user, chart)
		.await?;
	// }}}
	// }}}
	// {{{ Deliver embed
	let (embed, attachment) = timed!("to embed", {
		play.to_embed(ctx.data(), user, song, chart, index, None)?
	});

	plays.push(play);
	embeds.push(embed);
	attachments.extend(attachment);
	// }}}

	Ok(())
}

pub async fn magic_impl<C: MessageContext>(
	ctx: &mut C,
	files: &[C::Attachment],
) -> Result<Vec<Play>, TaggedError> {
	let user = User::from_context(ctx)?;
	let files = ctx.download_images(files).await?;

	if files.is_empty() {
		return Err(anyhow!("No images found attached to message").tag(ErrorKind::User));
	}

	let mut embeds = Vec::with_capacity(files.len());
	let mut attachments = Vec::with_capacity(files.len());
	let mut plays = Vec::with_capacity(files.len());
	let mut analyzer = ImageAnalyzer::default();

	for (i, (attachment, bytes)) in files.into_iter().enumerate() {
		// {{{ Process attachment
		let mut image = image::load_from_memory(&bytes)?;
		let mut grayscale_image = DynamicImage::ImageLuma8(image.to_luma8());

		let result = magic_detect_one(
			ctx,
			&user,
			&mut embeds,
			&mut attachments,
			&mut plays,
			&mut analyzer,
			attachment,
			i,
			&mut image,
			&mut grayscale_image,
		)
		.await;

		if let Err(err) = result {
			let user_err = get_user_error!(err);
			analyzer
				.send_discord_error(ctx, &image, C::filename(attachment), user_err)
				.await?;
		}
		// }}}
	}

	if !embeds.is_empty() {
		ctx.send(
			CreateReply::default()
				.reply(true)
				.embeds(embeds)
				.attachments(attachments),
		)
		.await?;
	}

	Ok(plays)
}
// }}}
// {{{ Tests
#[cfg(test)]
mod magic_tests {

	use std::{path::PathBuf, str::FromStr};

	use crate::{
		arcaea::score::ScoringSystem,
		commands::discord::{mock::MockContext, play_song_title},
		golden_test, with_test_ctx,
	};

	use super::*;

	#[tokio::test]
	async fn no_pics() -> Result<(), Error> {
		with_test_ctx!("commands/score/magic/no_pics", |ctx| async move {
			magic_impl(ctx, &[]).await?;
			Ok(())
		})
	}

	golden_test!(simple_pic, "score/magic/single_pic");
	async fn simple_pic(ctx: &mut MockContext) -> Result<(), TaggedError> {
		let plays =
			magic_impl(ctx, &[PathBuf::from_str("test/screenshots/alter_ego.jpg")?]).await?;
		assert_eq!(plays.len(), 1);
		assert_eq!(plays[0].score(ScoringSystem::Standard).0, 9926250);
		assert_eq!(play_song_title(ctx, &plays[0])?, "ALTER EGO");
		Ok(())
	}

	golden_test!(weird_kerning, "score/magic/weird_kerning");
	async fn weird_kerning(ctx: &mut MockContext) -> Result<(), TaggedError> {
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
}
// }}}
// {{{ Discord wrapper
/// Identify scores from attached images.
#[poise::command(prefix_command, slash_command)]
pub async fn magic(
	mut ctx: PoiseContext<'_>,
	#[description = "Images containing scores"] files: Vec<serenity::Attachment>,
) -> Result<(), Error> {
	let res = magic_impl(&mut ctx, &files).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
// {{{ Score show
// {{{ Implementation
pub async fn show_impl<C: MessageContext>(
	ctx: &mut C,
	ids: &[u32],
) -> Result<Vec<Play>, TaggedError> {
	if ids.is_empty() {
		return Err(anyhow!("Empty ID list provided").tag(ErrorKind::User));
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
					CreateReply::default().content(format!("Could not find play with id {}", id)),
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

	if !embeds.is_empty() {
		ctx.send(
			CreateReply::default()
				.reply(true)
				.embeds(embeds)
				.attachments(attachments),
		)
		.await?;
	}

	Ok(plays)
}
/// }}}
// {{{ Tests
#[cfg(test)]
mod show_tests {
	use super::*;
	use crate::{commands::discord::mock::MockContext, golden_test, with_test_ctx};
	use std::{path::PathBuf, str::FromStr};

	#[tokio::test]
	async fn no_ids() -> Result<(), Error> {
		with_test_ctx!("commands/score/show/no_ids", |ctx| async move {
			show_impl(ctx, &[]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn nonexistent_id() -> Result<(), Error> {
		with_test_ctx!("commands/score/show/nonexistent_id", |ctx| async move {
			show_impl(ctx, &[666]).await?;
			Ok(())
		})
	}

	golden_test!(agrees_with_magic, "commands/score/show/agrees_with_magic");
	async fn agrees_with_magic(ctx: &mut MockContext) -> Result<(), TaggedError> {
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
}
// }}}
// {{{ Discord wrapper
/// Show scores given their IDs.
#[poise::command(prefix_command, slash_command)]
pub async fn show(
	mut ctx: PoiseContext<'_>,
	#[description = "Ids of score to show"] ids: Vec<u32>,
) -> Result<(), Error> {
	let res = show_impl(&mut ctx, &ids).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
// {{{ Score delete
// {{{ Implementation
pub async fn delete_impl<C: MessageContext>(ctx: &mut C, ids: &[u32]) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;

	if ids.is_empty() {
		return Err(anyhow!("Empty ID list provided").tag(ErrorKind::User));
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
		golden_test, with_test_ctx,
	};
	use std::{path::PathBuf, str::FromStr};

	#[tokio::test]
	async fn no_ids() -> Result<(), Error> {
		with_test_ctx!("commands/score/delete/no_ids", |ctx| async move {
			delete_impl(ctx, &[]).await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn nonexistent_id() -> Result<(), Error> {
		with_test_ctx!("commands/score/delete/nonexistent_id", |ctx| async move {
			delete_impl(ctx, &[666]).await?;
			Ok(())
		})
	}

	golden_test!(delete_twice, "commands/score/delete/delete_twice");
	async fn delete_twice(ctx: &mut MockContext) -> Result<(), TaggedError> {
		let plays =
			magic_impl(ctx, &[PathBuf::from_str("test/screenshots/alter_ego.jpg")?]).await?;

		let id = plays[0].id;
		delete_impl(ctx, &[id, id]).await?;
		Ok(())
	}

	golden_test!(
		no_show_after_delete,
		"commands/score/delete/no_show_after_delete"
	);
	async fn no_show_after_delete(ctx: &mut MockContext) -> Result<(), TaggedError> {
		let plays =
			magic_impl(ctx, &[PathBuf::from_str("test/screenshots/alter_ego.jpg")?]).await?;

		// Showcase proper usage
		let ids = [plays[0].id];
		delete_impl(ctx, &ids).await?;

		// This will tell the user the play doesn't exist
		let shown_plays = show_impl(ctx, &ids).await?;
		assert_eq!(shown_plays.len(), 0);

		Ok(())
	}

	golden_test!(delete_multiple, "commands/score/delete/delete_multiple");
	async fn delete_multiple(ctx: &mut MockContext) -> Result<(), TaggedError> {
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
}
// }}}
// {{{ Discord wrapper
/// Delete scores, given their IDs.
#[poise::command(prefix_command, slash_command)]
pub async fn delete(
	mut ctx: PoiseContext<'_>,
	#[description = "Id of score to delete"] ids: Vec<u32>,
) -> Result<(), Error> {
	let res = delete_impl(&mut ctx, &ids).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
