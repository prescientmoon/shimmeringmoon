#![allow(dead_code)]
use std::{
	fmt::Display,
	io::Cursor,
	sync::{Mutex, OnceLock},
};

use image::{DynamicImage, GenericImageView};
use num::Rational64;
use poise::serenity_prelude::{Attachment, AttachmentId, CreateAttachment, CreateEmbed};
use tesseract::{PageSegMode, Tesseract};

use crate::{
	chart::{CachedSong, Chart, Difficulty, Song, SongCache},
	context::{Error, UserContext},
	user::User,
};

// {{{ ImageDimensions
#[derive(Debug, Clone, Copy)]
pub struct ImageDimensions {
	width: u32,
	height: u32,
}

impl ImageDimensions {
	#[inline]
	pub fn new(width: u32, height: u32) -> Self {
		Self { width, height }
	}

	#[inline]
	pub fn aspect_ratio(&self) -> f32 {
		self.width as f32 / self.height as f32
	}

	#[inline]
	pub fn from_image(image: &DynamicImage) -> Self {
		Self::new(image.width(), image.height())
	}
}
// }}}
// {{{ AbsoluteRect
#[derive(Debug, Clone, Copy)]
pub struct AbsoluteRect {
	pub x: u32,
	pub y: u32,
	pub width: u32,
	pub height: u32,
	pub dimensions: ImageDimensions,
}

impl AbsoluteRect {
	#[inline]
	pub fn new(x: u32, y: u32, width: u32, height: u32, dimensions: ImageDimensions) -> Self {
		Self {
			x,
			y,
			width,
			height,
			dimensions,
		}
	}

	#[inline]
	pub fn to_relative(&self) -> RelativeRect {
		RelativeRect::new(
			self.x as f32 / self.dimensions.width as f32,
			self.y as f32 / self.dimensions.height as f32,
			self.width as f32 / self.dimensions.width as f32,
			self.height as f32 / self.dimensions.height as f32,
			self.dimensions,
		)
	}
}
// }}}
// {{{ RelativeRect
#[derive(Debug, Clone, Copy)]
pub struct RelativeRect {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
	pub dimensions: ImageDimensions,
}

fn lerp(i: f32, a: f32, b: f32) -> f32 {
	a + (b - a) * i
}

impl RelativeRect {
	#[inline]
	pub fn new(x: f32, y: f32, width: f32, height: f32, dimensions: ImageDimensions) -> Self {
		Self {
			x,
			y,
			width,
			height,
			dimensions,
		}
	}

	#[inline]
	pub fn to_absolute(&self) -> AbsoluteRect {
		AbsoluteRect::new(
			(self.x * self.dimensions.width as f32) as u32,
			(self.y * self.dimensions.height as f32) as u32,
			(self.width * self.dimensions.width as f32) as u32,
			(self.height * self.dimensions.height as f32) as u32,
			self.dimensions,
		)
	}

	pub fn from_aspect_ratio(
		dimensions: ImageDimensions,
		datapoints: &[RelativeRect],
	) -> Option<Self> {
		let aspect_ratio = dimensions.aspect_ratio();

		for i in 0..(datapoints.len() - 1) {
			let low = datapoints[i];
			let high = datapoints[i + 1];

			let low_ratio = low.dimensions.aspect_ratio();
			let high_ratio = high.dimensions.aspect_ratio();

			if (i == 0 || low_ratio <= aspect_ratio)
				&& (aspect_ratio <= high_ratio || i == datapoints.len() - 2)
			{
				let p = (aspect_ratio - low_ratio) / (high_ratio - low_ratio);
				return Some(Self::new(
					lerp(p, low.x, high.x),
					lerp(p, low.y, high.y),
					lerp(p, low.width, high.width),
					lerp(p, low.height, high.height),
					dimensions,
				));
			}
		}

		None
	}
}
// }}}
// {{{ Data points
// {{{ Processing
fn process_datapoints(rects: &mut Vec<RelativeRect>) {
	rects.sort_by_key(|r| (r.dimensions.aspect_ratio() * 1000.0).floor() as u32);

	// Filter datapoints that are close together
	let mut i = 0;
	while i < rects.len() - 1 {
		let low = rects[i];
		let high = rects[i + 1];

		if (low.dimensions.aspect_ratio() - high.dimensions.aspect_ratio()).abs() < 0.001 {
			// TODO: we could interpolate here but oh well
			rects.remove(i + 1);
		}

		i += 1;
	}
}

