use std::path::PathBuf;

use sqlx::SqlitePool;

use crate::{chart::SongCache, jacket::JacketCache};

// Types used by all command functions
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, UserContext, Error>;

// Custom user data passed to all command functions
pub struct UserContext {
	pub data_dir: PathBuf,
	pub db: SqlitePool,
	pub song_cache: SongCache,
	pub jacket_cache: JacketCache,
}

impl UserContext {
	#[inline]
	pub async fn new(data_dir: PathBuf, db: SqlitePool) -> Result<Self, Error> {
		let song_cache = SongCache::new(&db).await?;
		let jacket_cache = JacketCache::new(&data_dir)?;
		Ok(Self {
			data_dir,
			db,
			song_cache,
			jacket_cache,
		})
	}
}
