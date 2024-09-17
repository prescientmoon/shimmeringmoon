use include_dir::{include_dir, Dir};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite_migration::Migrations;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use crate::{
	arcaea::{chart::SongCache, jacket::JacketCache},
	assets::{get_data_dir, EXO_FONT, GEOSANS_FONT, KAZESAWA_BOLD_FONT, KAZESAWA_FONT},
	recognition::{hyperglass::CharMeasurements, ui::UIMeasurements},
	timed,
};

// Types used by all command functions
pub type Error = anyhow::Error;
pub type Context<'a> = poise::Context<'a, UserContext, Error>;

pub type DbConnection = r2d2::Pool<SqliteConnectionManager>;

// Custom user data passed to all command functions
#[derive(Clone)]
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

pub fn connect_db(data_dir: &Path) -> DbConnection {
	fs::create_dir_all(data_dir).expect("Could not create $SHIMMERING_DATA_DIR");

	let data_dir = data_dir.to_str().unwrap().to_owned();

	let db_path = format!("{}/db.sqlite", data_dir);
	let mut conn = rusqlite::Connection::open(&db_path).unwrap();
	static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");
	static MIGRATIONS: LazyLock<Migrations> = LazyLock::new(|| {
		Migrations::from_directory(&MIGRATIONS_DIR).expect("Could not load migrations")
	});

	MIGRATIONS
		.to_latest(&mut conn)
		.expect("Could not run migrations");

	Pool::new(SqliteConnectionManager::file(&db_path)).expect("Could not open sqlite database.")
}

impl UserContext {
	#[inline]
	pub async fn new() -> Result<Self, Error> {
		timed!("create_context", {
			let db = connect_db(&get_data_dir());

			let mut song_cache = SongCache::new(&db)?;
			let ui_measurements = UIMeasurements::read()?;
			let jacket_cache = timed!("make_jacket_cache", { JacketCache::new(&mut song_cache)? });

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

#[cfg(test)]
pub mod testing {
	use super::*;

	pub async fn get_shared_context() -> &'static UserContext {
		static CELL: tokio::sync::OnceCell<UserContext> = tokio::sync::OnceCell::const_new();
		CELL.get_or_init(async || {
			// env::set_var("SHIMMERING_DATA_DIR", "")
			UserContext::new().await.unwrap()
		})
		.await
	}

	pub fn import_songs_and_jackets_from(to: &Path) -> () {
		let out = std::process::Command::new("scripts/copy-chart-info.sh")
			.arg(get_data_dir())
			.arg(to)
			.output()
			.expect("Could not run sh chart info copy script");

		assert!(
			out.status.success(),
			"chart info copy script exited with non-0 code"
		);
	}

	#[macro_export]
	macro_rules! with_test_ctx {
		($test_path:expr, $f:expr) => {{
			use std::str::FromStr;

			let mut data = (*crate::context::testing::get_shared_context().await).clone();
			let dir = tempfile::tempdir()?;
			data.db = crate::context::connect_db(dir.path());
			crate::context::testing::import_songs_and_jackets_from(dir.path());

			let mut ctx = crate::commands::discord::mock::MockContext::new(data);
			crate::user::User::create_from_context(&ctx)?;

			let res: Result<(), Error> = $f(&mut ctx).await;
			res?;

			ctx.golden(&std::path::PathBuf::from_str($test_path)?)?;
			Ok(())
		}};
	}
}
