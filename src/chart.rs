use sqlx::prelude::FromRow;

use crate::context::{Error, UserContext};

#[derive(Debug, Clone, Copy, sqlx::Type)]
pub enum Difficulty {
	PST,
	PRS,
	FTR,
	ETR,
	BYD,
}

impl Difficulty {
	#[inline]
	pub fn to_index(self) -> usize {
		self as usize
	}
}

#[derive(Debug, Clone, FromRow)]
pub struct Song {
	pub id: u32,
	pub title: String,
	pub ocr_alias: Option<String>,
	pub artist: Option<String>,
}

#[derive(Debug, Clone, Copy, FromRow)]
pub struct Chart {
	pub id: u32,
	pub song_id: u32,

	pub difficulty: Difficulty,
	pub level: u32,

	pub note_count: u32,
	pub chart_constant: u32,
}

#[derive(Debug, Clone)]
pub struct CachedSong {
	song: Song,
	charts: [Option<Chart>; 5],
}

impl CachedSong {
	pub fn new(song: Song, charts: [Option<Chart>; 5]) -> Self {
		Self { song, charts }
	}
}

#[derive(Debug, Clone, Default)]
pub struct SongCache {
	songs: Vec<Option<CachedSong>>,
}

impl SongCache {
	pub async fn new(ctx: &UserContext) -> Result<Self, Error> {
		let mut result = Self::default();

		let songs: Vec<Song> = sqlx::query_as("SELECT * FROM songs")
			.fetch_all(&ctx.db)
			.await?;

		for song in songs {
			let song_id = song.id as usize;

			if song_id >= result.songs.len() {
				result.songs.resize(song_id, None);
			}

			let charts: Vec<Chart> = sqlx::query_as("SELECT * FROM charts WHERE song_id=?")
				.bind(song.id)
				.fetch_all(&ctx.db)
				.await?;

			let mut chart_cache = [None; 5];
			for chart in charts {
				chart_cache[chart.difficulty.to_index()] = Some(chart);
			}

			result.songs[song_id] = Some(CachedSong::new(song, chart_cache));
		}

		Ok(result)
	}
}
