#![allow(dead_code)]
use std::{cell::RefCell, env::var, path::PathBuf, str::FromStr, sync::OnceLock};

use freetype::{Face, Library};

#[inline]
fn get_data_dir() -> PathBuf {
	PathBuf::from_str(&var("SHIMMERING_DATA_DIR").expect("Missing `SHIMMERING_DATA_DIR` env var"))
		.expect("`SHIMMERING_DATA_DIR` is not a valid path")
}

#[inline]
fn get_font(name: &str, assets_dir: &PathBuf) -> RefCell<Face> {
	let face = FREETYPE_LIB.with(|lib| {
		lib.new_face(assets_dir.join(format!("{}-variable.ttf", name)), 0)
			.expect(&format!("Could not load {} font", name))
	});
	RefCell::new(face)
}

thread_local! {
pub static DATA_DIR: PathBuf = get_data_dir();
pub static ASSETS_DIR: PathBuf = DATA_DIR.with(|p| p.join("assets"));
pub static FREETYPE_LIB: Library = Library::init().unwrap();
pub static SAIRA_FONT: RefCell<Face> = ASSETS_DIR.with(|assets_dir| get_font("saira", assets_dir));
pub static EXO_FONT: RefCell<Face> = ASSETS_DIR.with(|assets_dir| get_font("exo", assets_dir));
}

#[inline]
pub fn should_skip_jacket_art() -> bool {
	static CELL: OnceLock<bool> = OnceLock::new();
	*CELL.get_or_init(|| var("SHIMMERING_NO_JACKETS").unwrap_or_default() == "1")
}
