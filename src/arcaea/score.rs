// {{{ Imports
use std::fmt::{Display, Write};

use num::{Rational32, Rational64};

use crate::context::Error;

use super::chart::Chart;
use super::rating::{rating_as_float, rating_from_fixed, Rating};
// }}}

// {{{ Scoring system
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum ScoringSystem {
	Standard,

	/// Forgives up to 9 missed shinies, then uses EX scoring.
	/// PMs correspond to SDPMs.
	SDF,

	/// Inspired by sdvx's EX-scoring.
	/// PMs correspond to MPMs.
	EX,
}

impl ScoringSystem {
	pub const SCORING_SYSTEMS: [Self; 3] = [Self::Standard, Self::SDF, Self::EX];

	/// Values used inside sqlite
	pub const SCORING_SYSTEM_DB_STRINGS: [&'static str; Self::SCORING_SYSTEMS.len()] =
		["standard", "sdf", "ex"];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

impl Default for ScoringSystem {
	fn default() -> Self {
		Self::Standard
	}
}
// }}}
// {{{ Grade
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
	D,
	C,
	B,
	A,
	AA,
	EX,
	EXP,
}

impl Grade {
	pub const GRADE_STRINGS: [&'static str; 7] = ["D", "C", "B", "A", "AA", "EX", "EX+"];
	pub const GRADE_SHORTHANDS: [&'static str; 7] = ["d", "c", "b", "a", "aa", "ex", "exp"];

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
		Self(self.0 - self.shinies(note_count))
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
	// {{{ Scoring system conversion
	/// Convert a standard score to any other scoring system. The output might be
	/// nonsense if the given score is not using the standard system.
	#[inline]
	pub fn convert_to(self, scoring_system: ScoringSystem, chart: &Chart) -> Self {
		match scoring_system {
			ScoringSystem::Standard => self,
			ScoringSystem::SDF => {
				let shinies = self.shinies(chart.note_count);
				Self(self.0 + 9.min(chart.note_count - shinies)).to_zeta(chart.note_count)
			}
			ScoringSystem::EX => self.to_zeta(chart.note_count),
		}
	}

	// }}}
	// {{{ Score => Play rating
	#[inline]
	pub fn play_rating(self, chart_constant: u32) -> Rating {
		rating_from_fixed(chart_constant as i32)
			+ if self.0 >= 10_000_000 {
				Rational32::from_integer(2)
			} else if self.0 >= 9_800_000 {
				Rational32::from_integer(1)
					+ Rational32::new(self.0 as i32 - 9_800_000, 200_000).reduced()
			} else {
				Rational32::new(self.0 as i32 - 9_500_000, 300_000).reduced()
			}
	}

	pub fn display_play_rating(self, prev: Option<Self>, chart: &Chart) -> Result<String, Error> {
		let mut buffer = String::with_capacity(14);

		let play_rating = rating_as_float(self.play_rating(chart.chart_constant));
		write!(buffer, "{:.2}", play_rating)?;

		if let Some(prev) = prev {
			let prev_play_rating = rating_as_float(prev.play_rating(chart.chart_constant));

			if play_rating >= prev_play_rating {
				write!(buffer, " (+{:.2})", play_rating - prev_play_rating)?;
			} else {
				write!(buffer, " ({:.2})", play_rating - prev_play_rating)?;
			}
		}

		Ok(buffer)
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
	pub fn resolve_distibution_ambiguities(
		score: Score,
		read_distribution: Option<(u32, u32, u32)>,
		note_count: u32,
	) -> Option<u32> {
		let read_distribution = read_distribution?;
		let pures = read_distribution.0;
		let fars = read_distribution.1;
		let losts = read_distribution.2;

		// {{{ Compute score from note breakdown subpairs
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
		// }}}
		// {{{ Look for consensus among recomputed scores
		// Lemma: if two computed scores agree, then so will the third
		if pf_score == fl_score {
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
		}
		// }}}
	}
	// }}}
	// {{{ Display self with diff
	/// Similar to the display implementation, but without the padding
	/// to at least 7 digits.
	fn display_mini_into(self, buffer: &mut String) -> Result<(), Error> {
		let score = self.0;
		if self.0 < 1_000 {
			write!(buffer, "{}", score)?;
		} else if self.0 < 1_000_000 {
			write!(buffer, "{}'{:0>3}", (score / 1000), score % 1000)?;
		} else {
			write!(buffer, "{}", self)?;
		}

		Ok(())
	}

	pub fn display_with_diff(self, prev: Option<Self>) -> Result<String, Error> {
		let mut buffer = String::with_capacity(24);
		write!(buffer, "{}", self)?;

		if let Some(prev) = prev {
			write!(buffer, " (")?;
			if self >= prev {
				write!(buffer, "+")?;
				Score(self.0 - prev.0).display_mini_into(&mut buffer)?;
			} else {
				write!(buffer, "-")?;
				Score(prev.0 - self.0).display_mini_into(&mut buffer)?;
			}
			write!(buffer, ")")?;
		}

		Ok(buffer)
	}
	// }}}
	// {{{ PM detection
	#[inline]
	pub fn is_pm(&self) -> bool {
		self.0 >= 10_000_000
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
// {{{ Tests
#[cfg(test)]
mod score_tests {
	use super::*;

	#[test]
	fn zeta_score_consistent_with_pms() {
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
