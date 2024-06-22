use sqlx::SqlitePool;

use crate::chart::SongCache;

// Types used by all command functions
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, UserContext, Error>;

// Custom user data passed to all command functions
pub struct UserContext {
	pub db: SqlitePool,
	pub song_cache: SongCache,
}

impl UserContext {
	#[inline]
	pub fn new(db: SqlitePool) -> Self {
		Self {
			db,
			song_cache: SongCache::default(),
		}
	}
}
