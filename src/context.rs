use std::{fs, path::PathBuf};

use sqlx::SqlitePool;

use crate::{
	arcaea::{chart::SongCache, jacket::JacketCache},
	assets::{EXO_FONT, GEOSANS_FONT},
	recognition::{hyperglass::CharMeasurements, ui::UIMeasurements},
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

	pub geosans_measurements: CharMeasurements,
	pub exo_measurements: CharMeasurements,
}

impl UserContext {
	#[inline]
	pub async fn new(data_dir: PathBuf, cache_dir: PathBuf, db: SqlitePool) -> Result<Self, Error> {
		fs::create_dir_all(&cache_dir)?;
		fs::create_dir_all(&data_dir)?;

		let mut song_cache = SongCache::new(&db).await?;
		let jacket_cache = JacketCache::new(&data_dir, &mut song_cache)?;
		let ui_measurements = UIMeasurements::read(&data_dir)?;

		let geosans_measurements = GEOSANS_FONT
			.with_borrow_mut(|font| CharMeasurements::from_text(font, "0123456789'", None))?;
		let exo_measurements = EXO_FONT.with_borrow_mut(|font| {
			CharMeasurements::from_text(
				font,
				"0123456789'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
				Some(700),
			)
		})?;

		println!("Created user context");

		Ok(Self {
			data_dir,
			db,
			song_cache,
			jacket_cache,
			ui_measurements,
			geosans_measurements,
			exo_measurements,
		})
	}
}
