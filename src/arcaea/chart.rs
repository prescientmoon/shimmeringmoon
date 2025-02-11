use std::str::FromStr;
// {{{ Imports
use std::{fmt::Display, num::NonZeroU16};

use anyhow::{anyhow, bail};
use image::{ImageBuffer, Rgb};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::ToSql;
use serde::{Deserialize, Serialize};

use crate::bitmap::Color;
use crate::context::Error;
// }}}

// {{{ Difficuly
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Difficulty {
	PST,
	PRS,
	FTR,
	ETR,
	BYD,
}

impl Difficulty {
	pub const DIFFICULTIES: [Difficulty; 5] =
		[Self::PST, Self::PRS, Self::FTR, Self::ETR, Self::BYD];

	pub const DIFFICULTY_SHORTHANDS: [&'static str; 5] = ["PST", "PRS", "FTR", "ETR", "BYD"];
	pub const DIFFICULTY_SHORTHANDS_IN_BRACKETS: [&'static str; 5] =
		["[PST]", "[PRS]", "[FTR]", "[ETR]", "[BYD]"];
	pub const DIFFICULTY_STRINGS: [&'static str; 5] =
		["PAST", "PRESENT", "FUTURE", "ETERNAL", "BEYOND"];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

impl FromStr for Difficulty {
	type Err = anyhow::Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		for (i, level) in Self::DIFFICULTY_SHORTHANDS.iter().enumerate() {
			if *level == s {
				return Ok(Self::DIFFICULTIES[i]);
			}
		}

		bail!("Invalid level '{s}'");
	}
}

impl FromSql for Difficulty {
	fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
		let str: String = rusqlite::types::FromSql::column_result(value)?;

		for (i, s) in Self::DIFFICULTY_SHORTHANDS.iter().enumerate() {
			if str == **s {
				return Ok(Self::DIFFICULTIES[i]);
			}
		}

		FromSqlResult::Err(FromSqlError::Other(
			format!("Cannot convert {} to difficulty", str).into(),
		))
	}
}

impl ToSql for Difficulty {
	fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
		Self::DIFFICULTY_SHORTHANDS[*self as usize].to_sql()
	}
}

impl Display for Difficulty {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", Self::DIFFICULTY_SHORTHANDS[self.to_index()])
	}
}

pub const DIFFICULTY_MENU_PIXEL_COLORS: [Color; Difficulty::DIFFICULTIES.len()] = [
	Color::from_rgb_int(0xAAE5F7),
	Color::from_rgb_int(0xBFDD85),
	Color::from_rgb_int(0xCB74AB),
	Color::from_rgb_int(0xC4B7D3),
	Color::from_rgb_int(0xF89AAC),
];
// }}}
// {{{ Level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Level {
	Unknown,
	One,
	Two,
	Three,
	Four,
	Five,
	Six,
	Seven,
	SevenP,
	Eight,
	EightP,
	Nine,
	NineP,
	Ten,
	TenP,
	Eleven,
	ElevenP,
	Twelve,
}

impl Level {
	pub const LEVELS: [Self; 18] = [
		Self::Unknown,
		Self::One,
		Self::Two,
		Self::Three,
		Self::Four,
		Self::Five,
		Self::Six,
		Self::Seven,
		Self::SevenP,
		Self::Eight,
		Self::EightP,
		Self::Nine,
		Self::NineP,
		Self::Ten,
		Self::TenP,
		Self::Eleven,
		Self::ElevenP,
		Self::Twelve,
	];

	pub const LEVEL_STRINGS: [&'static str; 18] = [
		"?", "1", "2", "3", "4", "5", "6", "7", "7+", "8", "8+", "9", "9+", "10", "10+", "11",
		"11+", "12",
	];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

impl FromStr for Level {
	type Err = anyhow::Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		for (i, level) in Self::LEVEL_STRINGS.iter().enumerate() {
			if *level == s {
				return Ok(Self::LEVELS[i]);
			}
		}

		bail!("Invalid level '{s}'");
	}
}

impl Display for Level {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", Self::LEVEL_STRINGS[self.to_index()])
	}
}

impl FromSql for Level {
	fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
		let str: String = rusqlite::types::FromSql::column_result(value)?;

		for (i, s) in Self::LEVEL_STRINGS.iter().enumerate() {
			if str == **s {
				return Ok(Self::LEVELS[i]);
			}
		}

