use std::{path::PathBuf, sync::Arc};

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::{chart::SongCache, jacket::JacketCache};

// Types used by all command functions
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, UserContext, Error>;

// Custom user data passed to all command functions
pub struct UserContext {
	pub data_dir: PathBuf,
	pub db: SqlitePool,
	pub song_cache: Arc<Mutex<SongCache>>,
	pub jacket_cache: JacketCache,
}

impl UserContext {
	#[inline]
	pub async fn new(data_dir: PathBuf, db: SqlitePool) -> Result<Self, Error> {
		let song_cache = SongCache::new(&data_dir, &db).await?;
		let jacket_cache = JacketCache::new(&song_cache)?;
		Ok(Self {
			data_dir,
			db,
			song_cache: Arc::new(Mutex::new(song_cache)),
			jacket_cache,
		})
	}
}
