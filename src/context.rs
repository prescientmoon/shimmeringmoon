use include_dir::{include_dir, Dir};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite_migration::Migrations;
use std::fs;
use std::sync::LazyLock;

use crate::{
	arcaea::{chart::SongCache, jacket::JacketCache},
	assets::{get_data_dir, EXO_FONT, GEOSANS_FONT, KAZESAWA_BOLD_FONT, KAZESAWA_FONT},
	recognition::{hyperglass::CharMeasurements, ui::UIMeasurements},
	timed,
};

// Types used by all command functions
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, UserContext, Error>;

pub type DbConnection = r2d2::Pool<SqliteConnectionManager>;

// Custom user data passed to all command functions
pub struct UserContext {
	pub db: DbConnection,
	pub song_cache: SongCache,
	pub jacket_cache: JacketCache,
	pub ui_measurements: UIMeasurements,

	pub geosans_measurements: CharMeasurements,
	pub exo_measurements: CharMeasurements,
	// TODO: do we really need both after I've fixed the bug in the ocr code?
	pub kazesawa_measurements: CharMeasurements,
	pub kazesawa_bold_measurements: CharMeasurements,
}

impl UserContext {
	#[inline]
	pub async fn new() -> Result<Self, Error> {
		timed!("create_context", {
			fs::create_dir_all(get_data_dir())?;

			// {{{ Connect to database
			let db = timed!("create_sqlite_pool", {
				Pool::new(
					SqliteConnectionManager::file(&format!(
						"{}/db.sqlite",
						get_data_dir().to_str().unwrap()
					))
					.with_init(|conn| {
						static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");
						static MIGRATIONS: LazyLock<Migrations> = LazyLock::new(|| {
							Migrations::from_directory(&MIGRATIONS_DIR)
								.expect("Could not load migrations")
						});

						MIGRATIONS
							.to_latest(conn)
							.expect("Could not run migrations");

						Ok(())
					}),
				)
				.expect("Could not open sqlite database.")
			});
			// }}}

			let mut song_cache = timed!("make_song_cache", { SongCache::new(&db)? });
			let jacket_cache = timed!("make_jacket_cache", { JacketCache::new(&mut song_cache)? });
			let ui_measurements = timed!("read_ui_measurements", { UIMeasurements::read()? });

			// {{{ Font measurements
			static WHITELIST: &str = "0123456789'abcdefghklmnopqrstuvwxyzABCDEFGHIJKLMNOPRSTUVWXYZ";

			let geosans_measurements = GEOSANS_FONT
				.with_borrow_mut(|font| CharMeasurements::from_text(font, WHITELIST, None))?;
			let kazesawa_measurements = KAZESAWA_FONT
				.with_borrow_mut(|font| CharMeasurements::from_text(font, WHITELIST, None))?;
			let kazesawa_bold_measurements = KAZESAWA_BOLD_FONT
				.with_borrow_mut(|font| CharMeasurements::from_text(font, WHITELIST, None))?;
			let exo_measurements = EXO_FONT
				.with_borrow_mut(|font| CharMeasurements::from_text(font, WHITELIST, Some(700)))?;
			// }}}

			Ok(Self {
				db,
				song_cache,
				jacket_cache,
				ui_measurements,
				geosans_measurements,
				exo_measurements,
				kazesawa_measurements,
				kazesawa_bold_measurements,
			})
		})
	}
}
