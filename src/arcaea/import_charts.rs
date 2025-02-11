use std::collections::HashMap;

use anyhow::{anyhow, Context};
use serde::Deserialize;

use crate::{
	arcaea::{chart::Side, rating::rating_as_fixed},
	context::paths::ShimmeringPaths,
};

use super::{
	chart::{Difficulty, Level},
	rating::{rating_from_fixed, Rating},
};

// {{{ Notecount
struct NotecountEntry {
	difficulty: Difficulty,
	level: Level,
	name: String,
	notecount: u32,
}

pub const NOTECOUNT_DATA: &[u8] = include_bytes!("notecounts.csv");

fn get_notecount_records() -> anyhow::Result<Vec<NotecountEntry>> {
	let mut entries = Vec::new();
	let mut reader = csv::Reader::from_reader(std::io::Cursor::new(NOTECOUNT_DATA));

	for result in reader.records() {
		let record = result?;

		let notecount = record
			.get(0)
			.ok_or_else(|| anyhow!("Missing notecount in csv entry"))?
			.parse()?;

		let raw_difficulty = record
			.get(1)
			.ok_or_else(|| anyhow!("Missing level/difficulty in csv entry"))?;

		let name = record
			.get(2)
			.ok_or_else(|| anyhow!("Missing name in csv entry"))?;

		let (raw_difficulty, raw_level) = raw_difficulty
			.split_once(" ")
			.ok_or_else(|| anyhow!("Invalid level/difficulty string in csv entry"))?;

		entries.push(NotecountEntry {
			notecount,
			name: name.to_owned(),
			level: raw_level.parse()?,
			difficulty: raw_difficulty.parse()?,
		});
	}

	Ok(entries)
}
// }}}
// {{{ PTT entries
#[derive(Clone, Copy, Deserialize)]
struct PTTEntry {
	#[serde(rename = "0")]
	pst: Option<f32>,
	#[serde(rename = "1")]
	prs: Option<f32>,
	#[serde(rename = "2")]
	ftr: Option<f32>,
	#[serde(rename = "3")]
	byd: Option<f32>,
	#[serde(rename = "4")]
	etr: Option<f32>,
}

impl PTTEntry {
	fn get_rating(&self, difficulty: Difficulty) -> Option<Rating> {
		let float = match difficulty {
			Difficulty::PST => self.pst,
			Difficulty::PRS => self.prs,
			Difficulty::FTR => self.ftr,
			Difficulty::BYD => self.byd,
			Difficulty::ETR => self.etr,
		};

		float.map(|f| rating_from_fixed((f * 100.0).round() as i32))
	}
}

fn get_ptt_entries(paths: &ShimmeringPaths) -> anyhow::Result<HashMap<String, PTTEntry>> {
	let result = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(
		paths.cc_data_path(),
	)?))?;

	Ok(result)
}
// }}}
// {{{ Songlist types
#[derive(Deserialize)]
struct LocalizedName {
	en: String,
	og: Option<String>,
}

impl LocalizedName {
	fn get(&self) -> &str {
		self.og.as_ref().unwrap_or(&self.en)
	}
}

#[derive(Deserialize)]
struct Chart {
	rating: u8,
	#[serde(default, rename = "ratingPlus")]
	rating_plus: bool,
	#[serde(rename = "ratingClass")]
	difficulty: u8,

	#[serde(rename = "chartDesigner")]
	chart_designer: String,

	#[allow(unused)]
	#[serde(rename = "jacketDesigner")]
	jacket_designer: String,

	#[serde(rename = "title_localized")]
	title: Option<LocalizedName>,
}

#[derive(Deserialize)]
struct Song {
	#[serde(rename = "idx")]
	id: u32,
	#[serde(rename = "id")]
	shorthand: String,
	#[serde(rename = "title_localized")]
	title: LocalizedName,

	artist: String,
	bpm: String,
	side: u32,
	difficulties: Vec<Chart>,
}

#[derive(Deserialize)]
struct DeletedSong {
	#[allow(unused)]
	deleted: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SonglistEntry {
	Song(Song),

	#[allow(unused)]
	Deleted(DeletedSong),
}

#[derive(Deserialize)]
struct Songlist {
	songs: Vec<SonglistEntry>,
}
// }}}
// {{{ Process songlist file
pub fn import_songlist(
	paths: &ShimmeringPaths,
	conn: &mut rusqlite::Connection,
) -> anyhow::Result<()> {
	let notecount_records = get_notecount_records().context("Failed to read notecount records")?;
	let ptt_entries = get_ptt_entries(paths).context("Failed to read ptt entries")?;

	let transaction = conn.transaction()?;
	transaction.execute("DELETE FROM charts", ())?;
	transaction.execute("DELETE FROM songs", ())?;

	let songlist: Songlist = serde_json::from_reader(std::io::BufReader::new(
		std::fs::File::open(paths.songlist_path())?,
	))?;

	let mut song_count = 0;
	let mut chart_count = 0;

	for song in songlist.songs {
		let song = match song {
			SonglistEntry::Song(song) => song,
			SonglistEntry::Deleted(_) => continue,
		};

		song_count += 1;
		transaction.execute(
			"
        INSERT INTO songs(id,title,shorthand,artist,side,bpm)
        VALUES (?,?,?,?,?,?)
      ",
			(
				song.id,
				song.title.get(),
				&song.shorthand,
				&song.artist,
				Side::SIDES[song.side as usize],
				song.bpm,
			),
		)?;

		for chart in song.difficulties {
			if chart.rating == 0 {
				continue;
			}

			chart_count += 1;

			let difficulty = crate::private_server::decode_difficulty(chart.difficulty)
				.ok_or_else(|| anyhow!("Invalid difficulty"))?;

			let level = format!(
				"{}{}",
				chart.rating,
				if chart.rating_plus { "+" } else { "" }
			)
			.parse()
			.context("Failed to parse level")?;

			let name = chart.title.as_ref().unwrap_or(&song.title).get();
			let notecount = notecount_records
				.iter()
				.find_map(|record| {
					let names_match = record.name == name
						|| record.name == format!("{name} ({})", &song.artist)
						|| record.name == song.shorthand;

					if names_match && record.level == level && record.difficulty == difficulty {
						Some(record.notecount)
					} else {
						None
					}
				})
				.ok_or_else(|| {
					anyhow!(
						"Cannot find note count for song '{}' [{}]",
						name,
						difficulty
					)
				})?;

			let cc = ptt_entries
				.get(&song.shorthand)
				.ok_or_else(|| anyhow!("Cannot find PTT data for song '{}'", song.shorthand))?
				.get_rating(difficulty)
				.ok_or_else(|| {
					anyhow!("Cannot find PTT data for song '{}' [{}]", name, difficulty)
				})?;

			transaction.execute(
				"
          INSERT INTO charts(
            song_id, title, difficulty,
            level, note_count, chart_constant,
            note_design
          ) VALUES(?,?,?,?,?,?,?)
        ",
				(
					song.id,
					chart.title.as_ref().map(|t| t.get()),
					difficulty,
					level,
					notecount,
					rating_as_fixed(cc),
					chart.chart_designer,
				),
			)?;
		}
	}

	transaction.commit()?;

	println!("✅ Succesfully imported {chart_count} charts, {song_count} songs");

	Ok(())
}
// }}}
