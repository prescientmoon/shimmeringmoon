// {{{ Imports
use db::{connect_db, SqlitePool};
use std::ops::Deref;

use crate::arcaea::jacket::read_jackets;
use crate::arcaea::{chart::SongCache, jacket::JacketCache};
use crate::assets::{EXO_FONT, GEOSANS_FONT, KAZESAWA_BOLD_FONT, KAZESAWA_FONT};
use crate::context::paths::ShimmeringPaths;
use crate::recognition::{hyperglass::CharMeasurements, ui::UIMeasurements};
use crate::timed;
// }}}

pub mod db;
mod hash;
pub mod paths;
mod process_jackets;

// {{{ Common types
pub type Error = anyhow::Error;
pub type PoiseContext<'a> = poise::Context<'a, UserContext, Error>;
// }}}
// {{{ Error handling
#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
	User,
	Internal,
}

#[derive(Debug)]
pub struct TaggedError {
	pub kind: ErrorKind,
	pub error: Error,
}

impl TaggedError {
	#[inline]
	pub fn new(kind: ErrorKind, error: Error) -> Self {
		Self { kind, error }
	}
}

#[macro_export]
macro_rules! get_user_error {
	($err:expr) => {{
		match $err.kind {
			$crate::context::ErrorKind::User => $err.error,
			$crate::context::ErrorKind::Internal => Err($err.error)?,
		}
	}};
}

impl<E: Into<Error>> From<E> for TaggedError {
	fn from(value: E) -> Self {
		Self::new(ErrorKind::Internal, value.into())
	}
}

pub trait TagError {
	fn tag(self, tag: ErrorKind) -> TaggedError;
}

impl TagError for Error {
	fn tag(self, tag: ErrorKind) -> TaggedError {
		TaggedError::new(tag, self)
	}
}
// }}}
// {{{ UserContext
/// Custom user data passed to all command functions
#[derive(Clone)]
pub struct UserContext {
	pub db: SqlitePool,
	pub song_cache: SongCache,
	pub jacket_cache: JacketCache,
	pub ui_measurements: UIMeasurements,

	pub paths: ShimmeringPaths,

	pub geosans_measurements: CharMeasurements,
	pub exo_measurements: CharMeasurements,
	// TODO: do we really need both after I've fixed the bug in the ocr code?
	pub kazesawa_measurements: CharMeasurements,
	pub kazesawa_bold_measurements: CharMeasurements,
}

impl UserContext {
	#[inline]
	pub fn new() -> Result<Self, Error> {
		timed!("create_context", {
			let paths = ShimmeringPaths::new()?;
			let db = connect_db(&paths)?;

			let mut song_cache = SongCache::new(db.get()?.deref())?;
			let ui_measurements = UIMeasurements::read()?;
			let jacket_cache = JacketCache::new(&paths)?;

			read_jackets(&paths, &mut song_cache)?;

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
				paths,
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
// }}}
// {{{ Testing helpers
#[cfg(test)]
pub mod testing {
	use std::cell::OnceCell;
	use tempfile::TempDir;

	use super::*;
	use crate::commands::discord::mock::MockContext;

	pub fn get_shared_context() -> &'static UserContext {
		static CELL: OnceCell<UserContext> = OnceCell::new();
		CELL.get_or_init(|| UserContext::new().unwrap())
	}

	pub fn import_songs_and_jackets_from(paths: &ShimmeringPaths, to: &Path) {
		let out = std::process::Command::new("scripts/copy-chart-info.sh")
			.arg(paths.data_dir())
			.arg(to)
			.output()
			.expect("Could not run sh chart info copy script");

		assert!(
			out.status.success(),
			"chart info copy script exited with non-0 code"
		);
	}

	pub fn get_mock_context() -> Result<(MockContext, TempDir), Error> {
		let mut data = (*get_shared_context()).clone();
		let dir = tempfile::tempdir()?;
		data.db = connect_db(dir.path());
		import_songs_and_jackets_from(&data.paths, dir.path());

		let ctx = MockContext::new(data);
		Ok((ctx, dir))
	}

	// rustfmt fucks up the formatting here,
	// but the skip attribute doesn't seem to work well on macros 🤔
	#[macro_export]
	macro_rules! golden_test {
		($name:ident, $test_path:expr) => {
			paste::paste! {
				#[tokio::test]
				async fn [<$name _test>]() -> Result<(), $crate::context::Error> {
			$crate::with_test_ctx!($test_path, $name)
				  }
			  }
		};
	}

	#[macro_export]
	macro_rules! with_test_ctx {
		($test_path:expr, $f:expr) => {{
			use std::str::FromStr;

			let (mut ctx, _guard) = $crate::context::testing::get_mock_context()?;
			let res = $crate::user::User::create_from_context(&ctx);
			ctx.handle_error(res).await?;

			let ctx: &mut $crate::commands::discord::mock::MockContext = &mut ctx;
			let res: Result<(), $crate::context::TaggedError> = $f(ctx).await;
			ctx.handle_error(res).await?;

			ctx.golden(&std::path::PathBuf::from_str("test")?.join($test_path))?;
			Ok(())
		}};
	}
}
// }}}
