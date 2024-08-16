use std::fmt::Display;

use hypertesseract::{PageSegMode, Tesseract};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use num::integer::Roots;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed, CreateMessage};

use crate::arcaea::chart::{Chart, Difficulty, Song, DIFFICULTY_MENU_PIXEL_COLORS};
use crate::arcaea::jacket::IMAGE_VEC_DIM;
use crate::arcaea::score::Score;
use crate::bitmap::{Color, Rect};
use crate::context::{Context, Error, UserContext};
use crate::levenshtein::edit_distance;
use crate::logs::debug_image_log;
use crate::recognition::fuzzy_song_name::guess_chart_name;
use crate::recognition::ui::{
	ScoreScreenRect, SongSelectRect, UIMeasurementRect, UIMeasurementRect::*,
};
use crate::timed;
use crate::transform::rotate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreKind {
	SongSelect,
	ScoreScreen,
}

/// Caches a byte vector in order to prevent reallocation
#[derive(Debug, Clone, Default)]
pub struct ImageAnalyzer {
	/// cached byte array
	pub bytes: Vec<u8>,

	/// Last rect used to crop something
	last_rect: Option<(UIMeasurementRect, Rect)>,
}

impl ImageAnalyzer {
	/// Similar to reinitializing this, but without deallocating memory
	#[inline]
	pub fn clear(&mut self) {
		self.bytes.clear();
		self.last_rect = None;
	}

	// {{{ Crop
	#[inline]
	pub fn crop(&mut self, image: &DynamicImage, rect: Rect) -> DynamicImage {
		image.crop_imm(rect.x as u32, rect.y as u32, rect.width, rect.height)
	}

	#[inline]
	pub fn interp_crop(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
		ui_rect: UIMeasurementRect,
	) -> Result<DynamicImage, Error> {
		let rect = ctx.ui_measurements.interpolate(ui_rect, image)?;
		self.last_rect = Some((ui_rect, rect));

		let result = self.crop(image, rect);
		debug_image_log(&result)?;

		Ok(result)
	}

