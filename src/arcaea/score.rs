use std::fmt::Display;

use num::Rational64;

use crate::context::Error;

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
