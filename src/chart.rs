use sqlx::{prelude::FromRow, SqlitePool};

use crate::context::Error;

// {{{ Difficuly
#[derive(Debug, Clone, Copy, sqlx::Type)]
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

	pub const DIFFICULTY_STRINGS: [&'static str; 5] = ["PST", "PRS", "FTR", "ETR", "BYD"];

	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

impl TryFrom<String> for Difficulty {
	type Error = String;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		for (i, s) in Self::DIFFICULTY_STRINGS.iter().enumerate() {
			if value == **s {
				return Ok(Self::DIFFICULTIES[i]);
			}
		}

		Err(format!("Cannot convert {} to difficulty", value))
	}
}
// }}}
// {{{ Song
#[derive(Debug, Clone, FromRow)]
pub struct Song {
	pub id: u32,
	pub title: String,
	pub ocr_alias: Option<String>,
	pub artist: Option<String>,
}
// }}}
// {{{ Chart
#[derive(Debug, Clone, FromRow)]
pub struct Chart {
	pub id: u32,
	pub song_id: u32,

	pub difficulty: Difficulty,
	pub level: String, // TODO: this could become an enum

	pub note_count: u32,
	pub chart_constant: u32,
}
// }}}
// {{{ Cache
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
	pub fn lookup(&self, difficulty: Difficulty) -> Option<&Chart> {
		self.charts
			.get(difficulty.to_index())
			.and_then(|c| c.as_ref())
	}
}

#[derive(Debug, Clone, Default)]
pub struct SongCache {
	songs: Vec<Option<CachedSong>>,
}

impl SongCache {
	#[inline]
	pub fn lookup(&self, id: u32) -> Option<&CachedSong> {
		self.songs.get(id as usize).and_then(|i| i.as_ref())
	}

	// {{{ Populate cache
	pub async fn new(pool: &SqlitePool) -> Result<Self, Error> {
		let mut result = Self::default();

		let songs = sqlx::query!("SELECT * FROM songs").fetch_all(pool).await?;

		for song in songs {
			let song = Song {
				id: song.id as u32,
				title: song.title,
				ocr_alias: song.ocr_alias,
				artist: song.artist,
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
					difficulty: Difficulty::try_from(chart.difficulty)?,
					level: chart.level,
					chart_constant: chart.chart_constant as u32,
					note_count: chart.note_count as u32,
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