		FromSqlResult::Err(FromSqlError::Other(
			format!("Cannot convert {} to level", str).into(),
		))
	}
}

impl ToSql for Level {
	fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
		Self::LEVEL_STRINGS[*self as usize].to_sql()
	}
}
// }}}
// {{{ Side
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Side {
	Light,
	Conflict,
	Silent,
	Lephon,
}

impl Side {
	pub const SIDES: [Self; 4] = [Self::Light, Self::Conflict, Self::Silent, Self::Lephon];
	pub const SIDE_STRINGS: [&'static str; 4] = ["light", "conflict", "silent", "lephon"];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

impl FromSql for Side {
	fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
		let str: String = rusqlite::types::FromSql::column_result(value)?;

		for (i, s) in Self::SIDE_STRINGS.iter().enumerate() {
			if str == **s {
				return Ok(Self::SIDES[i]);
			}
		}

		FromSqlResult::Err(FromSqlError::Other(
			format!("Cannot convert {} to side", str).into(),
		))
	}
}

impl ToSql for Side {
	fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
		Self::SIDE_STRINGS[*self as usize].to_sql()
	}
}
// }}}
// {{{ Song
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
	pub id: u32,
	pub shorthand: String,
	pub title: String,
	pub lowercase_title: String,

	#[allow(dead_code)]
	pub artist: String,

	pub bpm: String,
	pub side: Side,
}

impl Song {
	/// Returns true if multiple songs are known to exist with the given title.
	#[inline]
	pub fn ambigous_name(&self) -> bool {
		self.title == "Genesis" || self.title == "Quon"
	}
}

impl Display for Song {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.ambigous_name() {
			write!(f, "{} ({})", self.title, self.artist)
		} else {
			write!(f, "{}", self.title)
		}
	}
}
// }}}
// {{{ Chart
#[derive(Debug, Clone, Copy)]
pub struct Jacket {
	pub raw: &'static [u8],
	pub bitmap: &'static ImageBuffer<Rgb<u8>, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chart {
	pub id: u32,
	pub song_id: u32,
	pub title: Option<String>, // Name override for charts like PRAGMATISM
	pub lowercase_title: Option<String>,
	pub note_design: Option<String>,

	pub difficulty: Difficulty,
	pub level: Level,

	pub note_count: u32,
	pub chart_constant: u32,

	#[serde(skip)]
	pub cached_jacket: Option<Jacket>,

	/// If `None`, the default jacket is used.
	/// Otherwise, a difficulty-specific jacket exists.
	pub jacket_source: Option<Difficulty>,
}
// }}}
// {{{ Cached song
#[derive(Debug, Clone)]
pub struct CachedSong {
	pub song: Song,
	chart_ids: [Option<NonZeroU16>; Difficulty::DIFFICULTIES.len()],
}

impl CachedSong {
	#[inline]
	pub fn new(song: Song) -> Self {
		Self {
			song,
			chart_ids: [None; 5],
		}
	}

	#[inline]
	pub fn charts(&self) -> impl Iterator<Item = (Difficulty, u32)> {
		self.chart_ids
			.into_iter()
			.enumerate()
			.filter_map(|(i, id)| id.map(|id| (Difficulty::DIFFICULTIES[i], id.get() as u32)))
	}
}
// }}}
// {{{ Song cache
#[derive(Debug, Clone, Default)]
pub struct SongCache {
	pub songs: Vec<Option<CachedSong>>,
	pub charts: Vec<Option<Chart>>,
}

impl SongCache {
	#[inline]
	pub fn lookup_song(&self, id: u32) -> Result<&CachedSong, Error> {
		self.songs
			.get(id as usize)
			.and_then(|i| i.as_ref())
			.ok_or_else(|| anyhow!("Could not find song with id {}", id))
	}

	#[inline]
	pub fn lookup_chart(&self, chart_id: u32) -> Result<(&Song, &Chart), Error> {
		let chart = self
			.charts
			.get(chart_id as usize)
			.and_then(|i| i.as_ref())
			.ok_or_else(|| anyhow!("Could not find chart with id {}", chart_id))?;
		let song = &self.lookup_song(chart.song_id)?.song;

		Ok((song, chart))
	}

