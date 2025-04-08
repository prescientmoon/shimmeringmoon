// {{{ Imports
use anyhow::{anyhow, Context};
use include_dir::{include_dir, Dir};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite_migration::Migrations;
use std::sync::LazyLock;

use crate::arcaea::import_charts::{import_songlist, NOTECOUNT_DATA};
use crate::context::hash::{hash_bytes, hash_files};
use crate::context::paths::ShimmeringPaths;
use crate::context::process_jackets::process_jackets;
// }}}

pub type SqlitePool = r2d2::Pool<SqliteConnectionManager>;

pub fn connect_db(paths: &ShimmeringPaths) -> anyhow::Result<SqlitePool> {
	let db_path = paths.db_path();
	let mut conn = rusqlite::Connection::open(&db_path)
		.with_context(|| "Could not connect to sqlite database")?;
	conn.pragma_update(None, "journal_mode", "WAL")?;
	conn.pragma_update(None, "foreign_keys", "ON")?;

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
	let current_songlist_hash = hash_files(&paths.songlist_path())?;
	let current_cc_data_hash = hash_files(&paths.cc_data_path())?;
	let current_notecount_hash = hash_bytes(NOTECOUNT_DATA);

	let (prev_raw_jackets_hash, prev_songlist_hash, prev_cc_data_hash, prev_notecount_hash) = conn
		.query_row("SELECT * FROM metadata", (), |row| {
			Ok((
				row.get_ref("raw_jackets_hash")?.as_str()?.to_owned(),
				row.get_ref("songlist_hash")?.as_str()?.to_owned(),
				row.get_ref("cc_data_hash")?.as_str()?.to_owned(),
				row.get_ref("notecount_hash")?.as_str()?.to_owned(),
			))
		})
		.with_context(|| anyhow!("No metadata row found"))?;

	let mut should_reprocess_jackets = true;

	if current_songlist_hash != prev_songlist_hash
		|| current_cc_data_hash != prev_cc_data_hash
		|| current_notecount_hash != prev_notecount_hash
	{
		println!("😞 Chart data hash mismatch. Re-importing everything");

		// {{ Import songlist & update hashes
		import_songlist(paths, &mut conn).context("Failed to import songlist file")?;

		conn.execute(
			"
        UPDATE metadata 
        SET songlist_hash=?,
            cc_data_hash=?,
            notecount_hash=?
      ",
			(
				current_songlist_hash,
				current_cc_data_hash,
				current_notecount_hash,
			),
		)?;
	// }}}
	} else if current_raw_jackets_hash != prev_raw_jackets_hash {
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
