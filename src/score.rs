#![allow(dead_code)]
use std::fmt::Display;
use std::fs;
use std::io::Cursor;
use std::str::FromStr;
use std::sync::OnceLock;

use edit_distance::edit_distance;
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use num::{traits::Euclid, Rational64};
use poise::serenity_prelude::{
	Attachment, AttachmentId, CreateAttachment, CreateEmbed, CreateEmbedAuthor, Timestamp,
};
use tesseract::{PageSegMode, Tesseract};

use crate::chart::{Chart, Difficulty, Song, SongCache};
use crate::context::{Error, UserContext};
use crate::jacket::IMAGE_VEC_DIM;
use crate::user::User;

// {{{ Score
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score(pub u32);

impl Score {
	// {{{ Score analysis
	// {{{ Mini getters
	#[inline]
	pub fn to_zeta(self, note_count: u32) -> Score {
		self.analyse(note_count).0
	}

	#[inline]
	pub fn shinies(self, note_count: u32) -> u32 {
		self.analyse(note_count).1
	}

	#[inline]
	pub fn units(self, note_count: u32) -> u32 {
		self.analyse(note_count).2
	}
	// }}}

	#[inline]
	pub fn increment(note_count: u32) -> Rational64 {
		Rational64::new_raw(5_000_000, note_count as i64).reduced()
	}

	/// Remove the contribution made by shinies to a score.
	#[inline]
	pub fn forget_shinies(self, note_count: u32) -> Self {
		Self(
			(Self::increment(note_count) * Rational64::from_integer(self.units(note_count) as i64))
				.floor()
				.to_integer() as u32,
		)
	}

	/// Compute a score without making a distinction between shinies and pures. That is, the given
	/// value for `pures` must refer to the sum of `pure` and `shiny` notes.
	///
	/// This is the simplest way to compute a score, and is useful for error analysis.
	#[inline]
	pub fn compute_naive(note_count: u32, pures: u32, fars: u32) -> Self {
		Self(
			(Self::increment(note_count) * Rational64::from_integer((2 * pures + fars) as i64))
				.floor()
				.to_integer() as u32,
		)
	}

