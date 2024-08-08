use std::{fs, path::PathBuf};

use sqlx::SqlitePool;

use crate::{
	arcaea::chart::SongCache, arcaea::jacket::JacketCache, recognition::ui::UIMeasurements,
};

// Types used by all command functions
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, UserContext, Error>;

// Custom user data passed to all command functions
pub struct UserContext {
	#[allow(dead_code)]
	pub data_dir: PathBuf,

	pub db: SqlitePool,
	pub song_cache: SongCache,
	pub jacket_cache: JacketCache,
	pub ui_measurements: UIMeasurements,
}

impl UserContext {
	#[inline]
	pub async fn new(data_dir: PathBuf, cache_dir: PathBuf, db: SqlitePool) -> Result<Self, Error> {
		fs::create_dir_all(&cache_dir)?;
		fs::create_dir_all(&data_dir)?;

		let mut song_cache = SongCache::new(&db).await?;
		let jacket_cache = JacketCache::new(&data_dir, &mut song_cache)?;
		let ui_measurements = UIMeasurements::read(&data_dir)?;

		println!("Created user context");

		Ok(Self {
			data_dir,
			db,
			song_cache,
			jacket_cache,
			ui_measurements,
		})
	}
}
