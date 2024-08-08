use std::fmt::Display;
use std::io::Cursor;
use std::str::FromStr;
use std::{env, fs};

use hypertesseract::{PageSegMode, Tesseract};
use image::{DynamicImage, GenericImageView};
use image::{ImageBuffer, Rgba};
use num::integer::Roots;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed, CreateMessage, Timestamp};

use crate::arcaea::chart::{Chart, Difficulty, Song, DIFFICULTY_MENU_PIXEL_COLORS};
use crate::arcaea::jacket::IMAGE_VEC_DIM;
use crate::arcaea::score::Score;
use crate::bitmap::{Color, Rect};
use crate::context::{Context, Error, UserContext};
use crate::levenshtein::edit_distance;
use crate::recognition::fuzzy_song_name::guess_chart_name;
use crate::recognition::ui::{
	ScoreScreenRect, SongSelectRect, UIMeasurementRect, UIMeasurementRect::*,
};
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
	pub fn crop_image_to_bytes(&mut self, image: &DynamicImage, rect: Rect) -> Result<(), Error> {
		self.clear();
		let image = image.crop_imm(rect.x as u32, rect.y as u32, rect.width, rect.height);
		let mut cursor = Cursor::new(&mut self.bytes);
		image.write_to(&mut cursor, image::ImageFormat::Png)?;

		fs::write(format!("./logs/{}.png", Timestamp::now()), &self.bytes)?;

		Ok(())
	}

	#[inline]
	pub fn crop(&mut self, image: &DynamicImage, rect: Rect) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
		if env::var("SHIMMERING_DEBUG_IMGS")
			.map(|s| s == "1")
			.unwrap_or(false)
		{
			self.crop_image_to_bytes(image, rect).unwrap();
		}

		image
			.crop_imm(rect.x as u32, rect.y as u32, rect.width, rect.height)
			.to_rgba8()
	}

	#[inline]
	pub fn interp_crop(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
		ui_rect: UIMeasurementRect,
	) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Error> {
		let rect = ctx.ui_measurements.interpolate(ui_rect, image)?;
		self.last_rect = Some((ui_rect, rect));
		Ok(self.crop(image, rect))
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
			self.crop_image_to_bytes(image, rect)?;

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
	) -> Result<Vec<Score>, Error> {
		let image = self.interp_crop(
			ctx,
			image,
			if kind == ScoreKind::ScoreScreen {
				ScoreScreen(ScoreScreenRect::Score)
			} else {
				SongSelect(SongSelectRect::Score)
			},
		)?;

		let mut results = vec![];
		for mode in [
			PageSegMode::SingleWord,
			PageSegMode::RawLine,
			PageSegMode::SingleLine,
			PageSegMode::SparseText,
			PageSegMode::SingleBlock,
		] {
			let result: Result<_, Error> = try {
				// {{{ Read score using tesseract
				let text = Tesseract::builder()
					.language(hypertesseract::Language::English)
					.whitelist_str("0123456789'/")?
					.page_seg_mode(mode)
					.assume_numeric_input()
					.build()?
					.load_image(&image)?
					.recognize()?
					.get_text()?;

				let text: String = text
					.trim()
					.chars()
					.map(|char| if char == '/' { '7' } else { char })
					.filter(|char| *char != ' ' && *char != '\'')
					.collect();

				let score = u32::from_str_radix(&text, 10)?;
				Score(score)
				// }}}
			};

			match result {
				Ok(result) => {
					results.push(result.0);
				}
				Err(err) => {
					println!("OCR score result error: {}", err);
				}
			}
		}

		// {{{ Score correction
		// The OCR sometimes fails to read "74" with the arcaea font,
		// so we try to detect that and fix it
		loop {
			let old_stack_len = results.len();
			println!("Results {:?}", results);
			results = results
				.iter()
				.flat_map(|result| {
					// If the length is correct, we are good to go!
					if *result >= 8_000_000 {
						vec![*result]
					} else {
						let mut results = vec![];
						for i in [0, 1, 3, 4] {
							let d = 10u32.pow(i);
							if (*result / d) % 10 == 4 && (*result / d) % 100 != 74 {
								let n = d * 10;
								results.push((*result / n) * n * 10 + 7 * n + (*result % n));
							}
						}

						results
					}
				})
				.collect();

			if old_stack_len == results.len() {
				break;
			}
		}
		// }}}
		// {{{ Return score if consensus exists
		// 1. Discard scores that are known to be impossible
		let mut results: Vec<_> = results
			.into_iter()
			.filter(|result| {
				8_000_000 <= *result
					&& *result <= 10_010_000
					&& note_count
						.map(|note_count| {
							let (zeta, shinies, score_units) = Score(*result).analyse(note_count);
							8_000_000 <= zeta.0
								&& zeta.0 <= 10_000_000 && shinies <= note_count
								&& score_units <= 2 * note_count
						})
						.unwrap_or(true)
			})
			.map(|r| Score(r))
			.collect();
		println!("Results {:?}", results);

		// 2. Look for consensus
		for result in results.iter() {
			if results.iter().filter(|e| **e == *result).count() > results.len() / 2 {
				return Ok(vec![*result]);
			}
		}
		// }}}

		// If there's no consensus, we return everything
		results.sort();
		results.dedup();
		println!("Results {:?}", results);

		Ok(results)
	}
	// }}}
	// {{{ Read difficulty
	pub fn read_difficulty(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
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

					// rect.width = 100;
					// rect.height = 100;
					// self.crop_image_to_bytes(image, rect).unwrap();

					let image_color = image.get_pixel(rect.x as u32, rect.y as u32);
					let image_color = Color::from_bytes(image_color.0);

					let distance = c.distance(image_color);
					(distance * 10000.0) as u32
				})
				.unwrap();

			return Ok(min.1);
		}

		let mut ocr = Tesseract::builder()
			.language(hypertesseract::Language::English)
			.page_seg_mode(PageSegMode::RawLine)
			.build()?;

		ocr.load_image(&self.interp_crop(ctx, image, ScoreScreen(ScoreScreenRect::Difficulty))?)?
			.recognize()?;

		let text: &str = &ocr.get_text()?;
		let text = text.trim().to_lowercase();

		// let conf = t.mean_text_conf();
		// if conf < 10 && conf != 0 {
		// 	Err(format!(
		// 		"Difficulty text is not readable (confidence = {}, text = {}).",
		// 		conf, text
		// 	))?;
		// }

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
		let text = Tesseract::builder()
			.language(hypertesseract::Language::English)
			.page_seg_mode(PageSegMode::RawLine)
			.build()?
			.load_image(&self.interp_crop(ctx, image, PlayKind)?)?
			.recognize()?
			.get_text()?
			.trim()
			.to_string();

		// let conf = t.mean_text_conf();
		// if conf < 10 && conf != 0 {
		// 	Err(format!(
		// 		"Score kind text is not readable (confidence = {}, text = {}).",
		// 		conf, text
		// 	))?;
		// }

		let result = if edit_distance(&text, "Result") < edit_distance(&text, "Select a song") {
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
		let text = Tesseract::builder()
			.language(hypertesseract::Language::English)
			.page_seg_mode(PageSegMode::SingleLine)
			.whitelist_str("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789,.()- ")?
			.build()?
			.load_image(&self.interp_crop(ctx, image, ScoreScreen(ScoreScreenRect::Title))?)?
			.recognize()?
			.get_text()?;

		// let conf = t.mean_text_conf();
		// if conf < 20 && conf != 0 {
		// 	Err(format!(
		// 		"Title text is not readable (confidence = {}, text = {}).",
		// 		conf,
		// 		raw_text.trim()
		// 	))?;
		// }

		guess_chart_name(&text, &ctx.song_cache, Some(difficulty), false)
	}
	// }}}
	// {{{ Read jacket
	pub async fn read_jacket<'a>(
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

		let item = ctx.song_cache.lookup(*song_id)?;
		let chart = item.lookup(difficulty)?;

		// NOTE: this will reallocate a few strings, but it is what it is
		Ok((&item.song, chart))
	}
	// }}}
	// {{{ Read distribution
	pub fn read_distribution(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
	) -> Result<(u32, u32, u32), Error> {
		let mut ocr = Tesseract::builder()
			.language(hypertesseract::Language::English)
			.page_seg_mode(PageSegMode::SparseText)
			.whitelist_str("0123456789")?
			.assume_numeric_input()
			.build()?;

		let mut out = [0; 3];

		use ScoreScreenRect::*;
		static KINDS: [ScoreScreenRect; 3] = [Pure, Far, Lost];

		for i in 0..3 {
			let text = ocr
				.load_image(&self.interp_crop(ctx, image, ScoreScreen(KINDS[i]))?)?
				.recognize()?
				.get_text()?;

			println!("Raw '{}'", text.trim());
			out[i] = u32::from_str(&text.trim()).unwrap_or(0);
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
		let text = Tesseract::builder()
			.language(hypertesseract::Language::English)
			.page_seg_mode(PageSegMode::SingleLine)
			.whitelist_str("0123456789")?
			.assume_numeric_input()
			.build()?
			.load_image(&self.interp_crop(ctx, image, ScoreScreen(ScoreScreenRect::MaxRecall))?)?
			.recognize()?
			.get_text()?;

		let max_recall = u32::from_str_radix(text.trim(), 10)?;

		// let conf = t.mean_text_conf();
		// if conf < 20 && conf != 0 {
		// 	Err(format!(
		// 		"Title text is not readable (confidence = {}, text = {}).",
		// 		conf,
		// 		raw_text.trim()
		// 	))?;
		// }

		Ok(max_recall)
	}
	// }}}
}
