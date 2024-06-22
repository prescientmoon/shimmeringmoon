#![allow(dead_code)]
use std::{io::Cursor, sync::OnceLock, time::Instant};

use image::DynamicImage;
use num::Rational64;
use tesseract::{PageSegMode, Tesseract};

use crate::{
	chart::{Chart, Difficulty},
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
					lerp(p, low.y, high.y) - 0.005,
					lerp(p, low.width, high.width),
					lerp(p, low.height, high.height) + 2. * 0.005,
					dimensions,
				));
			}
		}

		None
	}
}
// }}}
// {{{ Data points
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

		rects
	})
}

fn difficulty_rects() -> &'static [RelativeRect] {
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
		rects.sort_by_key(|r| (r.dimensions.aspect_ratio() * 1000.0).floor() as u32);
		rects
	})
}
// }}}
// {{{ Plays
/// Returns the zeta score and the number of shinies
pub fn score_to_zeta_score(score: u32, note_count: u32) -> (u32, u32) {
	// Smallest possible difference between (zeta-)scores
	let increment = Rational64::new_raw(5000000, note_count as i64).reduced();
	let zeta_increment = Rational64::new_raw(2000000, note_count as i64).reduced();

	let score = Rational64::from_integer(score as i64);
	let score_units = (score / increment).floor();

	let non_shiny_score = (score_units * increment).floor();
	let shinies = score - non_shiny_score;

	let zeta_score_units = Rational64::from_integer(2) * score_units + shinies;
	let zeta_score = (zeta_increment * zeta_score_units).floor().to_integer() as u32;

	(zeta_score, shinies.to_integer() as u32)
}

// {{{ Create play
#[derive(Debug, Clone)]
pub struct CreatePlay {
	chart_id: u32,
	user_id: u32,
	discord_attachment_id: Option<String>,

	// Actual score data
	score: u32,
	zeta_score: Option<u32>,

	// Optional score details
	max_recall: Option<u32>,
	far_notes: Option<u32>,

	// Creation data
	creation_ptt: Option<u32>,
	creation_zeta_ptt: Option<u32>,
}

impl CreatePlay {
	#[inline]
	pub fn new(score: u32, chart: Chart, user: User) -> Self {
		Self {
			chart_id: chart.id,
			user_id: user.id,
			discord_attachment_id: None,
			score,
			zeta_score: Some(score_to_zeta_score(score, chart.note_count).0),
			max_recall: None,
			far_notes: None,
			// TODO: populate these
			creation_ptt: None,
			creation_zeta_ptt: None,
		}
	}

	pub async fn save(self, ctx: &UserContext) -> Result<Play, Error> {
		let play = sqlx::query_as!(
			Play,
			"
            INSERT INTO plays(
               user_id,chart_id,discord_attachment_id,
               score,zeta_score,max_recall,far_notes
            )
            VALUES(?,?,?,?,?,?,?)
            RETURNING *
            ",
			self.user_id,
			self.chart_id,
			self.discord_attachment_id,
			self.score,
			self.zeta_score,
			self.max_recall,
			self.far_notes
		)
		.fetch_one(&ctx.db)
		.await?;

		Ok(play)
	}
}
// }}}
// {{{ Play
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Play {
	id: i64,
	chart_id: i64,
	user_id: i64,
	discord_attachment_id: Option<String>,

	// Actual score data
	score: i64,
	zeta_score: Option<i64>,

	// Optional score details
	max_recall: Option<i64>,
	far_notes: Option<i64>,

	// Creation data
	created_at: chrono::NaiveDateTime,
	creation_ptt: Option<i64>,
	creation_zeta_ptt: Option<i64>,
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
				let score = 10000000 + shiny_count;
				let zeta_score_units = 4 * (note_count - shiny_count) + 5 * shiny_count;
				let expected_zeta_score = Rational64::from_integer(zeta_score_units as i64)
					* Rational64::new_raw(2000000, note_count as i64).reduced();
				let (zeta_score, computed_shiny_count) = score_to_zeta_score(score, note_count);
				assert_eq!(zeta_score, expected_zeta_score.to_integer() as u32);
				assert_eq!(computed_shiny_count, shiny_count);
			}
		}
	}
}
// }}}
// }}}
// {{{ Ocr types
#[derive(Debug, Clone, Copy)]
pub struct ScoreReadout {
	pub score: u32,
	pub difficulty: Difficulty,
}

impl ScoreReadout {
	pub fn new(score: u32, difficulty: Difficulty) -> Self {
		Self { score, difficulty }
	}
}
// }}}
// {{{ Run OCR
/// Caches a byte vector in order to prevent reallocation
#[derive(Debug, Clone, Default)]
pub struct ImageCropper {
	/// cached byte array
	pub bytes: Vec<u8>,
}

impl ImageCropper {
	fn crop_image_to_bytes(
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

	pub fn read_score(&mut self, image: &DynamicImage) -> Result<ScoreReadout, Error> {
		let rect =
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), score_rects())
				.ok_or_else(|| "Could not find score area in picture")?
				.to_absolute();
		self.crop_image_to_bytes(&image, rect)?;

		let mut t = Tesseract::new(None, Some("eng"))?
			// .set_variable("classify_bln_numeric_mode", "1'")?
			.set_variable("tessedit_char_whitelist", "0123456789'")?
			.set_image_from_mem(&self.bytes)?;

		t.set_page_seg_mode(PageSegMode::PsmRawLine);
		t = t.recognize()?;

		if t.mean_text_conf() < 10 {
			Err("Score text is not readable.")?;
		}

		let text: String = t
			.get_text()?
			.trim()
			.chars()
			.filter(|char| *char != ' ' && *char != '\'')
			.collect();

		let int = u32::from_str_radix(&text, 10)?;
		Ok(ScoreReadout::new(int, Difficulty::FTR))
	}
}
// }}}
