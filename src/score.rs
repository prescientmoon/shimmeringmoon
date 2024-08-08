#![allow(dead_code)]
use std::fmt::Display;
use std::fs;
use std::io::Cursor;
use std::str::FromStr;

use image::{imageops::FilterType, DynamicImage, GenericImageView};
use num::integer::Roots;
use num::{traits::Euclid, Rational64};
use poise::serenity_prelude::{
	Attachment, AttachmentId, CreateAttachment, CreateEmbed, CreateEmbedAuthor, Timestamp,
};
use sqlx::{query_as, SqlitePool};
use tesseract::{PageSegMode, Tesseract};

use crate::bitmap::{Color, Rect};
use crate::chart::{Chart, Difficulty, Song, SongCache, DIFFICULTY_MENU_PIXEL_COLORS};
use crate::context::{Error, UserContext};
use crate::image::rotate;
use crate::jacket::IMAGE_VEC_DIM;
use crate::levenshtein::{edit_distance, edit_distance_with};
use crate::ocr::ui::{ScoreScreenRect, SongSelectRect, UIMeasurementRect};
use crate::user::User;

// {{{ Grade
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
	EXP,
	EX,
	AA,
	A,
	B,
	C,
	D,
}

impl Grade {
	pub const GRADE_STRINGS: [&'static str; 7] = ["EX+", "EX", "AA", "A", "B", "C", "D"];
	pub const GRADE_SHORTHANDS: [&'static str; 7] = ["exp", "ex", "aa", "a", "b", "c", "d"];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

impl Display for Grade {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", Self::GRADE_STRINGS[self.to_index()])
	}
}
// }}}
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

	#[inline]
	pub fn play_rating_f32(self, chart_constant: u32) -> f32 {
		(self.play_rating(chart_constant)) as f32 / 100.0
	}
	// }}}
	// {{{ Score => grade
	#[inline]
	// TODO: Perhaps make an enum for this
	pub fn grade(self) -> Grade {
		let score = self.0;
		if score > 9900000 {
			Grade::EXP
		} else if score > 9800000 {
			Grade::EX
		} else if score > 9500000 {
			Grade::AA
		} else if score > 9200000 {
			Grade::A
		} else if score > 8900000 {
			Grade::B
		} else if score > 8600000 {
			Grade::C
		} else {
			Grade::D
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
			let fl_score = Score::compute_naive(
				note_count,
				note_count.checked_sub(losts + fars).unwrap_or(0),
				fars,
			);
			let lp_score = Score::compute_naive(
				note_count,
				pures,
				note_count.checked_sub(losts + pures).unwrap_or(0),
			);

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
			let (pures, rem) = units.checked_sub(fars)?.div_rem_euclid(&2);
			if rem == 1 {
				println!("The impossible happened: got an invalid amount of far notes!");
				return None;
			}

			let lost = note_count.checked_sub(fars + pures)?;
			let non_max_pures = pures.checked_sub(shinies)?;
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
			if score > chart.note_count + 10_000_000 {
				return None;
			}

			let non_max_pures = (chart.note_count + 10_000_000).checked_sub(score)?;
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

	#[inline]
	pub fn short_status(&self, chart: &Chart) -> Option<char> {
		let score = self.score.0;
		if score >= 10_000_000 {
			let non_max_pures = (chart.note_count + 10_000_000).checked_sub(score)?;
			if non_max_pures == 0 {
				Some('M')
			} else {
				Some('P')
			}
		} else if let Some(distribution) = self.distribution(chart.note_count)
			&& distribution.3 == 0
		{
			Some('F')
		} else {
			Some('C')
		}
	}
	// }}}
	// {{{ Play to embed
	/// Creates a discord embed for this play.
	///
	/// The `index` variable is only used to create distinct filenames.
	pub async fn to_embed(
		&self,
		db: &SqlitePool,
		user: &User,
		song: &Song,
		chart: &Chart,
		index: usize,
		author: Option<&poise::serenity_prelude::User>,
	) -> Result<(CreateEmbed, Option<CreateAttachment>), Error> {
		// {{{ Get previously best score
		let previously_best = query_as!(
			DbPlay,
			"
        SELECT * FROM plays
        WHERE user_id=?
        AND chart_id=?
        AND created_at<?
        ORDER BY score DESC
    ",
			user.id,
			chart.id,
			self.created_at
		)
		.fetch_optional(db)
		.await
		.map_err(|_| {
			format!(
				"Could not find any scores for {} [{:?}]",
				song.title, chart.difficulty
			)
		})?
		.map(|p| p.to_play());
		// }}}

		let attachement_name = format!("{:?}-{:?}-{:?}.png", song.id, self.score.0, index);
		let icon_attachement = match chart.cached_jacket.as_ref() {
			Some(jacket) => Some(CreateAttachment::bytes(jacket.raw, &attachement_name)),
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
					self.score.play_rating_f32(chart.chart_constant)
				),
				true,
			)
			.field("Grade", format!("{}", self.score.grade()), true)
			.field("ξ-Score", format!("{} (+?)", self.zeta_score), true)
			// {{{ ξ-Rating
			.field(
				"ξ-Rating",
				{
					let play_rating = self.zeta_score.play_rating_f32(chart.chart_constant);
					if let Some(previous) = previously_best {
						let previous_play_rating =
							previous.zeta_score.play_rating_f32(chart.chart_constant);

						if play_rating >= previous_play_rating {
							format!(
								"{:.2} (+{})",
								play_rating,
								play_rating - previous_play_rating
							)
						} else {
							format!(
								"{:.2} (-{})",
								play_rating,
								play_rating - previous_play_rating
							)
						}
					} else {
						format!("{:.2}", play_rating)
					}
				},
				true,
			)
			// }}}
			.field("ξ-Grade", format!("{}", self.zeta_score.grade()), true)
			.field(
				"Status",
				self.status(chart).unwrap_or("-".to_string()),
				true,
			)
			.field("Max recall", "—", true)
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
// {{{ Score image kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreKind {
	SongSelect,
	ScoreScreen,
}
// }}}
// {{{ Recognise chart
fn strip_case_insensitive_suffix<'a>(string: &'a str, suffix: &str) -> Option<&'a str> {
	let suffix = suffix.to_lowercase();
	if string.to_lowercase().ends_with(&suffix) {
		Some(&string[0..string.len() - suffix.len()])
	} else {
		None
	}
}

pub fn guess_song_and_chart<'a>(
	ctx: &'a UserContext,
	name: &'a str,
) -> Result<(&'a Song, &'a Chart), Error> {
	let name = name.trim();
	let (name, difficulty) = name
		.strip_suffix("PST")
		.zip(Some(Difficulty::PST))
		.or_else(|| strip_case_insensitive_suffix(name, "[PST]").zip(Some(Difficulty::PST)))
		.or_else(|| strip_case_insensitive_suffix(name, "PRS").zip(Some(Difficulty::PRS)))
		.or_else(|| strip_case_insensitive_suffix(name, "[PRS]").zip(Some(Difficulty::PRS)))
		.or_else(|| strip_case_insensitive_suffix(name, "FTR").zip(Some(Difficulty::FTR)))
		.or_else(|| strip_case_insensitive_suffix(name, "[FTR]").zip(Some(Difficulty::FTR)))
		.or_else(|| strip_case_insensitive_suffix(name, "ETR").zip(Some(Difficulty::ETR)))
		.or_else(|| strip_case_insensitive_suffix(name, "[ETR]").zip(Some(Difficulty::ETR)))
		.or_else(|| strip_case_insensitive_suffix(name, "BYD").zip(Some(Difficulty::BYD)))
		.or_else(|| strip_case_insensitive_suffix(name, "[BYD]").zip(Some(Difficulty::BYD)))
		.unwrap_or((&name, Difficulty::FTR));

	guess_chart_name(name, &ctx.song_cache, Some(difficulty), true)
}

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

	// Cached vec used by the levenshtein distance function
	let mut levenshtein_vec = Vec::with_capacity(20);
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

				let song_title = &song.lowercase_title;
				distance_vec.clear();

				let base_distance = edit_distance_with(&text, &song_title, &mut levenshtein_vec);
				if base_distance < 1.max(song.title.len() / 3) {
					distance_vec.push(base_distance * 10 + 2);
				}

				let shortest_len = Ord::min(song_title.len(), text.len());
				if let Some(sliced) = &song_title.get(..shortest_len)
					&& (text.len() >= 6 || unsafe_heuristics)
				{
					let slice_distance = edit_distance_with(&text, sliced, &mut levenshtein_vec);
					if slice_distance < 1 {
						distance_vec.push(slice_distance * 10 + 3);
					}
				}

				if let Some(shorthand) = &chart.shorthand
					&& unsafe_heuristics
				{
					let short_distance = edit_distance_with(&text, shorthand, &mut levenshtein_vec);
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
	pub fn crop_image_to_bytes(&mut self, image: &DynamicImage, rect: Rect) -> Result<(), Error> {
		self.bytes.clear();
		let image = image.crop_imm(rect.x as u32, rect.y as u32, rect.width, rect.height);
		let mut cursor = Cursor::new(&mut self.bytes);
		image.write_to(&mut cursor, image::ImageFormat::Png)?;

		fs::write(format!("./logs/{}.png", Timestamp::now()), &self.bytes)?;

		Ok(())
	}

	// {{{ Read score
	pub fn read_score(
		&mut self,
		ctx: &UserContext,
		note_count: Option<u32>,
		image: &DynamicImage,
		kind: ScoreKind,
	) -> Result<Vec<Score>, Error> {
		println!("kind {kind:?}");
		self.crop_image_to_bytes(
			&image.resize_exact(image.width(), image.height(), FilterType::Nearest),
			ctx.ui_measurements.interpolate(
				if kind == ScoreKind::ScoreScreen {
					UIMeasurementRect::ScoreScreen(ScoreScreenRect::Score)
				} else {
					UIMeasurementRect::SongSelect(SongSelectRect::Score)
				},
				image,
			)?,
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
							UIMeasurementRect::SongSelect(match d {
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

		self.crop_image_to_bytes(
			image,
			ctx.ui_measurements.interpolate(
				UIMeasurementRect::ScoreScreen(ScoreScreenRect::Difficulty),
				image,
			)?,
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
	// {{{ Read score kind
	pub fn read_score_kind(
		&mut self,
		ctx: &UserContext,
		image: &DynamicImage,
	) -> Result<ScoreKind, Error> {
		self.crop_image_to_bytes(
			&image,
			ctx.ui_measurements
				.interpolate(UIMeasurementRect::PlayKind, image)?,
		)?;

		let mut t = Tesseract::new(None, Some("eng"))?.set_image_from_mem(&self.bytes)?;
		t.set_page_seg_mode(PageSegMode::PsmRawLine);
		t = t.recognize()?;

		let text: &str = &t.get_text()?;
		let text = text.trim().to_lowercase();

		let conf = t.mean_text_conf();
		if conf < 10 && conf != 0 {
			Err(format!(
				"Score kind text is not readable (confidence = {}, text = {}).",
				conf, text
			))?;
		}

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
		self.crop_image_to_bytes(
			&image,
			ctx.ui_measurements.interpolate(
				UIMeasurementRect::ScoreScreen(ScoreScreenRect::Title),
				image,
			)?,
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

		guess_chart_name(raw_text, &ctx.song_cache, Some(difficulty), false)
	}
	// }}}
	// {{{ Read jacket
	pub async fn read_jacket<'a>(
		&mut self,
		ctx: &'a UserContext,
		image: &mut DynamicImage,
		kind: ScoreKind,
		difficulty: Difficulty,
		out_rect: &mut Option<Rect>,
	) -> Result<(&'a Song, &'a Chart), Error> {
		let rect = ctx.ui_measurements.interpolate(
			if kind == ScoreKind::ScoreScreen {
				UIMeasurementRect::ScoreScreen(ScoreScreenRect::Jacket)
			} else {
				UIMeasurementRect::SongSelect(SongSelectRect::Jacket)
			},
			image,
		)?;

		let cropped = if kind == ScoreKind::ScoreScreen {
			*out_rect = Some(rect);
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

			*out_rect = Some(Rect::new(rect.x, rect.y + rect.height as i32, len, len));
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
		let mut t = Tesseract::new(None, Some("eng"))?
			.set_variable("classify_bln_numeric_mode", "1")?
			.set_variable("tessedit_char_whitelist", "0123456789")?;
		t.set_page_seg_mode(PageSegMode::PsmSparseText);

		self.crop_image_to_bytes(
			&image,
			ctx.ui_measurements
				.interpolate(UIMeasurementRect::ScoreScreen(ScoreScreenRect::Pure), image)?,
		)?;

		t = t.set_image_from_mem(&self.bytes)?.recognize()?;
		let pure_notes = u32::from_str(&t.get_text()?.trim()).unwrap_or(0);
		println!("Raw {}", t.get_text()?.trim());

		self.crop_image_to_bytes(
			&image,
			ctx.ui_measurements
				.interpolate(UIMeasurementRect::ScoreScreen(ScoreScreenRect::Far), image)?,
		)?;

		t = t.set_image_from_mem(&self.bytes)?.recognize()?;
		let far_notes = u32::from_str(&t.get_text()?.trim()).unwrap_or(0);
		println!("Raw {}", t.get_text()?.trim());

		self.crop_image_to_bytes(
			&image,
			ctx.ui_measurements
				.interpolate(UIMeasurementRect::ScoreScreen(ScoreScreenRect::Lost), image)?,
		)?;

		t = t.set_image_from_mem(&self.bytes)?.recognize()?;
		let lost_notes = u32::from_str(&t.get_text()?.trim()).unwrap_or(0);
		println!("Raw {}", t.get_text()?.trim());

		Ok((pure_notes, far_notes, lost_notes))
	}
	// }}}
}
// }}}
