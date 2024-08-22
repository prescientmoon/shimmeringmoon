use std::{fmt::Display, num::NonZeroU16, path::PathBuf};

use image::{ImageBuffer, Rgb};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};

use crate::{
	bitmap::Color,
	context::{DbConnection, Error},
};

// {{{ Difficuly
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
	pub const DIFFICULTY_STRINGS: [&'static str; 5] =
		["PAST", "PRESENT", "FUTURE", "ETERNAL", "BEYOND"];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
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

impl Display for Difficulty {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			Self::DIFFICULTY_SHORTHANDS[self.to_index()].to_lowercase()
		)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
	Twelve,
}

impl Level {
	pub const LEVELS: [Self; 17] = [
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
		Self::Twelve,
	];

	pub const LEVEL_STRINGS: [&'static str; 17] = [
		"?", "1", "2", "3", "4", "5", "6", "7", "7+", "8", "8+", "9", "9+", "10", "10+", "11", "12",
	];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
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
// }}}
// {{{ Side
#[derive(Debug, Clone, Copy)]
pub enum Side {
	Light,
	Conflict,
	Silent,
}

impl Side {
	pub const SIDES: [Self; 3] = [Self::Light, Self::Conflict, Self::Silent];
	pub const SIDE_STRINGS: [&'static str; 3] = ["light", "conflict", "silent"];

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
// }}}
// {{{ Song
#[derive(Debug, Clone)]
pub struct Song {
	pub id: u32,
	pub title: String,
	pub lowercase_title: String,

	#[allow(dead_code)]
	pub artist: String,

	pub bpm: String,
	pub pack: Option<String>,
	pub side: Side,
}
// }}}
// {{{ Chart
#[derive(Debug, Clone, Copy)]
pub struct Jacket {
	pub raw: &'static [u8],
	pub bitmap: &'static ImageBuffer<Rgb<u8>, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct Chart {
	pub id: u32,
	pub song_id: u32,
	pub shorthand: Option<String>,
	pub note_design: Option<String>,

	pub difficulty: Difficulty,
	pub level: Level,

	pub note_count: u32,
	pub chart_constant: u32,

	pub cached_jacket: Option<Jacket>,
}

impl Chart {
	#[inline]
	pub fn jacket_path(&self, data_dir: &PathBuf) -> PathBuf {
		data_dir
			.join("jackets")
			.join(format!("{}-{}.jpg", self.song_id, self.id))
	}
}
// }}}
// {{{ Cached song
#[derive(Debug, Clone)]
pub struct CachedSong {
	pub song: Song,
	chart_ids: [Option<NonZeroU16>; 5],
}

impl CachedSong {
	#[inline]
	pub fn new(song: Song) -> Self {
		Self {
			song,
			chart_ids: [None; 5],
		}
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
			.ok_or_else(|| format!("Could not find song with id {}", id).into())
	}

	#[inline]
	pub fn lookup_chart(&self, chart_id: u32) -> Result<(&Song, &Chart), Error> {
		let chart = self
			.charts
			.get(chart_id as usize)
			.and_then(|i| i.as_ref())
			.ok_or_else(|| format!("Could not find chart with id {}", chart_id))?;
		let song = &self.lookup_song(chart.song_id)?.song;

		Ok((song, chart))
	}

	#[inline]
	pub fn lookup_song_mut(&mut self, id: u32) -> Result<&mut CachedSong, Error> {
		self.songs
			.get_mut(id as usize)
			.and_then(|i| i.as_mut())
			.ok_or_else(|| format!("Could not find song with id {}", id).into())
	}

	#[inline]
	pub fn lookup_chart_mut(&mut self, chart_id: u32) -> Result<&mut Chart, Error> {
		self.charts
			.get_mut(chart_id as usize)
			.and_then(|i| i.as_mut())
			.ok_or_else(|| format!("Could not find chart with id {}", chart_id).into())
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
				format!(
					"Cannot find chart {} [{difficulty:?}]",
					cached_song.song.title
				)
			})?
			.get() as u32;
		let chart = self.lookup_chart(chart_id)?.1;
		Ok((&cached_song.song, chart))
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
	pub fn new(conn: &DbConnection) -> Result<Self, Error> {
		let conn = conn.get()?;
		let mut result = Self::default();

		// {{{ Songs
		let mut query = conn.prepare_cached("SELECT * FROM songs")?;
		let songs = query.query_map((), |row| {
			Ok(Song {
				id: row.get("id")?,
				lowercase_title: row.get::<_, String>("title")?.to_lowercase(),
				title: row.get("title")?,
				artist: row.get("artist")?,
				pack: row.get("pack")?,
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
				shorthand: row.get("shorthand")?,
				difficulty: row.get("difficulty")?,
				level: row.get("level")?,
				chart_constant: row.get("chart_constant")?,
				note_count: row.get("note_count")?,
				note_design: row.get("note_design")?,
				cached_jacket: None,
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
