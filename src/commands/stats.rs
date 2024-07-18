use std::io::Cursor;

use chrono::DateTime;
use image::{ImageBuffer, Rgb};
use plotters::{
	backend::{BitMapBackend, PixelFormat, RGBPixel},
	chart::{ChartBuilder, LabelAreaPosition},
	drawing::IntoDrawingArea,
	element::Circle,
	series::LineSeries,
	style::{Color, IntoFont, TextStyle, BLUE, WHITE},
};
use poise::{
	serenity_prelude::{CreateAttachment, CreateMessage},
	CreateReply,
};
use sqlx::query_as;

use crate::{
	bitmap::{BitmapCanvas, LayoutDrawer, LayoutManager},
	chart::{Chart, Song},
	context::{Context, Error},
	jacket::BITMAP_IMAGE_SIZE,
	score::{guess_song_and_chart, DbPlay, Play, Score},
	user::{discord_it_to_discord_user, User},
};

// {{{ Stats
/// Stats display
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("chart", "b30"),
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

	let (song, chart) = guess_song_and_chart(&ctx.data(), &name)?;

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
	.map_err(|_| {
		format!(
			"Could not find any scores for {} [{:?}]",
			song.title, chart.difficulty
		)
	})?
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
// {{{ Score plot
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

	let (song, chart) = guess_song_and_chart(&ctx.data(), &name)?;

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
		ctx.reply(format!(
			"No plays found on {} [{:?}]",
			song.title, chart.difficulty
		))
		.await?;
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
		let root = BitMapBackend::with_buffer(&mut buffer, (width, height)).into_drawing_area();

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
// }}}
// {{{ B30
/// Show the 30 best scores
#[poise::command(prefix_command, slash_command)]
pub async fn b30(ctx: Context<'_>) -> Result<(), Error> {
	let user = match User::from_context(&ctx).await {
		Ok(user) => user,
		Err(_) => {
			ctx.say("You are not an user in my database, sorry!")
				.await?;
			return Ok(());
		}
	};

	let plays: Vec<DbPlay> = query_as(
		"
        SELECT id, chart_id, user_id,
        created_at, MAX(score) as score, zeta_score,
        creation_ptt, creation_zeta_ptt, far_notes, max_recall, discord_attachment_id
        FROM plays p
        WHERE user_id = ?
        GROUP BY chart_id
        ORDER BY score DESC
    ",
	)
	.bind(user.id)
	.fetch_all(&ctx.data().db)
	.await?;

	if plays.len() < 30 {
		ctx.reply("Not enough plays found").await?;
		return Ok(());
	}

	// TODO: consider not reallocating everything here
	let mut plays: Vec<(Play, &Song, &Chart)> = plays
		.into_iter()
		.map(|play| {
			let play = play.to_play();
			// TODO: change the .lookup to perform binary search or something
			let (song, chart) = ctx.data().song_cache.lookup_chart(play.chart_id)?;
			Ok((play, song, chart))
		})
		.collect::<Result<Vec<_>, Error>>()?;

	plays.sort_by_key(|(play, _, chart)| -play.score.play_rating(chart.chart_constant));
	plays.truncate(30);

	let mut layout = LayoutManager::default();
	let jacket_area = layout.make_box(BITMAP_IMAGE_SIZE, BITMAP_IMAGE_SIZE);
	let jacket_margin = 10;
	let jacket_with_margin =
		layout.margin(jacket_area, jacket_margin, jacket_margin, 5, jacket_margin);
	let top_left_area = layout.make_box(90, layout.height(jacket_with_margin));
	let top_area = layout.glue_vertically(top_left_area, jacket_with_margin);
	let bottom_area = layout.make_box(layout.width(top_area), 40);
	let item_area = layout.glue_horizontally(top_area, bottom_area);
	let item_with_margin = layout.margin_xy(item_area, 25, 20);
	let (item_grid, item_origins) = layout.repeated_evenly(item_with_margin, (5, 6));
	let root = item_grid;

	// layout.normalize(root);
	let width = layout.width(root);
	let height = layout.height(root);

	let canvas = BitmapCanvas::new(width, height);
	let mut drawer = LayoutDrawer::new(layout, canvas);

	let asset_cache = &ctx.data().jacket_cache;
	let bg = &asset_cache.b30_background;

	drawer.blit_rbg(
		root,
		(
			-((bg.width() - width) as i32) / 2,
			-((bg.height() - height) as i32) / 2,
		),
		bg.dimensions(),
		bg.as_raw(),
	);

	for (i, origin) in item_origins.enumerate() {
		drawer
			.layout
			.edit_to_relative(item_with_margin, item_grid, origin.0, origin.1);

		drawer.fill(top_area, (59, 78, 102, 255));

		let (_play, song, chart) = &plays[i];

		// {{{ Display jacket
		let jacket = chart.cached_jacket.as_ref().ok_or_else(|| {
			format!(
				"Cannot find jacket for chart {} [{:?}]",
				song.title, chart.difficulty
			)
		})?;

		drawer.blit_rbg(
			jacket_area,
			(0, 0),
			jacket.bitmap.dimensions(),
			&jacket.bitmap.as_raw(),
		);
		// }}}
		// {{{ Display difficulty background
		let diff_bg = &asset_cache.diff_backgrounds[chart.difficulty.to_index()];
		drawer.blit_rbga(
			jacket_area,
			(
				BITMAP_IMAGE_SIZE as i32 - (diff_bg.width() as i32) / 2,
				-(diff_bg.height() as i32) / 2,
			),
			diff_bg.dimensions(),
			&diff_bg.as_raw(),
		);
		// }}}
		// {{{ Display difficulty text
		let x_offset = if chart.level.ends_with("+") {
			3
		} else if chart.level == "11" {
			-2
		} else {
			0
		};
		// jacket_area.draw_text(
		// 	&chart.level,
		// 	&TextStyle::from(("Exo", 30).into_font())
		// 		.color(&WHITE)
		// 		.with_anchor::<RGBAColor>(Pos {
		// 			h_pos: HPos::Center,
		// 			v_pos: VPos::Center,
		// 		})
		// 		.into_text_style(&jacket_area),
		// 	(BITMAP_IMAGE_SIZE as i32 + x_offset, 2),
		// )?;
		// }}}
		// {{{ Display chart name
		// Draw background
		drawer.fill(bottom_area, (0x82, 0x71, 0xA7, 255));

		let tx = 10;
		let ty = drawer.layout.height(bottom_area) as i32 / 2;

		// let text = &song.title;
		// let mut size = 30;
		// let mut text_style = TextStyle::from(("Exo", size).into_font().style(FontStyle::Bold))
		// 	.with_anchor::<RGBAColor>(Pos {
		// 		h_pos: HPos::Left,
		// 		v_pos: VPos::Center,
		// 	})
		// 	.into_text_style(&bottom_area);
		//
		// while text_style.font.layout_box(text).unwrap().1 .0 >= item_area.0 as i32 - 20 {
		// 	size -= 3;
		// 	text_style.font = ("Exo", size).into_font();
		// }
		//
		// Draw drop shadow
		// bottom_area.draw_text(
		// 	&song.title,
		// 	&text_style.color(&RGBAColor(0, 0, 0, 0.2)),
		// 	(tx + 3, ty + 3),
		// )?;
		// bottom_area.draw_text(
		// 	&song.title,
		// 	&text_style.color(&RGBAColor(0, 0, 0, 0.2)),
		// 	(tx - 3, ty + 3),
		// )?;
		// bottom_area.draw_text(
		// 	&song.title,
		// 	&text_style.color(&RGBAColor(0, 0, 0, 0.2)),
		// 	(tx + 3, ty - 3),
		// )?;
		// bottom_area.draw_text(
		// 	&song.title,
		// 	&text_style.color(&RGBAColor(0, 0, 0, 0.2)),
		// 	(tx - 3, ty - 3),
		// )?;

		// Draw text
		// bottom_area.draw_text(&song.title, &text_style.color(&WHITE), (tx, ty))?;
		// }}}
		// {{{ Display index
		let bg = &asset_cache.count_background;

		// Draw background
		drawer.blit_rbga(item_area, (-8, jacket_margin as i32), bg.dimensions(), bg);

		// let text_style = TextStyle::from(("Exo", 30).into_font().style(FontStyle::Bold))
		// 	.with_anchor::<RGBAColor>(Pos {
		// 		h_pos: HPos::Left,
		// 		v_pos: VPos::Center,
		// 	})
		// 	.into_text_style(&area);

		let tx = 7;
		let ty = (jacket_margin + bg.height() as i32 / 2) - 3;

		// Draw drop shadow
		// area.draw_text(
		// 	&format!("#{}", i + 1),
		// 	&text_style.color(&BLACK),
		// 	(tx + 2, ty + 2),
		// )?;

		// Draw main text
		// area.draw_text(&format!("#{}", i + 1), &text_style.color(&WHITE), (tx, ty))?;
		// }}}
	}

	let mut out_buffer = Vec::new();
	let image: ImageBuffer<Rgb<u8>, _> =
		ImageBuffer::from_raw(width, height, drawer.canvas.buffer).unwrap();

	let mut cursor = Cursor::new(&mut out_buffer);
	image.write_to(&mut cursor, image::ImageFormat::Png)?;

	let reply = CreateReply::default().attachment(CreateAttachment::bytes(out_buffer, "b30.png"));
	ctx.send(reply).await?;

	Ok(())
}
// }}}
