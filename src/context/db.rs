// {{{ Imports
use anyhow::{anyhow, Context};
use include_dir::{include_dir, Dir};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite_migration::Migrations;
use std::sync::LazyLock;

use crate::context::hash::hash_files;
use crate::context::paths::ShimmeringPaths;
use crate::context::process_jackets::process_jackets;
// }}}

pub type SqlitePool = r2d2::Pool<SqliteConnectionManager>;

pub fn connect_db(paths: &ShimmeringPaths) -> anyhow::Result<SqlitePool> {
	let db_path = paths.db_path();
	let mut conn = rusqlite::Connection::open(&db_path)
		.with_context(|| "Could not connect to sqlite database")?;

	// {{{ Run migrations
	static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");
	static MIGRATIONS: LazyLock<Migrations> = LazyLock::new(|| {
		Migrations::from_directory(&MIGRATIONS_DIR).expect("Could not load migrations")
	});

	MIGRATIONS
		.to_latest(&mut conn)
		.with_context(|| "Could not run migrations")?;
	println!("✅ Ensured db schema is up to date");
	// }}}
	// {{{ Check if we need to reprocess jackets
	let current_raw_jackets_hash = hash_files(&paths.raw_jackets_path())?;

	// All this nonsense is so we can query without allocating
	// space for the output string 💀
	let mut statement = conn.prepare("SELECT raw_jackets_hash FROM metadata")?;
	let mut rows = statement.query(())?;
	let prev_raw_jackets_hash = rows
		.next()?
		.ok_or_else(|| anyhow!("No metadata row found"))?
		.get_ref("raw_jackets_hash")?
		.as_str()?;

	let mut should_reprocess_jackets = true;

	if current_raw_jackets_hash != prev_raw_jackets_hash {
		println!("😞 Jacket hashes do not match. Re-running the processing pipeline");
	} else if !paths.recognition_matrix_path().exists() {
		println!("😞 Jacket recognition matrix not found.");
	} else if !paths.jackets_path().exists() {
		println!("😞 Processed jackets not found.");
	} else {
		println!("✅ Jacket hashes match. Skipping jacket processing");
		should_reprocess_jackets = false;
	}

	if should_reprocess_jackets {
		process_jackets(paths, &conn)?;
		conn.prepare("UPDATE metadata SET raw_jackets_hash=?")?
			.execute([current_raw_jackets_hash])?;
		println!("✅ Jacket processing pipeline run succesfully");
	}
	// }}}

	Pool::new(SqliteConnectionManager::file(&db_path))
		.with_context(|| "Could not open sqlite database.")
}