fn widen_by(rects: &mut Vec<RelativeRect>, x: f32, y: f32) {
	for rect in rects {
		rect.x -= x;
		rect.y -= y;
		rect.width += 2. * x;
		rect.height += 2. * y;
	}
}
// }}}
// {{{ Score
fn score_rects() -> &'static [RelativeRect] {
	static CELL: OnceLock<Vec<RelativeRect>> = OnceLock::new();
	CELL.get_or_init(|| {
		let mut rects: Vec<RelativeRect> = vec![
			AbsoluteRect::new(642, 287, 284, 51, ImageDimensions::new(1560, 720)).to_relative(),
			AbsoluteRect::new(651, 285, 305, 55, ImageDimensions::new(1600, 720)).to_relative(),
			AbsoluteRect::new(748, 485, 503, 82, ImageDimensions::new(2000, 1200)).to_relative(),
			AbsoluteRect::new(841, 683, 500, 92, ImageDimensions::new(2160, 1620)).to_relative(),
			AbsoluteRect::new(851, 707, 532, 91, ImageDimensions::new(2224, 1668)).to_relative(),
			AbsoluteRect::new(1037, 462, 476, 89, ImageDimensions::new(2532, 1170)).to_relative(),
			AbsoluteRect::new(973, 653, 620, 105, ImageDimensions::new(2560, 1600)).to_relative(),
			AbsoluteRect::new(1069, 868, 636, 112, ImageDimensions::new(2732, 2048)).to_relative(),
			AbsoluteRect::new(1125, 510, 534, 93, ImageDimensions::new(2778, 1284)).to_relative(),
		];
		process_datapoints(&mut rects);
		widen_by(&mut rects, 0.0, 0.01);
		rects
	})
}
// }}}
// {{{ Difficulty
fn difficulty_rects() -> &'static [RelativeRect] {
	static CELL: OnceLock<Vec<RelativeRect>> = OnceLock::new();
	CELL.get_or_init(|| {
		let mut rects: Vec<RelativeRect> = vec![
			AbsoluteRect::new(232, 203, 104, 23, ImageDimensions::new(1560, 720)).to_relative(),
			AbsoluteRect::new(252, 204, 99, 21, ImageDimensions::new(1600, 720)).to_relative(),
			AbsoluteRect::new(146, 356, 155, 34, ImageDimensions::new(2000, 1200)).to_relative(),
			AbsoluteRect::new(155, 546, 167, 38, ImageDimensions::new(2160, 1620)).to_relative(),
			AbsoluteRect::new(163, 562, 175, 38, ImageDimensions::new(2224, 1668)).to_relative(),
			AbsoluteRect::new(378, 332, 161, 34, ImageDimensions::new(2532, 1170)).to_relative(),
			AbsoluteRect::new(183, 487, 197, 44, ImageDimensions::new(2560, 1600)).to_relative(),
			AbsoluteRect::new(198, 692, 219, 46, ImageDimensions::new(2732, 2048)).to_relative(),
			AbsoluteRect::new(414, 364, 177, 38, ImageDimensions::new(2778, 1284)).to_relative(),
		];
		process_datapoints(&mut rects);
		rects
	})
}
// }}}
// {{{ Chart title
fn title_rects() -> &'static [RelativeRect] {
	static CELL: OnceLock<Vec<RelativeRect>> = OnceLock::new();
	CELL.get_or_init(|| {
		let mut rects: Vec<RelativeRect> = vec![
			AbsoluteRect::new(227, 74, 900, 61, ImageDimensions::new(1560, 720)).to_relative(),
			AbsoluteRect::new(413, 72, 696, 58, ImageDimensions::new(1600, 720)).to_relative(),
			AbsoluteRect::new(484, 148, 1046, 96, ImageDimensions::new(2000, 1200)).to_relative(),
			AbsoluteRect::new(438, 324, 1244, 104, ImageDimensions::new(2160, 1620)).to_relative(),
			AbsoluteRect::new(216, 336, 1366, 96, ImageDimensions::new(2224, 1668)).to_relative(),
			AbsoluteRect::new(634, 116, 1252, 102, ImageDimensions::new(2532, 1170)).to_relative(),
			AbsoluteRect::new(586, 222, 1320, 118, ImageDimensions::new(2560, 1600)).to_relative(),
			AbsoluteRect::new(348, 417, 1716, 120, ImageDimensions::new(2732, 2048)).to_relative(),
			AbsoluteRect::new(760, 128, 1270, 118, ImageDimensions::new(2778, 1284)).to_relative(),
		];
		process_datapoints(&mut rects);
		widen_by(&mut rects, 0.1, 0.0);
		rects
	})
}
// }}}
// {{{ Jacket
pub fn jacket_rects() -> &'static [RelativeRect] {
	static CELL: OnceLock<Vec<RelativeRect>> = OnceLock::new();
	CELL.get_or_init(|| {
		let mut rects: Vec<RelativeRect> = vec![
			AbsoluteRect::new(171, 268, 375, 376, ImageDimensions::new(1560, 720)).to_relative(),
			AbsoluteRect::new(190, 267, 376, 377, ImageDimensions::new(1600, 720)).to_relative(),
			AbsoluteRect::new(46, 456, 590, 585, ImageDimensions::new(2000, 1200)).to_relative(),
			AbsoluteRect::new(51, 655, 633, 632, ImageDimensions::new(2160, 1620)).to_relative(),
			AbsoluteRect::new(53, 675, 654, 653, ImageDimensions::new(2224, 1668)).to_relative(),
			AbsoluteRect::new(274, 434, 614, 611, ImageDimensions::new(2532, 1170)).to_relative(),
			AbsoluteRect::new(58, 617, 753, 750, ImageDimensions::new(2560, 1600)).to_relative(),
			AbsoluteRect::new(65, 829, 799, 800, ImageDimensions::new(2732, 2048)).to_relative(),
			AbsoluteRect::new(300, 497, 670, 670, ImageDimensions::new(2778, 1284)).to_relative(),
		];
		process_datapoints(&mut rects);
		rects
	})
}
// }}}
// }}}
// {{{ Score
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score(pub u32);

