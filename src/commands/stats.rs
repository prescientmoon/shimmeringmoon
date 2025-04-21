// {{{ Imports
use std::io::Cursor;

use anyhow::anyhow;
use image::{DynamicImage, ImageBuffer};
use poise::serenity_prelude::{CreateAttachment, CreateEmbed};
use poise::CreateReply;

use crate::arcaea::achievement::GoalStats;
use crate::arcaea::chart::Level;
use crate::arcaea::jacket::BITMAP_IMAGE_SIZE;
use crate::arcaea::play::{compute_b30_ptt, get_best_plays};
use crate::arcaea::rating::rating_as_float;
use crate::arcaea::score::ScoringSystem;
use crate::assets::{
	get_difficulty_background, with_font, B30_BACKGROUND, COUNT_BACKGROUND, EXO_FONT,
	GRADE_BACKGROUND, NAME_BACKGROUND, PTT_EMBLEM, SCORE_BACKGROUND, STATUS_BACKGROUND,
	TOP_BACKGROUND,
};
use crate::bitmap::{Align, BitmapCanvas, Color, LayoutDrawer, LayoutManager, Rect};
use crate::context::{Error, PoiseContext, TaggedError};
use crate::logs::debug_image_log;
use crate::user::User;

use super::discord::MessageContext;
use super::DataSource;
// }}}

