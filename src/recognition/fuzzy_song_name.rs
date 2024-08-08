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
				return Err(format!("Name '{}' is too vague to choose a match", raw_text).into());
			};
		};
	};

	Ok((song, chart))
}
// }}}