	#[inline]
	pub fn interp_crop_resize(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
		ui_rect: UIMeasurementRect,
		size: (u32, u32),
	) -> Result<DynamicImage, Error> {
		let rect = ctx.ui_measurements.interpolate(ui_rect, image)?;
		self.last_rect = Some((ui_rect, rect));

		let result = self.crop(image, rect);
		let result = result.resize(size.0, size.1, FilterType::Nearest);

		debug_image_log(&result)?;

		Ok(result)
	}
	// }}}
	// {{{ Error handling
	pub async fn send_discord_error(
		&mut self,
		ctx: Context<'_>,
		image: &DynamicImage,
		filename: &str,
		err: impl Display,
	) -> Result<(), Error> {
		let mut embed = CreateEmbed::default().description(format!(
			"Nerdy info
```
{}
```",
			err
		));

		if let Some((ui_rect, rect)) = self.last_rect {
			self.crop(image, rect);

			let bytes = std::mem::take(&mut self.bytes);
			let error_attachement = CreateAttachment::bytes(bytes, filename);

			embed = embed.attachment(filename).title(format!(
				"An error occurred, around the time I was extracting data for {ui_rect:?}"
			));

			let msg = CreateMessage::default().embed(embed);
			ctx.channel_id()
				.send_files(ctx.http(), [error_attachement], msg)
				.await?;
		} else {
			embed = embed.title("An error occurred");

			let msg = CreateMessage::default().embed(embed);
			ctx.channel_id().send_files(ctx.http(), [], msg).await?;
		}

		Ok(())
	}
	// }}}
	// {{{ Read score
	pub fn read_score(
		&mut self,
		ctx: &UserContext,
		note_count: Option<u32>,
		image: &DynamicImage,
		kind: ScoreKind,
	) -> Result<Score, Error> {
		let image = timed!("interp_crop_resize", {
			self.interp_crop(
				ctx,
				image,
				match kind {
					ScoreKind::SongSelect => SongSelect(SongSelectRect::Score),
					ScoreKind::ScoreScreen => ScoreScreen(ScoreScreenRect::Score),
				},
			)?
		});

		let measurements = match kind {
			ScoreKind::SongSelect => &ctx.exo_measurements,
			ScoreKind::ScoreScreen => &ctx.geosans_measurements,
		};

		let result = timed!("full recognition", {
			Score(
				measurements
					.recognise(&image, "0123456789'", None)?
					.chars()
					.filter(|c| *c != '\'')
					.collect::<String>()
					.parse()?,
			)
		});

		// Discard scores if it's impossible
		if result.0 <= 10_010_000
			&& note_count.map_or(true, |note_count| {
				let (zeta, shinies, score_units) = result.analyse(note_count);
				8_000_000 <= zeta.0
					&& zeta.0 <= 10_000_000
					&& shinies <= note_count
					&& score_units <= 2 * note_count
			}) {
			Ok(result)
		} else {
			Err(format!("Score {result} is not vaild").into())
		}
	}
	// }}}
	// {{{ Read difficulty
	pub fn read_difficulty(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
		grayscale_image: &DynamicImage,
		kind: ScoreKind,
	) -> Result<Difficulty, Error> {
		if kind == ScoreKind::SongSelect {
			let min = DIFFICULTY_MENU_PIXEL_COLORS
				.iter()
				.zip(Difficulty::DIFFICULTIES)
				.min_by_key(|(c, d)| {
					let rect = ctx
						.ui_measurements
						.interpolate(
							SongSelect(match d {
								Difficulty::PST => SongSelectRect::Past,
								Difficulty::PRS => SongSelectRect::Present,
								Difficulty::FTR => SongSelectRect::Future,
								_ => SongSelectRect::Beyond,
							}),
							image,
						)
						.unwrap();

					let image_color = image.get_pixel(rect.x as u32, rect.y as u32);
					let image_color = Color::from_bytes(image_color.0);

					let distance = c.distance(image_color);
					(distance * 10000.0) as u32
				})
				.unwrap();

			return Ok(min.1);
		}

		let image = self.interp_crop(
			ctx,
			grayscale_image,
			ScoreScreen(ScoreScreenRect::Difficulty),
		)?;

		let text = ctx.kazesawa_bold_measurements.recognise(
			&image,
			"PASTPRESENTFUTUREETERNALBEYOND",
			None,
		)?;

		let difficulty = Difficulty::DIFFICULTIES
			.iter()
			.zip(Difficulty::DIFFICULTY_STRINGS)
			.min_by_key(|(_, difficulty_string)| edit_distance(difficulty_string, &text))
			.map(|(difficulty, _)| *difficulty)
			.ok_or_else(|| format!("Unrecognised difficulty '{}'", text))?;

		Ok(difficulty)
	}
	// }}}
	// {{{ Read score kind
	pub fn read_score_kind(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
	) -> Result<ScoreKind, Error> {
		let image = self.interp_crop(ctx, image, PlayKind)?;
		let text = ctx
			.kazesawa_measurements
			.recognise(&image, "ResultSelectaSong ", None)?;

		let result = if edit_distance(&text, "Result") < edit_distance(&text, "SelectaSong") {
			ScoreKind::ScoreScreen
		} else {
			ScoreKind::SongSelect
		};

		Ok(result)
	}
	// }}}
	// {{{ Read song
	pub fn read_song<'a>(
		&mut self,
		ctx: &'a UserContext,
		image: &DynamicImage,
		difficulty: Difficulty,
	) -> Result<(&'a Song, &'a Chart), Error> {
		let (text, conf) = Tesseract::builder()
			.language(hypertesseract::Language::English)
			.page_seg_mode(PageSegMode::SingleLine)
			.whitelist_str("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789,.()- ")?
			.build()?
			.recognize_text_cloned_with_conf(
				&self
					.interp_crop(ctx, image, ScoreScreen(ScoreScreenRect::Title))?
					.into_rgba8(),
			)?;

		if conf < 20 && conf != 0 {
			return Err(format!(
				"Title text is not readable (confidence = {}, text = {}).",
				conf,
				text.trim()
			)
			.into());
		}

		guess_chart_name(&text, &ctx.song_cache, Some(difficulty), false)
	}
	// }}}
	// {{{ Read jacket
	pub fn read_jacket<'a>(
		&mut self,
		ctx: &'a UserContext,
		image: &mut DynamicImage,
		kind: ScoreKind,
		difficulty: Difficulty,
	) -> Result<(&'a Song, &'a Chart), Error> {
		let rect = ctx.ui_measurements.interpolate(
			if kind == ScoreKind::ScoreScreen {
				ScoreScreen(ScoreScreenRect::Jacket)
			} else {
				SongSelect(SongSelectRect::Jacket)
			},
			image,
		)?;

		let cropped = if kind == ScoreKind::ScoreScreen {
			image.view(rect.x as u32, rect.y as u32, rect.width, rect.height)
		} else {
			let angle = f32::atan2(rect.height as f32, rect.width as f32);
			let side = rect.height + rect.width;
			rotate(
				image,
				Rect::new(rect.x, rect.y, side, side),
				(rect.x, rect.y + rect.height as i32),
				angle,
			);

			let len = (rect.width.pow(2) + rect.height.pow(2)).sqrt();

			image.view(rect.x as u32, rect.y as u32 + rect.height, len, len)
		};
		let (distance, song_id) = ctx
			.jacket_cache
			.recognise(&*cropped)
			.ok_or_else(|| "Could not recognise jacket")?;

		if distance > (IMAGE_VEC_DIM * 3) as f32 {
			Err("No known jacket looks like this")?;
		}

		let (song, chart) = ctx.song_cache.lookup_by_difficulty(*song_id, difficulty)?;

		Ok((song, chart))
	}
	// }}}
	// {{{ Read distribution
	pub fn read_distribution(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
	) -> Result<(u32, u32, u32), Error> {
		let mut out = [0; 3];

		use ScoreScreenRect::*;
		static KINDS: [ScoreScreenRect; 3] = [Pure, Far, Lost];

		for i in 0..3 {
			let image = self.interp_crop(ctx, image, ScoreScreen(KINDS[i]))?;
			out[i] = ctx
				.kazesawa_bold_measurements
				.recognise(&image, "0123456789", Some(30))?
				.parse()?;
		}

		println!("Ditribution {out:?}");

		Ok((out[0], out[1], out[2]))
	}
	// }}}
	// {{{ Read max recall
	pub fn read_max_recall<'a>(
		&mut self,
		ctx: &'a UserContext,
		image: &DynamicImage,
	) -> Result<u32, Error> {
		let image = self.interp_crop(ctx, image, ScoreScreen(ScoreScreenRect::MaxRecall))?;
		let max_recall = ctx
			.exo_measurements
			.recognise(&image, "0123456789", None)?
			.parse()?;

		Ok(max_recall)
	}
	// }}}
}