// {{{ Stats
/// Query various stats.
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("meta", "b30", "bany"),
	subcommand_required
)]
pub async fn stats(_ctx: PoiseContext<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Render best plays
async fn best_plays<C: MessageContext>(
	ctx: &mut C,
	user: &User,
	source: DataSource,
	scoring_system: ScoringSystem,
	grid_size: (u32, u32),
	require_full: bool,
) -> Result<(), TaggedError> {
	let user_ctx = ctx.data();
	let plays = get_best_plays(
		user_ctx,
		user,
		source,
		scoring_system,
		if require_full {
			grid_size.0 * grid_size.1
		} else {
			grid_size.0 * (grid_size.1.max(1) - 1) + 1
		} as usize,
		(grid_size.0 * grid_size.1) as usize,
		None,
	)
	.await?;

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
	let (item_grid, item_origins) =
		layout.repeated_evenly(item_with_margin, (grid_size.0, grid_size.1));
	let root = layout.margin_uniform(item_grid, 30);
	// }}}
	// {{{ Rendering prep
	let width = layout.width(root);
	let height = layout.height(root);

	let canvas = BitmapCanvas::new(width, height);
	let mut drawer = LayoutDrawer::new(layout, canvas);
	// }}}
	// {{{ Render background
	let bg = &*B30_BACKGROUND;

	let scale = (drawer.layout.width(root) as f32 / bg.width() as f32)
		.max(drawer.layout.height(root) as f32 / bg.height() as f32)
		.max(1.0)
		.ceil() as u32;

	drawer.blit_rbg_scaled_up(
		root,
		// Align the center of the image with the center of the root
		Rect::from_image(bg).scaled(scale).align(
			(Align::Center, Align::Center),
			drawer.layout.lookup(root).center(),
		),
		bg.dimensions(),
		bg.as_raw(),
		scale,
	);
	// }}}

	for (i, origin) in item_origins.enumerate() {
		drawer
			.layout
			.edit_to_relative(item_with_margin, item_grid, origin.0, origin.1);

		let (play, song, chart) = if let Some(item) = plays.get(i) {
			item
		} else {
			break;
		};

		let top_bg = &*TOP_BACKGROUND;
		drawer.blit_rbga(top_area, (0, 0), top_bg);

		// {{{ Display index
		let bg = &*COUNT_BACKGROUND;
		let bg_center = Rect::from_image(bg).center();

		// Draw background
		drawer.blit_rbga(item_area, (-8, jacket_margin), bg);
		with_font(&EXO_FONT, |faces| {
			drawer.text(
				item_area,
				(bg_center.0 - 12, bg_center.1 - 3 + jacket_margin),
				faces,
				crate::bitmap::TextStyle {
					size: 25,
					weight: Some(800),
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
		let bg = &*NAME_BACKGROUND;
		drawer.blit_rbga(bottom_area, (0, 0), bg);

		// Draw text
		with_font(&EXO_FONT, |faces| {
			let initial_size = 24;
			let mut style = crate::bitmap::TextStyle {
				size: initial_size,
				weight: Some(800),
				color: Color::WHITE,
				align: (Align::Start, Align::Center),
				stroke: Some((Color::BLACK, 1.5)),
				drop_shadow: None,
			};

			while BitmapCanvas::plan_text_rendering((0, 0), faces, style, &song.title)?
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
				faces,
				style,
				&song.title,
			)
		})?;
		// }}}
		// {{{ Display jacket
		let jacket = chart.cached_jacket.as_ref().ok_or_else(|| {
			anyhow!(
				"Cannot find jacket for chart {} [{:?}]",
				song.title,
				chart.difficulty
			)
		})?;

		drawer.fill(jacket_with_border, Color::from_rgb_int(0x271E35));
		drawer.blit_rbg(jacket_area, (0, 0), jacket.bitmap);
		// }}}
		// {{{ Display difficulty background
		let diff_bg = get_difficulty_background(chart.difficulty);
		let diff_bg_area = Rect::from_image(diff_bg).align_whole(
			(Align::Center, Align::Center),
			(drawer.layout.width(jacket_with_border) as i32, 0),
		);

		drawer.blit_rbga(jacket_with_border, diff_bg_area.top_left(), diff_bg);
		// }}}
		// {{{ Display difficulty text
		let level_text = Level::LEVEL_STRINGS[chart.level.to_index()];
		let x_offset = if level_text.ends_with("+") {
			3
		} else if chart.level == Level::Eleven {
			-2
		} else {
			0
		};

		let diff_area_center = diff_bg_area.center();

		with_font(&EXO_FONT, |faces| {
			drawer.text(
				jacket_with_border,
				(diff_area_center.0 + x_offset, diff_area_center.1),
				faces,
				crate::bitmap::TextStyle {
					size: 25,
					weight: Some(600),
					color: Color::from_rgb_int(0xffffff),
					align: (Align::Center, Align::Center),
					stroke: None,
					drop_shadow: None,
				},
				level_text,
			)
		})?;
		// }}}
		// {{{ Display score background
		let score_bg = &*SCORE_BACKGROUND;
		let score_bg_pos = Rect::from_image(score_bg).align(
			(Align::End, Align::End),
			(
				drawer.layout.width(jacket_area) as i32,
				drawer.layout.height(jacket_area) as i32,
			),
		);

		drawer.blit_rbga(jacket_area, score_bg_pos, score_bg);
		// }}}
		// {{{ Display score text
		with_font(&EXO_FONT, |faces| {
			drawer.text(
				jacket_area,
				(
					score_bg_pos.0 + 5,
					score_bg_pos.1 + score_bg.height() as i32 / 2,
				),
				faces,
				crate::bitmap::TextStyle {
					size: 23,
					weight: Some(800),
					color: Color::WHITE,
					align: (Align::Start, Align::Center),
					stroke: Some((Color::BLACK, 1.5)),
					drop_shadow: None,
				},
				&format!("{:0>10}", format!("{}", play.score(scoring_system))),
			)
		})?;
		// }}}
		// {{{ Display status background
		let status_bg = &*STATUS_BACKGROUND;
		let status_bg_area = Rect::from_image(status_bg).align_whole(
			(Align::Center, Align::Center),
			(
				drawer.layout.width(jacket_area) as i32 + 3,
				drawer.layout.height(jacket_area) as i32 + 1,
			),
		);

		drawer.blit_rbga(jacket_area, status_bg_area.top_left(), status_bg);
		// }}}
		// {{{ Display status text
		with_font(&EXO_FONT, |faces| {
			let status = play.short_status(scoring_system, chart).ok_or_else(|| {
				anyhow!(
					"Could not get status for score {}",
					play.score(scoring_system)
				)
			})?;

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
				faces,
				crate::bitmap::TextStyle {
					size: if status == 'M' { 30 } else { 36 },
					weight: Some(if status == 'M' { 800 } else { 500 }),
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
		let grade_bg = &*GRADE_BACKGROUND;
		let grade_bg_area = Rect::from_image(grade_bg).align_whole(
			(Align::Center, Align::Center),
			(top_left_center, jacket_margin + 140),
		);

		drawer.blit_rbga(top_area, grade_bg_area.top_left(), grade_bg);
		// }}}
		// {{{ Display grade text
		with_font(&EXO_FONT, |faces| {
			let grade = play.score(scoring_system).grade();
			let center = grade_bg_area.center();

			drawer.text(
				top_left_area,
				(center.0, center.1),
				faces,
				crate::bitmap::TextStyle {
					size: 30,
					weight: Some(650),
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
		with_font(&EXO_FONT, |faces| -> Result<(), Error> {
			let mut style = crate::bitmap::TextStyle {
				size: 12,
				weight: Some(600),
				color: Color::WHITE,
				align: (Align::Center, Align::Center),
				stroke: None,
				drop_shadow: None,
			};

			drawer.text(
				top_left_area,
				(top_left_center, 73),
				faces,
				style,
				"POTENTIAL",
			)?;

			style.size = 25;
			style.weight = Some(700);

			drawer.text(
				top_left_area,
				(top_left_center, 94),
				faces,
				style,
				&format!(
					"{:.2}",
					play.play_rating_f32(scoring_system, chart.chart_constant)
				),
			)?;

			Ok(())
		})?;
		// }}}
		// {{{ Display ptt emblem
		let ptt_emblem = &*PTT_EMBLEM;
		drawer.blit_rbga(
			top_left_area,
			Rect::from_image(ptt_emblem)
				.align((Align::Center, Align::Center), (top_left_center, 115)),
			ptt_emblem,
		);
		// }}}
	}

	let mut out_buffer = Vec::new();
	let mut image = DynamicImage::ImageRgb8(
		ImageBuffer::from_raw(width, height, drawer.canvas.buffer.into_vec()).unwrap(),
	);

	debug_image_log(&image);

	if image.height() > 2048 {
		image = image.resize(2048, 2048, image::imageops::FilterType::Lanczos3);
	}

	let mut cursor = Cursor::new(&mut out_buffer);
	image.write_to(&mut cursor, image::ImageFormat::WebP)?;

	let reply = CreateReply::default()
		.attachment(CreateAttachment::bytes(out_buffer, "b30.webp"))
		.content(format!(
			"Your ptt is {:.2}",
			rating_as_float(compute_b30_ptt(scoring_system, &plays))
		));
	ctx.send(reply).await?;

	Ok(())
}
// }}}
// {{{ B30
// {{{ Implementation
pub async fn b30_impl<C: MessageContext>(
	ctx: &mut C,
	source: Option<DataSource>,
	scoring_system: Option<ScoringSystem>,
) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;
	best_plays(
		ctx,
		&user,
		source.unwrap_or_default(),
		scoring_system.unwrap_or_default(),
		(5, 6),
		true,
	)
	.await?;
	Ok(())
}
// }}}
// {{{ Discord wrapper
/// Show the 30 best scores
#[poise::command(prefix_command, slash_command, user_cooldown = 10)]
pub async fn b30(
	mut ctx: PoiseContext<'_>,
	source: Option<DataSource>,
	scoring_system: Option<ScoringSystem>,
) -> Result<(), Error> {
	ctx.defer().await?;
	let res = b30_impl(&mut ctx, source, scoring_system).await;
	ctx.handle_error(res).await?;
	Ok(())
}
// }}}
// }}}
// {{{ B-any
// {{{ Implementation
async fn bany_impl<C: MessageContext>(
	ctx: &mut C,
	source: Option<DataSource>,
	scoring_system: Option<ScoringSystem>,
	width: u32,
	height: u32,
) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;
	user.assert_is_pookie()?;
	best_plays(
		ctx,
		&user,
		source.unwrap_or_default(),
		scoring_system.unwrap_or_default(),
		(width, height),
		false,
	)
	.await?;

	Ok(())
}
// }}}
// {{{ Discord wrapper
#[poise::command(prefix_command, slash_command, hide_in_help, global_cooldown = 5)]
pub async fn bany(
	mut ctx: PoiseContext<'_>,
	source: Option<DataSource>,
	scoring_system: Option<ScoringSystem>,
	width: u32,
	height: u32,
) -> Result<(), Error> {
	ctx.defer().await?;
	let res = bany_impl(&mut ctx, source, scoring_system, width, height).await;
	ctx.handle_error(res).await?;
	Ok(())
}
// }}}
// }}}
// {{{ Meta
// {{{ Implementation
async fn meta_impl<C: MessageContext>(ctx: &mut C) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;
	let conn = ctx.data().db.get()?;
	let song_count: usize = conn
		.prepare_cached("SELECT count() as count FROM songs")?
		.query_row((), |row| row.get(0))?;

	let chart_count: usize = conn
		.prepare_cached("SELECT count() as count FROM charts")?
		.query_row((), |row| row.get(0))?;

	let users_count: usize = conn
		.prepare_cached("SELECT count() as count FROM users")?
		.query_row((), |row| row.get(0))?;

	let pookie_count: usize = conn
		.prepare_cached(
			"
        SELECT count() as count 
        FROM users 
        WHERE is_pookie=1
      ",
		)?
		.query_row((), |row| row.get(0))?;

	let play_count: usize = conn
		.prepare_cached("SELECT count() as count FROM plays")?
		.query_row((), |row| row.get(0))?;

	let your_play_count: usize = conn
		.prepare_cached(
			"
        SELECT count() as count 
        FROM plays 
        WHERE user_id=?
      ",
		)?
		.query_row([user.id], |row| row.get(0))?;

	let embed = CreateEmbed::default()
		.title("Bot statistics")
		.field("Songs", format!("{song_count}"), true)
		.field("Charts", format!("{chart_count}"), true)
		.field("Users", format!("{users_count}"), true)
		.field("Pookies", format!("{pookie_count}"), true)
		.field("Plays", format!("{play_count}"), true)
		.field("Your plays", format!("{your_play_count}"), true);

	ctx.send(CreateReply::default().reply(true).embed(embed))
		.await?;

	// TODO: remove once achivement system is implemented
	println!(
		"{:?}",
		GoalStats::make(ctx.data(), &user, ScoringSystem::Standard).await?
	);

	Ok(())
}
// }}}
// {{{ Discord wrapper
/// Show stats about the bot itself.
#[poise::command(prefix_command, slash_command, user_cooldown = 1)]
async fn meta(mut ctx: PoiseContext<'_>) -> Result<(), Error> {
	let res = meta_impl(&mut ctx).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