impl Score {
	// {{{ Score => ζ-Score
	/// Returns the zeta score and the number of shinies
	pub fn to_zeta(self, note_count: u32) -> (Score, u32) {
		// Smallest possible difference between (zeta-)scores
		let increment = Rational64::new_raw(5_000_000, note_count as i64).reduced();
		let zeta_increment = Rational64::new_raw(2_000_000, note_count as i64).reduced();

		let score = Rational64::from_integer(self.0 as i64);
		let score_units = (score / increment).floor();

		let non_shiny_score = (score_units * increment).floor();
		let shinies = score - non_shiny_score;

		let zeta_score_units = Rational64::from_integer(2) * score_units + shinies;
		let zeta_score = Score((zeta_increment * zeta_score_units).floor().to_integer() as u32);

		(zeta_score, shinies.to_integer() as u32)
	}
	// }}}
	// {{{ Score => Play rating
	#[inline]
	pub fn play_rating(self, chart_constant: u32) -> i32 {
		chart_constant as i32
			+ if self.0 >= 10_000_000 {
				200
			} else if self.0 >= 9_800_000 {
				100 + (self.0 as i32 - 9_800_000) / 2_000
			} else {
				(self.0 as i32 - 9_500_000) / 3_000
			}
	}
	// }}}
	// {{{ Score => grade
	#[inline]
	// TODO: Perhaps make an enum for this
	pub fn grade(self) -> &'static str {
		let score = self.0;
		if score > 9900000 {
			"EX+"
		} else if score > 9800000 {
			"EX"
		} else if score > 9500000 {
			"AA"
		} else if score > 9200000 {
			"A"
		} else if score > 8900000 {
			"B"
		} else if score > 8600000 {
			"C"
		} else {
			"D"
		}
	}
	// }}}
}

