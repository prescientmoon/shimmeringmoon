//! This module implements a clunky but reliable way of fuzzy-finding an arcaea
//! chart names. This algorithm is left-biased, in case the right half of the
//! name is being covered by character arc.
//!
//! This module also makes use of an
//! extra shorthand system, with shorthands defined in the repo in
//! `data/shorthands.csv` and imported by `scripts/main.py`. The shorthands are
//! useful for non-ascii song names, or when trying to bridge the gap to how
//! the game supposedly refers to some names internally (I do *not* use any
//! databases extracted from the game, but this is still useful for having a
//! "canonical" way to refer to some weirdly-named charts).

use anyhow::bail;

use crate::arcaea::chart::{Chart, Difficulty, Song, SongCache};
use crate::context::{Error, UserContext};
use crate::levenshtein::edit_distance_with;

/// Similar to `.strip_suffix`, but case insensitive
#[inline]
fn strip_case_insensitive_suffix<'a>(string: &'a str, suffix: &str) -> Option<&'a str> {
	let suffix = suffix.to_lowercase();
	if string.to_lowercase().ends_with(&suffix) {
		Some(&string[0..string.len() - suffix.len()])
	} else {
		None
	}
}

// {{{ Guess song and chart by name
pub fn guess_song_and_chart<'a>(
	ctx: &'a UserContext,
	name: &'a str,
) -> Result<(&'a Song, &'a Chart), Error> {
	let mut name = name.trim();
	let mut inferred_difficulty = None;

	for difficulty in Difficulty::DIFFICULTIES {
		for shorthand in [
			Difficulty::DIFFICULTY_SHORTHANDS[difficulty.to_index()],
			Difficulty::DIFFICULTY_SHORTHANDS_IN_BRACKETS[difficulty.to_index()],
		] {
			if let Some(stripped) = strip_case_insensitive_suffix(name, shorthand) {
				inferred_difficulty = Some(difficulty);
				name = stripped;
				break;
			}
		}
	}

	guess_chart_name(name, &ctx.song_cache, inferred_difficulty, true)
}
// }}}
// {{{ Guess chart by name
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
			.charts()
			.filter_map(|chart| {
				let cached_song = &cache.lookup_song(chart.song_id).ok()?;
				let song = &cached_song.song;
				let plausible_difficulty = match difficulty {
					Some(difficulty) => difficulty == chart.difficulty,
					None => {
						let chart_count = cached_song.charts().count();
						chart_count == 1 || chart.difficulty == Difficulty::FTR
					}
				};

				if !plausible_difficulty {
					return None;
				}

				let song_title = &song.lowercase_title;
				distance_vec.clear();

				// Apply raw distance
				let base_distance = edit_distance_with(text, song_title, &mut levenshtein_vec);
				if base_distance <= song.title.len() / 3 {
					distance_vec.push(base_distance * 10 + 2);
				}

				// Cut title to the length of the text, and then check
				let shortest_len = Ord::min(song_title.len(), text.len());
				if let Some(sliced) = &song_title.get(..shortest_len) {
					if text.len() >= 6 || unsafe_heuristics {
						let slice_distance = edit_distance_with(text, sliced, &mut levenshtein_vec);
						if slice_distance == 0 {
							distance_vec.push(3);
						}
					}
				}

				// Shorthand-based matching
				if let Some(shorthand) = &chart.shorthand {
					if unsafe_heuristics {
						let short_distance =
							edit_distance_with(text, shorthand, &mut levenshtein_vec);

						if short_distance <= shorthand.len() / 3 {
							distance_vec.push(short_distance * 10 + 1);
						}
					}
				}

				distance_vec
					.iter()
					.min()
					.map(|distance| (song, chart, *distance))
			})
			.collect();

		close_enough.sort_by_key(|(song, _, _)| song.id);
		close_enough.dedup_by_key(|(song, _, _)| song.id);

		if close_enough.is_empty() {
			if text.len() <= 1 {
				bail!(
					"Could not find match for chart name '{}' [{:?}]",
					raw_text,
					difficulty
				);
			} else {
				text = &text[..text.len() - 1];
			}
		} else if close_enough.len() == 1 {
			break (close_enough[0].0, close_enough[0].1);
		} else if unsafe_heuristics {
			close_enough.sort_by_key(|(_, _, distance)| *distance);
			break (close_enough[0].0, close_enough[0].1);
		} else {
			bail!("Name '{}' is too vague to choose a match", raw_text);
		};
	};

	Ok((song, chart))
}
// }}}
