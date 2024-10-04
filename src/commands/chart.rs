// {{{ Imports
use anyhow::anyhow;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed};

use crate::arcaea::{chart::Side, play::Play};
use crate::context::{Context, Error, ErrorKind, TagError, TaggedError};
use crate::recognition::fuzzy_song_name::guess_song_and_chart;
use crate::user::User;
use std::io::Cursor;

use chrono::DateTime;
use image::{ImageBuffer, Rgb};
use plotters::backend::{BitMapBackend, PixelFormat, RGBPixel};
use plotters::chart::{ChartBuilder, LabelAreaPosition};
use plotters::drawing::IntoDrawingArea;
use plotters::element::Circle;
use plotters::series::LineSeries;
use plotters::style::{IntoFont, TextStyle, BLUE, WHITE};
use poise::CreateReply;

use crate::arcaea::score::{Score, ScoringSystem};

use super::discord::{CreateReplyExtra, MessageContext};
// }}}

// {{{ Top command
/// Chart-related utilities.
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("info", "best", "plot"),
	subcommand_required
)]
pub async fn chart(_ctx: Context<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Info
// {{{ Implementation
async fn info_impl(ctx: &mut impl MessageContext, name: &str) -> Result<(), TaggedError> {
	let (song, chart) = guess_song_and_chart(ctx.data(), name)?;

	let attachement_name = "chart.png";
	let icon_attachement = chart
		.cached_jacket
		.map(|jacket| CreateAttachment::bytes(jacket.raw, attachement_name));

	let play_count: usize = ctx
		.data()
		.db
		.get()?
		.prepare_cached(
			"
        SELECT COUNT(*) as count
        FROM plays
        WHERE chart_id=?
      ",
		)?
		.query_row([chart.id], |row| row.get(0))
		.unwrap_or(0);

	let mut embed = CreateEmbed::default()
		.title(format!(
			"{} [{:?} {}]",
			&song.title, chart.difficulty, chart.level
		))
		.field("Note count", format!("{}", chart.note_count), true)
		.field(
			"Chart constant",
			format!("{:.1}", chart.chart_constant as f32 / 100.0),
			true,
		)
		.field("Total plays", format!("{play_count}"), true)
		.field("BPM", &song.bpm, true)
		.field("Side", Side::SIDE_STRINGS[song.side.to_index()], true)
		.field("Artist", &song.title, true);

	if let Some(note_design) = &chart.note_design {
		embed = embed.field("Note design", note_design, true);
	}

	if let Some(pack) = &song.pack {
		embed = embed.field("Pack", pack, true);
	}

	if icon_attachement.is_some() {
		embed = embed.thumbnail(format!("attachment://{}", &attachement_name));
	}

	ctx.send(
		CreateReply::default()
			.reply(true)
			.embed(embed)
			.attachments(icon_attachement),
	)
	.await?;

	Ok(())
}
// }}}
// {{{ Tests
#[cfg(test)]
mod info_tests {
	use crate::{commands::discord::mock::MockContext, golden_test, with_test_ctx};

	use super::*;

	#[tokio::test]
	async fn no_suffix() -> Result<(), Error> {
		with_test_ctx!("commands/commands/chart/info/no_suffix", |ctx| async move {
			info_impl(ctx, "Pentiment").await?;
			Ok(())
		})
	}

	#[tokio::test]
	async fn specify_difficulty() -> Result<(), Error> {
		with_test_ctx!(
			"commands/commands/chart/info/specify_difficulty",
			|ctx| async move {
				info_impl(ctx, "Hellohell [ETR]").await?;
				Ok(())
			}
		)
	}

	golden_test!(last_byd, "commands/chart/info/last_byd");
	async fn last_byd(ctx: &mut MockContext) -> Result<(), TaggedError> {
		info_impl(ctx, "Last | Moment [BYD]").await?;
		info_impl(ctx, "Last | Eternity [BYD]").await?;
		Ok(())
	}
}
// }}}
// {{{ Discord wrapper
/// Show a chart given it's name
#[poise::command(prefix_command, slash_command, user_cooldown = 1)]
async fn info(
	mut ctx: Context<'_>,
	#[rest]
	#[description = "Name of chart (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let res = info_impl(&mut ctx, &name).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
// {{{ Best score
// {{{ Implementation
async fn best_impl<C: MessageContext>(ctx: &mut C, name: &str) -> Result<Play, TaggedError> {
	let user = User::from_context(ctx)?;

	let (song, chart) = guess_song_and_chart(ctx.data(), name)?;
	let play = ctx
		.data()
		.db
		.get()?
		.prepare_cached(
			"
        SELECT 
        p.id, p.chart_id, p.user_id, p.created_at,
        p.max_recall, p.far_notes, s.score
        FROM plays p
        JOIN scores s ON s.play_id = p.id
        WHERE s.scoring_system='standard'
        AND p.user_id=?
        AND p.chart_id=?
        ORDER BY s.score DESC
        LIMIT 1
      ",
		)?
		.query_row((user.id, chart.id), |row| Play::from_sql(chart, row))
		.map_err(|_| {
			anyhow!(
				"Could not find any scores for {} [{:?}]",
				song.title,
				chart.difficulty
			)
			.tag(ErrorKind::User)
		})?;

	let (embed, attachment) = play.to_embed(
		ctx.data(),
		&user,
		song,
		chart,
		0,
		Some(&ctx.fetch_user(&user.discord_id).await?),
	)?;

	ctx.send(
		CreateReply::default()
			.reply(true)
			.embed(embed)
			.attachments(attachment),
	)
	.await?;

	Ok(play)
}
// }}}
// {{{ Tests
// {{{ Tests
#[cfg(test)]
mod best_tests {
	use std::{path::PathBuf, str::FromStr};

	use crate::{
		commands::{discord::mock::MockContext, score::magic_impl},
		golden_test, with_test_ctx,
	};

	use super::*;

	#[tokio::test]
	async fn no_scores() -> Result<(), Error> {
		with_test_ctx!("commands/chart/best/no_scores", |ctx| async move {
			best_impl(ctx, "Pentiment").await?;
			Ok(())
		})
	}

	golden_test!(pick_correct_score, "commands/chart/best/pick_correct_score");
	async fn pick_correct_score(ctx: &mut MockContext) -> Result<(), TaggedError> {
		let plays = magic_impl(
			ctx,
			&[
				PathBuf::from_str("test/screenshots/fracture_ray_ex.jpg")?,
				// Make sure we aren't considering higher scores from other stuff
				PathBuf::from_str("test/screenshots/antithese_74_kerning.jpg")?,
				PathBuf::from_str("test/screenshots/fracture_ray_missed_ex.jpg")?,
			],
		)
		.await?;

		let play = best_impl(ctx, "Fracture ray").await?;
		assert_eq!(play.score(ScoringSystem::Standard).0, 9_805_651);
		assert_eq!(plays[0], play);

		Ok(())
	}
}
// }}}
// }}}
// {{{ Discord wrapper
/// Show the best score on a given chart
#[poise::command(prefix_command, slash_command, user_cooldown = 1)]
async fn best(
	mut ctx: Context<'_>,
	#[rest]
	#[description = "Name of chart (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let res = best_impl(&mut ctx, &name).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
// {{{ Score plot
// {{{ Implementation
async fn plot_impl<C: MessageContext>(
	ctx: &mut C,
	scoring_system: Option<ScoringSystem>,
	name: String,
) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;
	let scoring_system = scoring_system.unwrap_or_default();

	let (song, chart) = guess_song_and_chart(ctx.data(), &name)?;

	// SAFETY: we limit the amount of plotted plays to 1000.
	let plays = ctx
		.data()
		.db
		.get()?
		.prepare_cached(
			"
      SELECT 
        p.id, p.chart_id, p.user_id, p.created_at,
        p.max_recall, p.far_notes, s.score
      FROM plays p
      JOIN scores s ON s.play_id = p.id
      WHERE s.scoring_system='standard'
      AND p.user_id=?
      AND p.chart_id=?
      ORDER BY s.score DESC
      LIMIT 1000
    ",
		)?
		.query_map((user.id, chart.id), |row| Play::from_sql(chart, row))?
		.collect::<Result<Vec<_>, _>>()?;

	if plays.is_empty() {
		return Err(
			anyhow!("No plays found on {} [{:?}]", song.title, chart.difficulty)
				.tag(ErrorKind::User),
		);
	}

	let min_time = plays.iter().map(|p| p.created_at).min().unwrap();
	let max_time = plays.iter().map(|p| p.created_at).max().unwrap();
	let mut min_score = plays
		.iter()
		.map(|p| p.score(scoring_system))
		.min()
		.unwrap()
		.0 as i64;

	if min_score > 9_900_000 {
		min_score = 9_900_000;
	} else if min_score > 9_800_000 {
		min_score = 9_800_000;
	} else if min_score > 9_500_000 {
		min_score = 9_500_000;
	} else {
		min_score = 9_000_000
	};

	let max_score = 10_010_000;
	let width = 1024;
	let height = 768;

	let mut buffer = vec![u8::MAX; RGBPixel::PIXEL_SIZE * (width * height) as usize];

	{
		let root = BitMapBackend::with_buffer(&mut buffer, (width, height)).into_drawing_area();

		let mut chart_buider = ChartBuilder::on(&root)
			.margin(25)
			.caption(
				format!("{} [{:?}]", song.title, chart.difficulty),
				("sans-serif", 40),
			)
			.set_label_area_size(LabelAreaPosition::Left, 100)
			.set_label_area_size(LabelAreaPosition::Bottom, 40)
			.build_cartesian_2d(
				min_time.and_utc().timestamp_millis()..max_time.and_utc().timestamp_millis(),
				min_score..max_score,
			)?;

		chart_buider
			.configure_mesh()
			.light_line_style(WHITE)
			.y_label_formatter(&|s| format!("{}", Score(*s as u32)))
			.y_desc("Score")
			.x_label_formatter(&|d| {
				format!(
					"{}",
					DateTime::from_timestamp_millis(*d).unwrap().date_naive()
				)
			})
			.y_label_style(TextStyle::from(("sans-serif", 20).into_font()))
			.x_label_style(TextStyle::from(("sans-serif", 20).into_font()))
			.draw()?;

		let mut points: Vec<_> = plays
			.into_iter()
			.map(|play| {
				(
					play.created_at.and_utc().timestamp_millis(),
					play.score(scoring_system),
				)
			})
			.collect();

		points.sort();
		points.dedup();

		chart_buider.draw_series(LineSeries::new(
			points.iter().map(|(t, s)| (*t, s.0 as i64)),
			&BLUE,
		))?;

		chart_buider.draw_series(points.iter().map(|(t, s)| {
			Circle::new((*t, s.0 as i64), 3, plotters::style::Color::filled(&BLUE))
		}))?;
		root.present()?;
	}

	let image: ImageBuffer<Rgb<u8>, _> = ImageBuffer::from_raw(width, height, buffer).unwrap();

	let mut buffer = Vec::new();
	let mut cursor = Cursor::new(&mut buffer);
	image.write_to(&mut cursor, image::ImageFormat::Png)?;

	let reply = CreateReply::default()
		.reply(true)
		.attachment(CreateAttachment::bytes(buffer, "plot.png"));
	ctx.send(reply).await?;

	Ok(())
}
// }}}
// {{{ Discord wrapper
/// Show the best score on a given chart
#[poise::command(prefix_command, slash_command, user_cooldown = 10)]
async fn plot(
	mut ctx: Context<'_>,
	scoring_system: Option<ScoringSystem>,
	#[rest]
	#[description = "Name of chart (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let res = plot_impl(&mut ctx, scoring_system, name).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