impl Display for Score {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let score = self.0;
		write!(
			f,
			"{}'{:0>3}'{:0>3}",
			score / 1000000,
			(score / 1000) % 1000,
			score % 1000
		)
	}
}
// }}}
// {{{ Plays
// {{{ Create play
#[derive(Debug, Clone)]
pub struct CreatePlay {
	chart_id: u32,
	user_id: u32,
	discord_attachment_id: Option<AttachmentId>,

	// Actual score data
	score: Score,
	zeta_score: Score,

	// Optional score details
	max_recall: Option<u32>,
	far_notes: Option<u32>,

	// Creation data
	creation_ptt: Option<u32>,
	creation_zeta_ptt: Option<u32>,
}

impl CreatePlay {
	#[inline]
	pub fn new(score: Score, chart: &Chart, user: &User) -> Self {
		Self {
			chart_id: chart.id,
			user_id: user.id,
			discord_attachment_id: None,
			score,
			zeta_score: score.to_zeta(chart.note_count as u32).0,
			max_recall: None,
			far_notes: None,
			// TODO: populate these
			creation_ptt: None,
			creation_zeta_ptt: None,
		}
	}

	#[inline]
	pub fn with_attachment(mut self, attachment: &Attachment) -> Self {
		self.discord_attachment_id = Some(attachment.id);
		self
	}

	// {{{ Save
	pub async fn save(self, ctx: &UserContext) -> Result<Play, Error> {
		let attachment_id = self.discord_attachment_id.map(|i| i.get() as i64);
		let play = sqlx::query!(
			"
                INSERT INTO plays(
                user_id,chart_id,discord_attachment_id,
                score,zeta_score,max_recall,far_notes
                )
                VALUES(?,?,?,?,?,?,?)
                RETURNING id, created_at
            ",
			self.user_id,
			self.chart_id,
			attachment_id,
			self.score.0,
			self.zeta_score.0,
			self.max_recall,
			self.far_notes
		)
		.fetch_one(&ctx.db)
		.await?;

		Ok(Play {
			id: play.id as u32,
			created_at: play.created_at,
			chart_id: self.chart_id,
			user_id: self.user_id,
			discord_attachment_id: self.discord_attachment_id,
			score: self.score,
			zeta_score: self.zeta_score,
			max_recall: self.max_recall,
			far_notes: self.far_notes,
			creation_ptt: self.creation_ptt,
			creation_zeta_ptt: self.creation_zeta_ptt,
		})
	}
	// }}}
}
// }}}
// {{{ Play
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Play {
	id: u32,
	chart_id: u32,
	user_id: u32,
	discord_attachment_id: Option<AttachmentId>,

	// Actual score data
	score: Score,
	zeta_score: Score,

	// Optional score details
	max_recall: Option<u32>,
	far_notes: Option<u32>,

	// Creation data
	created_at: chrono::NaiveDateTime,
	creation_ptt: Option<u32>,
	creation_zeta_ptt: Option<u32>,
}

