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
use poise::{
	serenity_prelude::{CreateAttachment, CreateMessage},
	CreateReply,
};
use sqlx::query_as;

use crate::{
	arcaea::{
		jacket::BITMAP_IMAGE_SIZE,
		play::{compute_b30_ptt, get_b30_plays, DbPlay},
		score::Score,
	},
	assets::{
		get_b30_background, get_count_background, get_difficulty_background, get_grade_background,
		get_name_backgound, get_ptt_emblem, get_score_background, get_status_background,
		get_top_backgound, EXO_FONT,
	},
	bitmap::{Align, BitmapCanvas, Color, LayoutDrawer, LayoutManager, Rect},
	context::{Context, Error},
	get_user,
	recognition::fuzzy_song_name::guess_song_and_chart,
	reply_errors,
	user::discord_it_to_discord_user,
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
	let user = get_user!(&ctx);

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
			&ctx.data().db,
			&user,
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
	let user = get_user!(&ctx);

	let (song, chart) = guess_song_and_chart(&ctx.data(), &name)?;

	// SAFETY: we limit the amount of plotted plays to 1000.
	let plays = query_as!(
		DbPlay,
		"
      SELECT * FROM plays
      WHERE user_id=?
      AND chart_id=?
      ORDER BY created_at ASC
      LIMIT 1000
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
				.map(|(t, s)| Circle::new((*t, *s), 3, plotters::style::Color::filled(&BLUE))),
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
	let user = get_user!(&ctx);
	let user_ctx = ctx.data();
	let plays = reply_errors!(
		ctx,
		get_b30_plays(&user_ctx.db, &user_ctx.song_cache, &user).await?
	);

	// {{{ Layout
	let mut layout = LayoutManager::default();
	let jacket_area = layout.make_box(BITMAP_IMAGE_SIZE, BITMAP_IMAGE_SIZE);
	let jacket_with_border = layout.margin_uniform(jacket_area, 3);
	let jacket_margin = 10;
	let jacket_with_margin = layout.margin(
		jacket_with_border,
		jacket_margin,
		jacket_margin,
		2,
		jacket_margin,
	);
	let top_left_area = layout.make_box(90, layout.height(jacket_with_margin));
	let top_area = layout.glue_vertically(top_left_area, jacket_with_margin);
	let bottom_area = layout.make_box(layout.width(top_area), 43);
	let bottom_in_area = layout.margin_xy(bottom_area, -20, -7);
	let item_area = layout.glue_horizontally(top_area, bottom_area);
	let item_with_margin = layout.margin_xy(item_area, 22, 17);
	let (item_grid, item_origins) = layout.repeated_evenly(item_with_margin, (5, 6));
	let root = layout.margin_uniform(item_grid, 30);
	// }}}
	// {{{ Rendering prep
	let width = layout.width(root);
	let height = layout.height(root);

	let canvas = BitmapCanvas::new(width, height);
	let mut drawer = LayoutDrawer::new(layout, canvas);
	// }}}
	// {{{ Render background
	let bg = get_b30_background();

	drawer.blit_rbg(
		root,
		// Align the center of the image with the center of the root
		Rect::from_image(bg).align(
			(Align::Center, Align::Center),
			drawer.layout.lookup(root).center(),
		),
		bg.dimensions(),
		bg.as_raw(),
	);
	// }}}

	for (i, origin) in item_origins.enumerate() {
		drawer
			.layout
			.edit_to_relative(item_with_margin, item_grid, origin.0, origin.1);

		let top_bg = get_top_backgound();
		drawer.blit_rbg(top_area, (0, 0), top_bg.dimensions(), top_bg);

		let (play, song, chart) = &plays[i];

		// {{{ Display index
		let bg = get_count_background();
		let bg_center = Rect::from_image(bg).center();

		// Draw background
		drawer.blit_rbga(item_area, (-8, jacket_margin as i32), bg.dimensions(), bg);

		EXO_FONT.with_borrow_mut(|font| {
			drawer.text(
				item_area,
				(bg_center.0 - 12, bg_center.1 - 3 + jacket_margin),
				font,
				crate::bitmap::TextStyle {
					size: 25,
					weight: 800,
					color: Color::WHITE,
					align: (Align::Center, Align::Center),
					stroke: None,
					drop_shadow: Some((Color::BLACK.alpha(0xaa), (2, 2))),
				},
				&format!("#{}", i + 1),
			)
		})?;
		// }}}
		// {{{ Display chart name
		// Draw background
		let bg = get_name_backgound();
		drawer.blit_rbg(bottom_area, (0, 0), bg.dimensions(), bg.as_raw());

		// Draw text
		EXO_FONT.with_borrow_mut(|font| {
			let initial_size = 24;
			let mut style = crate::bitmap::TextStyle {
				size: initial_size,
				weight: 800,
				color: Color::WHITE,
				align: (Align::Start, Align::Center),
				stroke: Some((Color::BLACK, 1.5)),
				drop_shadow: None,
			};

			while drawer
				.canvas
				.plan_text_rendering((0, 0), font, style, &song.title)?
				.1
				.width >= drawer.layout.width(bottom_in_area)
			{
				style.size -= 3;
				style.stroke = Some((
					Color::BLACK,
					style.size as f32 / (initial_size as f32) * 1.5,
				));
			}

			drawer.text(
				bottom_in_area,
				(0, drawer.layout.height(bottom_in_area) as i32 / 2),
				font,
				style,
				&song.title,
			)
		})?;
		// }}}
		// {{{ Display jacket
		let jacket = chart.cached_jacket.as_ref().ok_or_else(|| {
			format!(
				"Cannot find jacket for chart {} [{:?}]",
				song.title, chart.difficulty
			)
		})?;

		drawer.fill(jacket_with_border, Color::from_rgb_int(0x271E35));
		drawer.blit_rbg(
			jacket_area,
			(0, 0),
			jacket.bitmap.dimensions(),
			&jacket.bitmap.as_raw(),
		);
		// }}}
		// {{{ Display difficulty background
		let diff_bg = get_difficulty_background(chart.difficulty);
		let diff_bg_area = Rect::from_image(diff_bg).align_whole(
			(Align::Center, Align::Center),
			(drawer.layout.width(jacket_with_border) as i32, 0),
		);

		drawer.blit_rbga(
			jacket_with_border,
			diff_bg_area.top_left(),
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

		let diff_area_center = diff_bg_area.center();

		EXO_FONT.with_borrow_mut(|font| {
			drawer.text(
				jacket_with_border,
				(diff_area_center.0 + x_offset, diff_area_center.1),
				font,
				crate::bitmap::TextStyle {
					size: 25,
					weight: 600,
					color: Color::from_rgb_int(0xffffff),
					align: (Align::Center, Align::Center),
					stroke: None,
					drop_shadow: None,
				},
				&chart.level,
			)
		})?;
		// }}}
		// {{{ Display score background
		let score_bg = get_score_background();
		let score_bg_pos = Rect::from_image(score_bg).align(
			(Align::End, Align::End),
			(
				drawer.layout.width(jacket_area) as i32,
				drawer.layout.height(jacket_area) as i32,
			),
		);

		drawer.blit_rbga(
			jacket_area,
			score_bg_pos,
			score_bg.dimensions(),
			&score_bg.as_raw(),
		);
		// }}}
		// {{{ Display score text
		EXO_FONT.with_borrow_mut(|font| {
			drawer.text(
				jacket_area,
				(
					score_bg_pos.0 + 5,
					score_bg_pos.1 + score_bg.height() as i32 / 2,
				),
				font,
				crate::bitmap::TextStyle {
					size: 23,
					weight: 800,
					color: Color::WHITE,
					align: (Align::Start, Align::Center),
					stroke: Some((Color::BLACK, 1.5)),
					drop_shadow: None,
				},
				&format!("{:0>10}", format!("{}", play.score)),
			)
		})?;
		// }}}
		// {{{ Display status background
		let status_bg = get_status_background();
		let status_bg_area = Rect::from_image(status_bg).align_whole(
			(Align::Center, Align::Center),
			(
				drawer.layout.width(jacket_area) as i32 + 3,
				drawer.layout.height(jacket_area) as i32 + 1,
			),
		);

		drawer.blit_rbga(
			jacket_area,
			status_bg_area.top_left(),
			status_bg.dimensions(),
			&status_bg.as_raw(),
		);
		// }}}
		// {{{ Display status text
		EXO_FONT.with_borrow_mut(|font| {
			let status = play
				.short_status(chart)
				.ok_or_else(|| format!("Could not get status for score {}", play.score))?;

			let x_offset = match status {
				'P' => 2,
				'M' => 2,
				// TODO: ensure the F is rendered properly as well
				_ => 0,
			};

			let center = status_bg_area.center();

			drawer.text(
				jacket_area,
				(center.0 + x_offset, center.1),
				font,
				crate::bitmap::TextStyle {
					size: if status == 'M' { 30 } else { 36 },
					weight: if status == 'M' { 800 } else { 500 },
					color: Color::WHITE,
					align: (Align::Center, Align::Center),
					stroke: None,
					drop_shadow: None,
				},
				&format!("{}", status),
			)
		})?;
		// }}}
		// {{{ Display grade background
		let top_left_center = (drawer.layout.width(top_left_area) as i32 + jacket_margin) / 2;
		let grade_bg = get_grade_background();
		let grade_bg_area = Rect::from_image(grade_bg).align_whole(
			(Align::Center, Align::Center),
			(top_left_center, jacket_margin + 140),
		);

		drawer.blit_rbga(
			top_area,
			grade_bg_area.top_left(),
			grade_bg.dimensions(),
			&grade_bg.as_raw(),
		);
		// }}}
		// {{{ Display grade text
		EXO_FONT.with_borrow_mut(|font| {
			let grade = play.score.grade();
			let center = grade_bg_area.center();

			drawer.text(
				top_left_area,
				(center.0, center.1),
				font,
				crate::bitmap::TextStyle {
					size: 30,
					weight: 650,
					color: Color::from_rgb_int(0x203C6B),
					align: (Align::Center, Align::Center),
					stroke: Some((Color::WHITE, 1.5)),
					drop_shadow: None,
				},
				&format!("{}", grade),
			)
		})?;
		// }}}
		// {{{ Display rating text
		EXO_FONT.with_borrow_mut(|font| -> Result<(), Error> {
			let mut style = crate::bitmap::TextStyle {
				size: 12,
				weight: 600,
				color: Color::WHITE,
				align: (Align::Center, Align::Center),
				stroke: None,
				drop_shadow: None,
			};

			drawer.text(
				top_left_area,
				(top_left_center, 73),
				font,
				style,
				"POTENTIAL",
			)?;

			style.size = 25;
			style.weight = 700;

			drawer.text(
				top_left_area,
				(top_left_center, 94),
				font,
				style,
				&format!("{:.2}", play.score.play_rating_f32(chart.chart_constant)),
			)?;

			Ok(())
		})?;
		// }}}
		// {{{ Display ptt emblem
		let ptt_emblem = get_ptt_emblem();
		drawer.blit_rbga(
			top_left_area,
			Rect::from_image(ptt_emblem)
				.align((Align::Center, Align::Center), (top_left_center, 115)),
			ptt_emblem.dimensions(),
			ptt_emblem.as_raw(),
		);
		// }}}
	}

	let mut out_buffer = Vec::new();
	let image: ImageBuffer<Rgb<u8>, _> =
		ImageBuffer::from_raw(width, height, drawer.canvas.buffer).unwrap();

	let mut cursor = Cursor::new(&mut out_buffer);
	image.write_to(&mut cursor, image::ImageFormat::Png)?;

	let reply = CreateReply::default()
		.attachment(CreateAttachment::bytes(out_buffer, "b30.png"))
		.content(format!(
			"Your ptt is {:.2}",
			compute_b30_ptt(&plays) as f32 / 100.0
		));
	ctx.send(reply).await?;

	Ok(())
}
// }}}
