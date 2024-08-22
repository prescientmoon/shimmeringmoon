use poise::serenity_prelude::{CreateAttachment, CreateEmbed, CreateMessage};

use crate::{
	arcaea::{chart::Side, play::Play},
	context::{Context, Error},
	get_user,
	recognition::fuzzy_song_name::guess_song_and_chart,
};
use std::io::Cursor;

use chrono::DateTime;
use image::{ImageBuffer, Rgb};
use plotters::{
	backend::{BitMapBackend, PixelFormat, RGBPixel},
	chart::{ChartBuilder, LabelAreaPosition},
	drawing::IntoDrawingArea,
	element::Circle,
	series::LineSeries,
	style::{IntoFont, TextStyle, BLUE, WHITE},
};
use poise::CreateReply;

use crate::{
	arcaea::score::{Score, ScoringSystem},
	user::discord_id_to_discord_user,
};

// {{{ Top command
/// Chart-related stats
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
/// Show a chart given it's name
#[poise::command(prefix_command, slash_command, user_cooldown = 1)]
async fn info(
	ctx: Context<'_>,
	#[rest]
	#[description = "Name of chart to show (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let (song, chart) = guess_song_and_chart(&ctx.data(), &name)?;

	let attachement_name = "chart.png";
	let icon_attachement = match chart.cached_jacket.as_ref() {
		Some(jacket) => Some(CreateAttachment::bytes(jacket.raw, attachement_name)),
		None => None,
	};

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
		.query_row([chart.id], |row| row.get(0))?;

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

	ctx.channel_id()
		.send_files(
			ctx.http(),
			icon_attachement,
			CreateMessage::new().embed(embed),
		)
		.await?;

	Ok(())
}
// }}}
// {{{ Best score
/// Show the best score on a given chart
#[poise::command(prefix_command, slash_command, user_cooldown = 1)]
async fn best(
	ctx: Context<'_>,
	#[rest]
	#[description = "Name of chart to show (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let user = get_user!(&ctx);

	let (song, chart) = guess_song_and_chart(&ctx.data(), &name)?;
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
			format!(
				"Could not find any scores for {} [{:?}]",
				song.title, chart.difficulty
			)
		})?;

	let (embed, attachment) = play.to_embed(
		ctx.data(),
		&user,
		song,
		chart,
		0,
		Some(&discord_id_to_discord_user(&ctx, &user.discord_id).await?),
	)?;

	ctx.channel_id()
		.send_files(ctx.http(), attachment, CreateMessage::new().embed(embed))
		.await?;

	Ok(())
}
// }}}
// {{{ Score plot
/// Show the best score on a given chart
#[poise::command(prefix_command, slash_command, user_cooldown = 10)]
async fn plot(
	ctx: Context<'_>,
	scoring_system: Option<ScoringSystem>,
	#[rest]
	#[description = "Name of chart to show (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let user = get_user!(&ctx);
	let scoring_system = scoring_system.unwrap_or_default();

	let (song, chart) = guess_song_and_chart(&ctx.data(), &name)?;

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

	if plays.len() == 0 {
		ctx.reply(format!(
			"No plays found on {} [{:?}]",
			song.title, chart.difficulty
		))
		.await?;
		return Ok(());
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
		min_score = 9_800_000;
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

	let reply = CreateReply::default().attachment(CreateAttachment::bytes(buffer, "plot.png"));
	ctx.send(reply).await?;

	Ok(())
}
// }}}