impl Play {
	// {{{ Play to embed
	pub async fn to_embed(
		&self,
		song: &Song,
		chart: &Chart,
	) -> Result<(CreateEmbed, Option<CreateAttachment>), Error> {
		let (_, shiny_count) = self.score.to_zeta(chart.note_count);

		let attachement_name = format!("{:?}-{:?}.png", song.id, self.score.0);
		let icon_attachement = match &chart.jacket {
			Some(path) => Some(
				CreateAttachment::file(&tokio::fs::File::open(path).await?, &attachement_name)
					.await?,
			),
			None => None,
		};

		let mut embed = CreateEmbed::default()
			.title(format!(
				"{} [{:?} {}]",
				&song.title, chart.difficulty, chart.level
			))
			.field("Score", format!("{} (+?)", self.score), true)
			.field(
				"Rating",
				format!(
					"{:.2} (+?)",
					(self.score.play_rating(chart.chart_constant)) as f32 / 100.
				),
				true,
			)
			.field("Grade", self.score.grade(), true)
			.field("ζ-Score", format!("{} (+?)", self.zeta_score), true)
			.field(
				"ζ-Rating",
				format!(
					"{:.2} (+?)",
					(self.zeta_score.play_rating(chart.chart_constant)) as f32 / 100.
				),
				true,
			)
			.field("ζ-Grade", self.zeta_score.grade(), true)
			.field("Status", "?", true)
			.field("Max recall", "?", true)
			.field("Breakdown", format!("{}/?/?/?", shiny_count), true);

		if icon_attachement.is_some() {
			embed = embed.thumbnail(format!("attachment://{}", &attachement_name));
		}

		Ok((embed, icon_attachement))
	}
	// }}}
}
// }}}
// {{{ Tests
#[cfg(test)]
mod score_tests {
	use super::*;

	#[test]
	fn zeta_score_consistent_with_pms() {
		// note counts
		for note_count in 200..=2000 {
			for shiny_count in 0..=note_count {
				let score = Score(10000000 + shiny_count);
				let zeta_score_units = 4 * (note_count - shiny_count) + 5 * shiny_count;
				let (zeta_score, computed_shiny_count) = score.to_zeta(note_count);
				let expected_zeta_score = Rational64::from_integer(zeta_score_units as i64)
					* Rational64::new_raw(2000000, note_count as i64).reduced();

				assert_eq!(zeta_score, Score(expected_zeta_score.to_integer() as u32));
				assert_eq!(computed_shiny_count, shiny_count);
			}
		}
	}
}
// }}}
// }}}
// {{{ Run OCR
/// Caches a byte vector in order to prevent reallocation
#[derive(Debug, Clone, Default)]
pub struct ImageCropper {
	/// cached byte array
	pub bytes: Vec<u8>,
}

impl ImageCropper {
	pub fn crop_image_to_bytes(
		&mut self,
		image: &DynamicImage,
		rect: AbsoluteRect,
	) -> Result<(), Error> {
		self.bytes.clear();
		let image = image.crop_imm(rect.x, rect.y, rect.width, rect.height);
		let mut cursor = Cursor::new(&mut self.bytes);
		image.write_to(&mut cursor, image::ImageFormat::Png)?;
		Ok(())
	}