	#[inline]
	pub fn lookup_song_mut(&mut self, id: u32) -> Result<&mut CachedSong, Error> {
		self.songs
			.get_mut(id as usize)
			.and_then(|i| i.as_mut())
			.ok_or_else(|| anyhow!("Could not find song with id {}", id))
	}

	#[inline]
	pub fn lookup_chart_mut(&mut self, chart_id: u32) -> Result<&mut Chart, Error> {
		self.charts
			.get_mut(chart_id as usize)
			.and_then(|i| i.as_mut())
			.ok_or_else(|| anyhow!("Could not find chart with id {}", chart_id))
	}

	#[inline]
	pub fn lookup_by_difficulty(
		&self,
		id: u32,
		difficulty: Difficulty,
	) -> Result<(&Song, &Chart), Error> {
		let cached_song = self.lookup_song(id)?;
		let chart_id = cached_song.chart_ids[difficulty.to_index()]
			.ok_or_else(|| {
				anyhow!(
					"Cannot find chart {} [{difficulty:?}]",
					cached_song.song.title
				)
			})?
			.get() as u32;
		let chart = self.lookup_chart(chart_id)?.1;
		Ok((&cached_song.song, chart))
	}

	#[inline]
	pub fn lookup_by_difficulty_mut(
		&mut self,
		id: u32,
		difficulty: Difficulty,
	) -> Result<&mut Chart, Error> {
		let cached_song = self.lookup_song(id)?;
		let chart_id = cached_song.chart_ids[difficulty.to_index()]
			.ok_or_else(|| {
				anyhow!(
					"Cannot find chart {} [{difficulty:?}]",
					cached_song.song.title
				)
			})?
			.get() as u32;
		let chart = self.lookup_chart_mut(chart_id)?;
		Ok(chart)
	}

	#[inline]
	pub fn charts(&self) -> impl Iterator<Item = &Chart> {
		self.charts.iter().filter_map(|i| i.as_ref())
	}

	#[inline]
	pub fn charts_mut(&mut self) -> impl Iterator<Item = &mut Chart> {
		self.charts.iter_mut().filter_map(|i| i.as_mut())
	}

	// {{{ Populate cache
	pub fn new(conn: &rusqlite::Connection) -> Result<Self, Error> {
		let mut result = Self::default();

		// {{{ Songs
		let mut query = conn.prepare_cached("SELECT * FROM songs")?;
		let songs = query.query_map((), |row| {
			Ok(Song {
				id: row.get("id")?,
				lowercase_title: row.get::<_, String>("title")?.to_lowercase(),
				shorthand: row.get("shorthand")?,
				title: row.get("title")?,
				artist: row.get("artist")?,
				bpm: row.get("bpm")?,
				side: row.get("side")?,
			})
		})?;

		for song in songs {
			let song = song?;
			let song_id = song.id as usize;

			if song_id >= result.songs.len() {
				result.songs.resize(song_id + 1, None);
			}

			result.songs[song_id] = Some(CachedSong::new(song));
		}
		// }}}
		// {{{ Charts
		let mut query = conn.prepare_cached("SELECT * FROM charts")?;
		let charts = query.query_map((), |row| {
			Ok(Chart {
				id: row.get("id")?,
				song_id: row.get("song_id")?,
				title: row.get("title")?,
				lowercase_title: row
					.get::<_, Option<String>>("title")?
					.map(|t| t.to_lowercase()),
				difficulty: row.get("difficulty")?,
				level: row.get("level")?,
				chart_constant: row.get("chart_constant")?,
				note_count: row.get("note_count")?,
				note_design: row.get("note_design")?,
				cached_jacket: None,
				jacket_source: None,
			})
		})?;

		for chart in charts {
			let chart = chart?;

			// {{{ Tie chart to song
			{
				let index = chart.difficulty.to_index();
				result.lookup_song_mut(chart.song_id)?.chart_ids[index] =
					Some(NonZeroU16::new(chart.id as u16).unwrap());
			}
			// }}}
			// {{{ Save chart to cache
			{
				let index = chart.id as usize;
				if index >= result.charts.len() {
					result.charts.resize(index + 1, None);
				}
				result.charts[index] = Some(chart);
			}
			// }}}
		}
		// }}}

		Ok(result)
	}
	// }}}
}
// }}}