	/// Returns the zeta score, the number of shinies, and the number of score units.
	///
	/// Pure (and higher) notes reward two score units, far notes reward one, and lost notes reward
	/// none.
	pub fn analyse(self, note_count: u32) -> (Score, u32, u32) {
		// Smallest possible difference between (zeta-)scores
		let increment = Self::increment(note_count);
		let zeta_increment = Rational64::new_raw(2_000_000, note_count as i64).reduced();

		let score = Rational64::from_integer(self.0 as i64);
		let score_units = (score / increment).floor();

		let non_shiny_score = (score_units * increment).floor();
		let shinies = score - non_shiny_score;

		let zeta_score_units = Rational64::from_integer(2) * score_units + shinies;
		let zeta_score = Score((zeta_increment * zeta_score_units).floor().to_integer() as u32);

		(
			zeta_score,
			shinies.to_integer() as u32,
			score_units.to_integer() as u32,
		)
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
	// {{{ Scores & Distribution => score
	pub fn resolve_ambiguities(
		scores: Vec<Score>,
		read_distribution: Option<(u32, u32, u32)>,
		note_count: u32,
	) -> Result<(Score, Option<u32>, Option<&'static str>), Error> {
		if scores.len() == 0 {
			return Err("No scores in list to disambiguate from.")?;
		}

		let mut no_shiny_scores: Vec<_> = scores
			.iter()
			.map(|score| score.forget_shinies(note_count))
			.collect();
		no_shiny_scores.sort();
		no_shiny_scores.dedup();

		if let Some(read_distribution) = read_distribution {
			let pures = read_distribution.0;
			let fars = read_distribution.1;
			let losts = read_distribution.2;

			// Compute score from note breakdown subpairs
			let pf_score = Score::compute_naive(note_count, pures, fars);
			let fl_score = Score::compute_naive(note_count, note_count - losts - fars, fars);
			let lp_score = Score::compute_naive(note_count, pures, note_count - losts - pures);

			if no_shiny_scores.len() == 1 {
				// {{{ Score is fixed, gotta figure out the exact distribution
				let score = *scores.iter().max().unwrap();

				// {{{ Look for consensus among recomputed scores
				// Lemma: if two computed scores agree, then so will the third
				let consensus_fars = if pf_score == fl_score {
					Some(fars)
				} else {
					// Due to the above lemma, we know all three scores must be distinct by
					// this point.
					//
					// Our strategy is to check which of the three scores agrees with the real
					// score, and to then trust the `far` value that contributed to that pair.
					let no_shiny_score = score.forget_shinies(note_count);
					let pf_appears = no_shiny_score == pf_score;
					let fl_appears = no_shiny_score == fl_score;
					let lp_appears = no_shiny_score == lp_score;

					match (pf_appears, fl_appears, lp_appears) {
						(true, false, false) => Some(fars),
						(false, true, false) => Some(fars),
						(false, false, true) => Some(note_count - pures - losts),
						_ => None,
					}
				};
				// }}}

				if scores.len() == 1 {
					Ok((score, consensus_fars, None))
				} else {
					Ok((score, consensus_fars, Some("Due to a reading error, I could not make sure the shiny-amount I calculated is accurate!")))
				}

			// }}}
			} else {
				// {{{ Score is not fixed, gotta figure out everything at once
				// Some of the values in the note distribution are likely wrong (due to reading
				// errors). To get around this, we take each pair from the triplet, compute the score
				// it induces, and figure out if there's any consensus as to which value in the
				// provided score list is the real one.
				//
				// Note that sometimes the note distribution cannot resolve any of the issues. This is
				// usually the case when the disagreement comes from the number of shinies.

				// {{{ Look for consensus among recomputed scores
				// Lemma: if two computed scores agree, then so will the third
				let (trusted_pure_count, consensus_computed_score, consensus_fars) = if pf_score
					== fl_score
				{
					(true, pf_score, fars)
				} else {
					// Due to the above lemma, we know all three scores must be distinct by
					// this point.
					//
					// Our strategy is to check which of the three scores appear in the
					// provided score list.
					let pf_appears = no_shiny_scores.contains(&pf_score);
					let fl_appears = no_shiny_scores.contains(&fl_score);
					let lp_appears = no_shiny_scores.contains(&lp_score);

					match (pf_appears, fl_appears, lp_appears) {
                        (true, false, false) => (true, pf_score, fars),
                        (false, true, false) => (false, fl_score, fars),
                        (false, false, true) => (true, lp_score, note_count - pures - losts),
                        _ => Err(format!("Cannot disambiguate scores {:?}. Multiple disjoint note breakdown subpair scores appear on the possibility list", scores))?
                    }
				};
				// }}}
				// {{{ Collect all scores that agree with the consensus score.
				let agreement: Vec<_> = scores
					.iter()
					.filter(|score| score.forget_shinies(note_count) == consensus_computed_score)
					.filter(|score| {
						let shinies = score.shinies(note_count);
						shinies <= note_count && (!trusted_pure_count || shinies <= pures)
					})
					.map(|v| *v)
					.collect();
				// }}}
				// {{{ Case 1: Disagreement in the amount of shinies!
				if agreement.len() > 1 {
					let agreement_shiny_amounts: Vec<_> =
						agreement.iter().map(|v| v.shinies(note_count)).collect();

					println!(
						"Shiny count disagreement. Possible scores: {:?}. Possible shiny amounts: {:?}, Read distribution: {:?}",
						scores, agreement_shiny_amounts, read_distribution
					);

					let msg = Some(
                            "Due to a reading error, I could not make sure the shiny-amount I calculated is accurate!"
                            );

					Ok((
						agreement.into_iter().max().unwrap(),
						Some(consensus_fars),
						msg,
					))
				// }}}
				// {{{ Case 2: Total agreement!
				} else if agreement.len() == 1 {
					Ok((agreement[0], Some(consensus_fars), None))
				// }}}
				// {{{ Case 3: No agreement!
				} else {
					Err(format!("Could not disambiguate between possible scores {:?}. Note distribution does not agree with any possibility, leading to a score of {:?}.", scores, consensus_computed_score))?
				}
				// }}}
				// }}}
			}
		} else {
			if no_shiny_scores.len() == 1 {
				if scores.len() == 1 {
					Ok((scores[0], None, None))
				} else {
					Ok((scores.into_iter().max().unwrap(), None, Some("Due to a reading error, I could not make sure the shiny-amount I calculated is accurate!")))
				}
			} else {
				Err("Cannot disambiguate between more than one score without a note distribution.")?
			}
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
			zeta_score: score.to_zeta(chart.note_count as u32),
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

	#[inline]
	pub fn with_fars(mut self, far_count: Option<u32>) -> Self {
		self.far_notes = far_count;
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
// {{{ DbPlay
/// Version of `Play` matching the format sqlx expects
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DbPlay {
	pub id: i64,
	pub chart_id: i64,
	pub user_id: i64,
	pub discord_attachment_id: Option<String>,
	pub score: i64,
	pub zeta_score: i64,
	pub max_recall: Option<i64>,
	pub far_notes: Option<i64>,
	pub created_at: chrono::NaiveDateTime,
	pub creation_ptt: Option<i64>,
	pub creation_zeta_ptt: Option<i64>,
}

impl DbPlay {
	#[inline]
	pub fn to_play(self) -> Play {
		Play {
			id: self.id as u32,
			chart_id: self.chart_id as u32,
			user_id: self.user_id as u32,
			score: Score(self.score as u32),
			zeta_score: Score(self.zeta_score as u32),
			max_recall: self.max_recall.map(|r| r as u32),
			far_notes: self.far_notes.map(|r| r as u32),
			created_at: self.created_at,
			discord_attachment_id: self
				.discord_attachment_id
				.and_then(|s| AttachmentId::from_str(&s).ok()),
			creation_ptt: self.creation_ptt.map(|r| r as u32),
			creation_zeta_ptt: self.creation_zeta_ptt.map(|r| r as u32),
		}
	}
}
// }}}
// {{{ Play
#[derive(Debug, Clone)]
pub struct Play {
	pub id: u32,
	pub chart_id: u32,
	pub user_id: u32,
	pub discord_attachment_id: Option<AttachmentId>,

	// Actual score data
	pub score: Score,
	pub zeta_score: Score,

	// Optional score details
	pub max_recall: Option<u32>,
	pub far_notes: Option<u32>,

	// Creation data
	pub created_at: chrono::NaiveDateTime,
	pub creation_ptt: Option<u32>,
	pub creation_zeta_ptt: Option<u32>,
}

impl Play {
	// {{{ Play => distribution
	pub fn distribution(&self, note_count: u32) -> Option<(u32, u32, u32, u32)> {
		if let Some(fars) = self.far_notes {
			let (_, shinies, units) = self.score.analyse(note_count);
			let (pures, rem) = (units - fars).div_rem_euclid(&2);
			if rem == 1 {
				println!("The impossible happened: got an invalid amount of far notes!");
				return None;
			}

			let lost = note_count - fars - pures;
			let non_max_pures = pures - shinies;
			Some((shinies, non_max_pures, fars, lost))
		} else {
			None
		}
	}
	// }}}
	// {{{ Play => status
	#[inline]
	pub fn status(&self, chart: &Chart) -> Option<String> {
		let score = self.score.0;
		if score >= 10_000_000 {
			// Prevent subtracting with overflow
			if score > chart.note_count + 10_000_000 {
				return None;
			}

			let non_max_pures = chart.note_count + 10_000_000 - score;
			if non_max_pures == 0 {
				Some("MPM".to_string())
			} else {
				Some(format!("PM (-{})", non_max_pures))
			}
		} else if let Some(distribution) = self.distribution(chart.note_count) {
			// if no lost notes...
			if distribution.3 == 0 {
				Some(format!("FR (-{}/-{})", distribution.1, distribution.2))
			} else {
				Some(format!(
					"C (-{}/-{}/-{})",
					distribution.1, distribution.2, distribution.3
				))
			}
		} else {
			None
		}
	}
	// }}}
	// {{{ Play to embed
	/// Creates a discord embed for this play.
	///
	/// The `index` variable is only used to create distinct filenames.
	pub async fn to_embed(
		&self,
		song: &Song,
		chart: &Chart,
		index: usize,
		author: Option<&poise::serenity_prelude::User>,
	) -> Result<(CreateEmbed, Option<CreateAttachment>), Error> {
		let attachement_name = format!("{:?}-{:?}-{:?}.png", song.id, self.score.0, index);
		let icon_attachement = match chart.cached_jacket {
			Some(bytes) => Some(CreateAttachment::bytes(bytes, &attachement_name)),
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
			.field(
				"Status",
				self.status(chart).unwrap_or("?".to_string()),
				true,
			)
			.field("Max recall", "?", true)
			.field("ID", format!("{}", self.id), true);

		if icon_attachement.is_some() {
			embed = embed.thumbnail(format!("attachment://{}", &attachement_name));
		}

		if let Some(user) = author {
			let mut embed_author = CreateEmbedAuthor::new(&user.name);
			if let Some(url) = user.avatar_url() {
				embed_author = embed_author.icon_url(url);
			}

			embed = embed
				.timestamp(Timestamp::from_millis(
					self.created_at.and_utc().timestamp_millis(),
				)?)
				.author(embed_author);
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
				let (zeta_score, computed_shiny_count, units) = score.analyse(note_count);
				let expected_zeta_score = Rational64::from_integer(zeta_score_units as i64)
					* Rational64::new_raw(2000000, note_count as i64).reduced();

				assert_eq!(zeta_score, Score(expected_zeta_score.to_integer() as u32));
				assert_eq!(computed_shiny_count, shiny_count);
				assert_eq!(units, 2 * note_count);
			}
		}
	}
}
// }}}
// }}}
// {{{ Image processing helpers
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

	/// Shift this rect on the y axis by a given absolute pixel amount
	#[inline]
	pub fn shift_y_abs(&self, amount: u32) -> Self {
		let mut res = Self::new(
			self.x,
			self.y + (amount as f32 / self.dimensions.height as f32),
			self.width,
			self.height,
			self.dimensions,
		);
		res.fix();
		res
	}

	/// Clamps the values apropriately
	#[inline]
	pub fn fix(&mut self) {
		self.x = self.x.max(0.);
		self.y = self.y.max(0.);
		self.width = self.width.min(1. - self.x);
		self.height = self.height.min(1. - self.y);
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
		rect.fix();
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
		widen_by(&mut rects, 0.0, 0.0075);
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
			AbsoluteRect::new(76, 172, 77, 18, ImageDimensions::new(1080, 607)).to_relative(),
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
		widen_by(&mut rects, 0.3, 0.0);
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
// {{{ Note distribution
pub fn note_distribution_rects() -> (
	&'static [RelativeRect],
	&'static [RelativeRect],
	&'static [RelativeRect],
) {
	static CELL: OnceLock<(
		&'static [RelativeRect],
		&'static [RelativeRect],
		&'static [RelativeRect],
	)> = OnceLock::new();
	*CELL.get_or_init(|| {
		let mut pure_rects: Vec<RelativeRect> = vec![
			AbsoluteRect::new(729, 523, 58, 22, ImageDimensions::new(1560, 720)).to_relative(),
			AbsoluteRect::new(815, 520, 57, 23, ImageDimensions::new(1600, 720)).to_relative(),
			AbsoluteRect::new(1019, 856, 91, 33, ImageDimensions::new(2000, 1200)).to_relative(),
			AbsoluteRect::new(1100, 1085, 102, 38, ImageDimensions::new(2160, 1620)).to_relative(),
			AbsoluteRect::new(1130, 1118, 105, 39, ImageDimensions::new(2224, 1668)).to_relative(),
			AbsoluteRect::new(1286, 850, 91, 35, ImageDimensions::new(2532, 1170)).to_relative(),
			AbsoluteRect::new(1305, 1125, 117, 44, ImageDimensions::new(2560, 1600)).to_relative(),
			AbsoluteRect::new(1389, 1374, 126, 48, ImageDimensions::new(2732, 2048)).to_relative(),
			AbsoluteRect::new(1407, 933, 106, 40, ImageDimensions::new(2778, 1284)).to_relative(),
		];

		process_datapoints(&mut pure_rects);

		let skip_distances = vec![40, 40, 57, 67, 65, 60, 75, 78, 65];
		let far_rects: Vec<_> = pure_rects
			.iter()
			.enumerate()
			.map(|(i, rect)| rect.shift_y_abs(skip_distances[i]))
			.collect();

		let lost_rects: Vec<_> = far_rects
			.iter()
			.enumerate()
			.map(|(i, rect)| rect.shift_y_abs(skip_distances[i]))
			.collect();

		(pure_rects.leak(), far_rects.leak(), lost_rects.leak())
	})
}
// }}}
// }}}
// }}}
// {{{ Recognise chart name
/// Runs a specialized fuzzy-search through all charts in the game.
///
/// The `unsafe_heuristics` toggle increases the amount of resolvable queries, but might let in
/// some false positives. We turn it on for simple user-search commands, but disallow it for things
/// like OCR-generated text.
pub fn guess_chart_name<'a>(
	raw_text: &str,
	cache: &'a SongCache,
	difficulty: Option<Difficulty>,
	unsafe_heuristics: bool,
) -> Result<(&'a Song, &'a Chart), Error> {
	let raw_text = raw_text.trim(); // not quite raw 🤔
	let mut text: &str = &raw_text.to_lowercase();

	// Cached vec used to store distance calculations
	let mut distance_vec = Vec::with_capacity(3);
	let (song, chart) = loop {
		let mut close_enough: Vec<_> = cache
			.songs()
			.filter_map(|item| {
				let song = &item.song;
				let chart = if let Some(difficulty) = difficulty {
					item.lookup(difficulty).ok()?
				} else {
					item.charts().next()?
				};

				let song_title = song.title.to_lowercase();
				distance_vec.clear();

				let base_distance = edit_distance(&text, &song_title);
				if base_distance < 1.max(song.title.len() / 3) {
					distance_vec.push(base_distance * 10 + 2);
				}

				let shortest_len = Ord::min(song_title.len(), text.len());
				if let Some(sliced) = &song_title.get(..shortest_len)
					&& (text.len() >= 6 || unsafe_heuristics)
				{
					let slice_distance = edit_distance(&text, sliced);
					if slice_distance < 1 {
						distance_vec.push(slice_distance * 10 + 3);
					}
				}

				if let Some(shorthand) = &chart.shorthand
					&& unsafe_heuristics
				{
					let short_distance = edit_distance(&text, shorthand);
					if short_distance < 1.max(shorthand.len() / 3) {
						distance_vec.push(short_distance * 10 + 1);
					}
				}

				distance_vec
					.iter()
					.min()
					.map(|distance| (song, chart, *distance))
			})
			.collect();

		if close_enough.len() == 0 {
			if text.len() <= 1 {
				Err(format!(
					"Could not find match for chart name '{}' [{:?}]",
					raw_text, difficulty
				))?;
			} else {
				text = &text[..text.len() - 1];
			}
		} else if close_enough.len() == 1 {
			break (close_enough[0].0, close_enough[0].1);
		} else {
			if unsafe_heuristics {
				close_enough.sort_by_key(|(_, _, distance)| *distance);
				break (close_enough[0].0, close_enough[0].1);
			} else {
				Err(format!(
					"Name '{}' is too vague to choose a match",
					raw_text
				))?;
			};
		};
	};

	// NOTE: this will reallocate a few strings, but it is what it is
	Ok((song, chart))
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
	pub fn crop_image_to_bytes(
		&mut self,
		image: &DynamicImage,
		rect: AbsoluteRect,
	) -> Result<(), Error> {
		self.bytes.clear();
		let image = image.crop_imm(rect.x, rect.y, rect.width, rect.height);
		let mut cursor = Cursor::new(&mut self.bytes);
		image.write_to(&mut cursor, image::ImageFormat::Png)?;

		fs::write(format!("./logs/{}.png", Timestamp::now()), &self.bytes)?;

		Ok(())
	}

	// {{{ Read score
	pub fn read_score(
		&mut self,
		note_count: Option<u32>,
		image: &DynamicImage,
	) -> Result<Vec<Score>, Error> {
		self.crop_image_to_bytes(
			&image.resize_exact(image.width(), image.height(), FilterType::Nearest),
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), score_rects())
				.ok_or_else(|| "Could not find score area in picture")?
				.to_absolute(),
		)?;

		let mut results = vec![];
		for mode in [
			PageSegMode::PsmSingleWord,
			PageSegMode::PsmRawLine,
			PageSegMode::PsmSingleLine,
			PageSegMode::PsmSparseText,
			PageSegMode::PsmSingleBlock,
		] {
			let result = self.read_score_with_mode(mode, "0123456789'/");
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

	fn read_score_with_mode(&mut self, mode: PageSegMode, whitelist: &str) -> Result<Score, Error> {
		let mut t = Tesseract::new(None, Some("eng"))?
			.set_variable("classify_bln_numeric_mode", "1")?
			.set_variable("tessedit_char_whitelist", whitelist)?
			.set_image_from_mem(&self.bytes)?;
		t.set_page_seg_mode(mode);
		t = t.recognize()?;

		// Disabled, as this was super unreliable
		// let conf = t.mean_text_conf();
		// if conf < 10 && conf != 0 {
		// 	Err(format!(
		// 		"Score text is not readable (confidence = {}, text = {}).",
		// 		conf,
		// 		t.get_text()?.trim()
		// 	))?;
		// }

		let text: String = t.get_text()?.trim().to_string();

		let text: String = text
			.chars()
			.map(|char| if char == '/' { '7' } else { char })
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

		let text: &str = &t.get_text()?;
		let text = text.trim().to_lowercase();

		let conf = t.mean_text_conf();
		if conf < 10 && conf != 0 {
			Err(format!(
				"Difficulty text is not readable (confidence = {}, text = {}).",
				conf, text
			))?;
		}

		let difficulty = Difficulty::DIFFICULTIES
			.iter()
			.zip(Difficulty::DIFFICULTY_STRINGS)
			.min_by_key(|(_, difficulty_string)| edit_distance(difficulty_string, &text))
			.map(|(difficulty, _)| *difficulty)
			.ok_or_else(|| format!("Unrecognised difficulty '{}'", text))?;

		Ok(difficulty)
	}
	// }}}
	// {{{ Read song
	pub fn read_song<'a>(
		&mut self,
		image: &DynamicImage,
		cache: &'a SongCache,
		difficulty: Difficulty,
	) -> Result<(&'a Song, &'a Chart), Error> {
		self.crop_image_to_bytes(
			&image,
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), title_rects())
				.ok_or_else(|| "Could not find title area in picture")?
				.to_absolute(),
		)?;

		let mut t = Tesseract::new(None, Some("eng"))?
			.set_variable(
				"tessedit_char_whitelist",
				"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789,.()- ",
			)?
			.set_image_from_mem(&self.bytes)?;
		t.set_page_seg_mode(PageSegMode::PsmSingleLine);
		t = t.recognize()?;

		let raw_text: &str = &t.get_text()?;

		// let conf = t.mean_text_conf();
		// if conf < 20 && conf != 0 {
		// 	Err(format!(
		// 		"Title text is not readable (confidence = {}, text = {}).",
		// 		conf,
		// 		raw_text.trim()
		// 	))?;
		// }

		guess_chart_name(raw_text, cache, Some(difficulty), false)
	}
	// }}}
	// {{{ Read jacket
	pub async fn read_jacket<'a>(
		&mut self,
		ctx: &'a UserContext,
		image: &DynamicImage,
		difficulty: Difficulty,
	) -> Result<(&'a Song, &'a Chart), Error> {
		let rect =
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), jacket_rects())
				.ok_or_else(|| "Could not find jacket area in picture")?
				.to_absolute();

