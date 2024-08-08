use std::path::PathBuf;

use image::{ImageBuffer, Rgb};
use sqlx::SqlitePool;

use crate::{bitmap::Color, context::Error};

// {{{ Difficuly
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
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
		["past", "present", "future", "eternal", "beyond"];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

impl TryFrom<String> for Difficulty {
	type Error = String;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		for (i, s) in Self::DIFFICULTY_SHORTHANDS.iter().enumerate() {
			if value == **s {
				return Ok(Self::DIFFICULTIES[i]);
			}
		}

		Err(format!("Cannot convert {} to difficulty", value))
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

impl TryFrom<String> for Side {
	type Error = String;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		for (i, s) in Self::SIDE_STRINGS.iter().enumerate() {
			if value == **s {
				return Ok(Self::SIDES[i]);
			}
		}

		Err(format!("Cannot convert {} to difficulty", value))
	}
}
// }}}
// {{{ Song
#[derive(Debug, Clone)]
pub struct Song {
	pub id: u32,
	pub title: String,
	pub lowercase_title: String,
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
	pub level: String, // TODO: this could become an enum

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
	charts: [Option<Chart>; 5],
}

impl CachedSong {
	#[inline]
	pub fn new(song: Song, charts: [Option<Chart>; 5]) -> Self {
		Self { song, charts }
	}

	#[inline]
	pub fn lookup(&self, difficulty: Difficulty) -> Result<&Chart, Error> {
		self.charts
			.get(difficulty.to_index())
			.and_then(|c| c.as_ref())
			.ok_or_else(|| {
				format!(
					"Could not find difficulty {:?} for song {}",
					difficulty, self.song.title
				)
				.into()
			})
	}

	#[inline]
	pub fn lookup_mut(&mut self, difficulty: Difficulty) -> Result<&mut Chart, Error> {
		self.charts
			.get_mut(difficulty.to_index())
			.and_then(|c| c.as_mut())
			.ok_or_else(|| {
				format!(
					"Could not find difficulty {:?} for song {}",
					difficulty, self.song.title
				)
				.into()
			})
	}

	#[inline]
	pub fn charts(&self) -> impl Iterator<Item = &Chart> {
		self.charts.iter().filter_map(|i| i.as_ref())
	}

	#[inline]
	pub fn charts_mut(&mut self) -> impl Iterator<Item = &mut Chart> {
		self.charts.iter_mut().filter_map(|i| i.as_mut())
	}
}
// }}}
// {{{ Song cache
#[derive(Debug, Clone, Default)]
pub struct SongCache {
	songs: Vec<Option<CachedSong>>,
}

impl SongCache {
	#[inline]
	pub fn lookup(&self, id: u32) -> Result<&CachedSong, Error> {
		self.songs
			.get(id as usize)
			.and_then(|i| i.as_ref())
			.ok_or_else(|| format!("Could not find song with id {}", id).into())
	}

	#[inline]
	pub fn lookup_chart(&self, chart_id: u32) -> Result<(&Song, &Chart), Error> {
		self.songs()
			.find_map(|item| {
				item.charts().find_map(|chart| {
					if chart.id == chart_id {
						Some((&item.song, chart))
					} else {
						None
					}
				})
			})
			.ok_or_else(|| format!("Could not find chart with id {}", chart_id).into())
	}

	#[inline]
	pub fn lookup_mut(&mut self, id: u32) -> Result<&mut CachedSong, Error> {
		self.songs
			.get_mut(id as usize)
			.and_then(|i| i.as_mut())
			.ok_or_else(|| format!("Could not find song with id {}", id).into())
	}

	#[inline]
	pub fn lookup_chart_mut(&mut self, chart_id: u32) -> Result<&mut Chart, Error> {
		self.songs_mut()
			.find_map(|item| {
				item.charts_mut().find_map(|chart| {
					if chart.id == chart_id {
						Some(chart)
					} else {
						None
					}
				})
			})
			.ok_or_else(|| format!("Could not find chart with id {}", chart_id).into())
	}

	#[inline]
	pub fn songs(&self) -> impl Iterator<Item = &CachedSong> {
		self.songs.iter().filter_map(|i| i.as_ref())
	}

	#[inline]
	pub fn songs_mut(&mut self) -> impl Iterator<Item = &mut CachedSong> {
		self.songs.iter_mut().filter_map(|i| i.as_mut())
	}

	// {{{ Populate cache
	pub async fn new(pool: &SqlitePool) -> Result<Self, Error> {
		let mut result = Self::default();

		let songs = sqlx::query!("SELECT * FROM songs").fetch_all(pool).await?;

		for song in songs {
			let song = Song {
				id: song.id as u32,
				lowercase_title: song.title.to_lowercase(),
				title: song.title,
				artist: song.artist,
				pack: song.pack,
				bpm: song.bpm,
				side: Side::try_from(song.side)?,
			};

			let song_id = song.id as usize;

			if song_id >= result.songs.len() {
				result.songs.resize(song_id + 1, None);
			}

			let charts = sqlx::query!("SELECT * FROM charts WHERE song_id=?", song.id)
				.fetch_all(pool)
				.await?;

			let mut chart_cache: [Option<_>; 5] = Default::default();
			for chart in charts {
				let chart = Chart {
					id: chart.id as u32,
					song_id: chart.song_id as u32,
					shorthand: chart.shorthand,
					difficulty: Difficulty::try_from(chart.difficulty)?,
					level: chart.level,
					chart_constant: chart.chart_constant as u32,
					note_count: chart.note_count as u32,
					cached_jacket: None,
					note_design: chart.note_design,
				};

				let index = chart.difficulty.to_index();
				chart_cache[index] = Some(chart);
			}

			result.songs[song_id] = Some(CachedSong::new(song, chart_cache));
		}

		Ok(result)
	}
	// }}}
}
// }}}