	// {{{ Read score
	pub fn read_score(&mut self, image: &DynamicImage) -> Result<Score, Error> {
		self.crop_image_to_bytes(
			&image,
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), score_rects())
				.ok_or_else(|| "Could not find score area in picture")?
				.to_absolute(),
		)?;

		let mut results = vec![];
		for mode in [
			PageSegMode::PsmSingleWord,
			PageSegMode::PsmRawLine,
			PageSegMode::PsmSingleLine,
		] {
			let result = self.read_score_with_mode(image, mode)?;
			results.push(result.0);
			// OCR sometimes loses digits
			if result.0 < 1_000_000 {
				continue;
			} else {
				return Ok(result);
			}
		}

		Err(format!(
			"Cannot read score, no matter the mode. Attempts: {:?}",
			results
		))?;
		unreachable!()
	}

	pub fn read_score_with_mode(
		&mut self,
		image: &DynamicImage,
		mode: PageSegMode,
	) -> Result<Score, Error> {
		let mut t = Tesseract::new(None, Some("eng"))?
			// .set_variable("classify_bln_numeric_mode", "1'")?
			.set_variable("tessedit_char_whitelist", "0123456789'")?
			.set_image_from_mem(&self.bytes)?;
		t.set_page_seg_mode(mode);
		t = t.recognize()?;
		let conf = t.mean_text_conf();

		if conf < 10 && conf != 0 {
			Err(format!(
				"Score text is not readable (confidence = {}, text = {}).",
				conf,
				t.get_text()?.trim()
			))?;
		}

		let text: String = t
			.get_text()?
			.trim()
			.chars()
			.filter(|char| *char != ' ' && *char != '\'')
			.collect();

		let score = u32::from_str_radix(&text, 10)?;
		Ok(Score(score))
	}
	// }}}
	// {{{ Read difficulty
	pub fn read_difficulty(&mut self, image: &DynamicImage) -> Result<Difficulty, Error> {
		self.crop_image_to_bytes(
			&image,
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), difficulty_rects())
				.ok_or_else(|| "Could not find difficulty area in picture")?
				.to_absolute(),
		)?;

		let mut t = Tesseract::new(None, Some("eng"))?.set_image_from_mem(&self.bytes)?;
		t.set_page_seg_mode(PageSegMode::PsmRawLine);
		t = t.recognize()?;

		if t.mean_text_conf() < 10 {
			Err("Difficulty text is not readable.")?;
		}

		let text: &str = &t.get_text()?;
		let text = text.trim();

		let difficulty = Difficulty::DIFFICULTIES
			.iter()
			.zip(Difficulty::DIFFICULTY_STRINGS)
			.min_by_key(|(_, difficulty_string)| {
				edit_distance::edit_distance(difficulty_string, text)
			})
			.map(|(difficulty, _)| *difficulty)
			.ok_or_else(|| format!("Unrecognised difficulty '{}'", text))?;

		Ok(difficulty)
	}
	// }}}
	// {{{ Read song
	pub fn read_song(
		&mut self,
		image: &DynamicImage,
		cache: &Mutex<SongCache>,
	) -> Result<CachedSong, Error> {
		self.crop_image_to_bytes(
			&image,
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), title_rects())
				.ok_or_else(|| "Could not find title area in picture")?
				.to_absolute(),
		)?;

		let mut t = Tesseract::new(None, Some("eng"))?
			.set_variable(
				"tessedit_char_whitelist",
				"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ",
			)?
			.set_image_from_mem(&self.bytes)?;
		t.set_page_seg_mode(PageSegMode::PsmSingleLine);
		t = t.recognize()?;

		// if t.mean_text_conf() < 10 {
		// 	Err("Difficulty text is not readable.")?;
		// }

		let raw_text: &str = &t.get_text()?;
		let raw_text = raw_text.trim(); // not quite raw 🤔
		let mut text = raw_text;

		println!("Raw text: {}, confidence: {}", text, t.mean_text_conf());

		let lock = cache.lock().map_err(|_| "Poisoned song cache")?;
		let cached_song = loop {
			let (closest, distance) = lock
				.songs()
				.map(|item| {
					(
						item,
						edit_distance::edit_distance(
							&item.song.title.to_lowercase(),
							&text.to_lowercase(),
						),
					)
				})
				.min_by_key(|(_, d)| *d)
				.ok_or_else(|| "Empty song cache")?;

			if distance > closest.song.title.len() / 3 {
				if text.len() == 1 {
					Err(format!(
						"Could not find match for chart name '{}'",
						raw_text
					))?;
				} else {
					text = &text[..text.len() - 1];
				}
			} else {
				break closest;
			};
		};

		// NOTE: this will reallocate a few strings, but it is what it is
		Ok(cached_song.clone())
	}
	// }}}
	// {{{ Read jacket
	pub fn read_jacket<'a>(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
	) -> Result<CachedSong, Error> {
		let rect =
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), jacket_rects())
				.ok_or_else(|| "Could not find jacket area in picture")?
				.to_absolute();

		let cropped = image.view(rect.x, rect.y, rect.width, rect.height);
		let (distance, song_id) = ctx
			.jacket_cache
			.recognise(&*cropped)
			.ok_or_else(|| "Could not recognise jacket")?;

		if distance > 100.0 {
			Err("No known jacket looks like this")?;
		}

		let song = ctx
			.song_cache
			.lock()
			.map_err(|_| "Poisoned song cache")?
			.lookup(*song_id)
			.ok_or_else(|| format!("Could not find song with id {}", song_id))?
			// NOTE: this will reallocate a few strings, but it is what it is
			.clone();

		Ok(song)
	}
	// }}}
}
// }}}