		let cropped = image.view(rect.x, rect.y, rect.width, rect.height);
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
	pub fn read_distribution(&mut self, image: &DynamicImage) -> Result<(u32, u32, u32), Error> {
		let mut t = Tesseract::new(None, Some("eng"))?
			.set_variable("classify_bln_numeric_mode", "1")?
			.set_variable("tessedit_char_whitelist", "0123456789")?;
		t.set_page_seg_mode(PageSegMode::PsmSingleLine);

		let (pure_rects, far_rects, lost_rects) = note_distribution_rects();
		self.crop_image_to_bytes(
			&image,
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), pure_rects)
				.ok_or_else(|| "Could not find pure-rect area in picture")?
				.to_absolute(),
		)?;

		t = t.set_image_from_mem(&self.bytes)?.recognize()?;
		let pure_notes = u32::from_str(&t.get_text()?.trim()).unwrap_or(0);
		println!("Raw {}", t.get_text()?.trim());

		self.crop_image_to_bytes(
			&image,
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), far_rects)
				.ok_or_else(|| "Could not find far-rect area in picture")?
				.to_absolute(),
		)?;

		t = t.set_image_from_mem(&self.bytes)?.recognize()?;
		let far_notes = u32::from_str(&t.get_text()?.trim()).unwrap_or(0);
		println!("Raw {}", t.get_text()?.trim());

		self.crop_image_to_bytes(
			&image,
			RelativeRect::from_aspect_ratio(ImageDimensions::from_image(image), lost_rects)
				.ok_or_else(|| "Could not find lost-rect area in picture")?
				.to_absolute(),
		)?;

		t = t.set_image_from_mem(&self.bytes)?.recognize()?;
		let lost_notes = u32::from_str(&t.get_text()?.trim()).unwrap_or(0);
		println!("Raw {}", t.get_text()?.trim());

		Ok((pure_notes, far_notes, lost_notes))
	}
	// }}}
}
// }}}
