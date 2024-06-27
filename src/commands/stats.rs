use std::io::Cursor;

use chrono::{DateTime, NaiveDateTime};
use image::{ImageBuffer, Rgb};
use plotters::{
	backend::{BitMapBackend, PixelFormat, RGBPixel},
	chart::{ChartBuilder, LabelAreaPosition},
	drawing::IntoDrawingArea,
	element::Circle,
	series::LineSeries,
	style::{
		text_anchor::{HPos, Pos, VPos},
		Color, FontTransform, IntoFont, TextStyle, BLUE, WHITE,
	},
};
use poise::{
	serenity_prelude::{CreateAttachment, CreateMessage},
	CreateReply,
};
use sqlx::query_as;

use crate::{
	chart::Difficulty,
	context::{Context, Error},
	score::{guess_chart_name, DbPlay, Score},
	user::{discord_it_to_discord_user, User},
};

// {{{ Stats
/// Stats display
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("chart"),
	subcommand_required
)]
pub async fn stats(_ctx: Context<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Chart
/// Chart-related stats
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("best", "plot"),
	subcommand_required
)]
pub async fn chart(_ctx: Context<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Best score
/// Show the best score on a given chart
#[poise::command(prefix_command, slash_command)]
pub async fn best(
	ctx: Context<'_>,
	#[rest]
	#[description = "Name of chart to show (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let user = match User::from_context(&ctx).await {
		Ok(user) => user,
		Err(_) => {
			ctx.say("You are not an user in my database, sorry!")
				.await?;
			return Ok(());
		}
	};

	let name = name.trim();
	let (name, difficulty) = name
		.strip_suffix("PST")
		.zip(Some(Difficulty::PST))
		.or_else(|| name.strip_suffix("PRS").zip(Some(Difficulty::PRS)))
		.or_else(|| name.strip_suffix("FTR").zip(Some(Difficulty::FTR)))
		.or_else(|| name.strip_suffix("ETR").zip(Some(Difficulty::ETR)))
		.or_else(|| name.strip_suffix("BYD").zip(Some(Difficulty::BYD)))
		.unwrap_or((&name, Difficulty::FTR));

	let (song, chart) = guess_chart_name(name, &ctx.data().song_cache, difficulty).await?;

	let play = query_as!(
		DbPlay,
		"
            SELECT * FROM plays
            WHERE user_id=?
            AND chart_id=?
            ORDER BY score DESC
        ",
		user.id,
		chart.id
	)
	.fetch_one(&ctx.data().db)
	.await
	.map_err(|_| format!("Could not find any scores for chart"))?
	.to_play();

	let (embed, attachment) = play
		.to_embed(
			&song,
			&chart,
			0,
			Some(&discord_it_to_discord_user(&ctx, &user.discord_id).await?),
		)
		.await?;

	ctx.channel_id()
		.send_files(ctx.http(), attachment, CreateMessage::new().embed(embed))
		.await?;

	Ok(())
}
// }}}
//  Score plot
/// Show the best score on a given chart
#[poise::command(prefix_command, slash_command)]
pub async fn plot(
	ctx: Context<'_>,
	#[rest]
	#[description = "Name of chart to show (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let user = match User::from_context(&ctx).await {
		Ok(user) => user,
		Err(_) => {
			ctx.say("You are not an user in my database, sorry!")
				.await?;
			return Ok(());
		}
	};

	let name = name.trim();
	let (name, difficulty) = name
		.strip_suffix("PST")
		.zip(Some(Difficulty::PST))
		.or_else(|| name.strip_suffix("PRS").zip(Some(Difficulty::PRS)))
		.or_else(|| name.strip_suffix("FTR").zip(Some(Difficulty::FTR)))
		.or_else(|| name.strip_suffix("ETR").zip(Some(Difficulty::ETR)))
		.or_else(|| name.strip_suffix("BYD").zip(Some(Difficulty::BYD)))
		.unwrap_or((&name, Difficulty::FTR));

	let (song, chart) = guess_chart_name(name, &ctx.data().song_cache, difficulty).await?;

	let plays = query_as!(
		DbPlay,
		"
            SELECT * FROM plays
            WHERE user_id=?
            AND chart_id=?
            ORDER BY created_at ASC
        ",
		user.id,
		chart.id
	)
	.fetch_all(&ctx.data().db)
	.await?;

	if plays.len() == 0 {
		ctx.reply("No plays found").await?;
		return Ok(());
	}

	let min_time = plays.iter().map(|p| p.created_at).min().unwrap();
	let max_time = plays.iter().map(|p| p.created_at).max().unwrap();
	let mut min_score = plays.iter().map(|p| p.score).min().unwrap();

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
		let mut root = BitMapBackend::with_buffer(&mut buffer, (width, height)).into_drawing_area();

		let mut chart = ChartBuilder::on(&root)
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

		chart
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
			.iter()
			.map(|play| (play.created_at.and_utc().timestamp_millis(), play.score))
			.collect();

		points.sort();
		points.dedup();

		chart.draw_series(LineSeries::new(points.iter().map(|(t, s)| (*t, *s)), &BLUE))?;

		chart.draw_series(
			points
				.iter()
				.map(|(t, s)| Circle::new((*t, *s), 3, BLUE.filled())),
		)?;

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
//
