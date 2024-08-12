use std::{cell::RefCell, env::var, path::PathBuf, str::FromStr, sync::OnceLock, thread::LocalKey};

use freetype::{Face, Library};
use image::{imageops::FilterType, ImageBuffer, Rgb, Rgba};

use crate::{arcaea::chart::Difficulty, timed};

#[inline]
pub fn get_data_dir() -> PathBuf {
	PathBuf::from_str(&var("SHIMMERING_DATA_DIR").expect("Missing `SHIMMERING_DATA_DIR` env var"))
		.expect("`SHIMMERING_DATA_DIR` is not a valid path")
}

#[inline]
pub fn get_assets_dir() -> PathBuf {
	get_data_dir().join("assets")
}

#[inline]
fn get_font(name: &str) -> RefCell<Face> {
	let face = timed!(format!("load font \"{name}\""), {
		FREETYPE_LIB.with(|lib| {
			lib.new_face(get_assets_dir().join(name), 0)
				.expect(&format!("Could not load {} font", name))
		})
	});
	RefCell::new(face)
}

thread_local! {
pub static FREETYPE_LIB: Library = Library::init().unwrap();
pub static SAIRA_FONT: RefCell<Face> = get_font("saira-variable.ttf");
pub static EXO_FONT: RefCell<Face> = get_font("exo-variable.ttf");
pub static GEOSANS_FONT: RefCell<Face> = get_font("geosans-light.ttf");
pub static KAZESAWA_FONT: RefCell<Face> = get_font("kazesawa-regular.ttf");
pub static KAZESAWA_BOLD_FONT: RefCell<Face> = get_font("kazesawa-bold.ttf");
pub static NOTO_SANS_FONT: RefCell<Face> = get_font("noto-sans.ttf");
pub static ARIAL_FONT: RefCell<Face> = get_font("arial.ttf");
pub static UNI_FONT: RefCell<Face> = get_font("unifont.otf");
}

#[inline]
pub fn with_font<T>(
	primary: &'static LocalKey<RefCell<Face>>,
	f: impl FnOnce(&mut [&mut Face]) -> T,
) -> T {
	UNI_FONT.with_borrow_mut(|uni| {
		// NOTO_SANS_FONT.with_borrow_mut(|noto| {
		// ARIAL_FONT.with_borrow_mut(|arial| {
		primary.with_borrow_mut(|primary| f(&mut [primary, uni]))
		// })
		// })
	})
}

#[inline]
pub fn should_skip_jacket_art() -> bool {
	static CELL: OnceLock<bool> = OnceLock::new();
	*CELL.get_or_init(|| var("SHIMMERING_NO_JACKETS").unwrap_or_default() == "1")
}

#[inline]
pub fn should_blur_jacket_art() -> bool {
	static CELL: OnceLock<bool> = OnceLock::new();
	*CELL.get_or_init(|| var("SHIMMERING_BLUR_JACKETS").unwrap_or_default() == "1")
}

pub fn get_b30_background() -> &'static ImageBuffer<Rgb<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgb<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		let raw_b30_background = image::open(get_assets_dir().join("b30_background.jpg"))
			.expect("Could not open b30 background");

		raw_b30_background
			.resize(
				8 * raw_b30_background.width(),
				8 * raw_b30_background.height(),
				FilterType::Lanczos3,
			)
			.blur(7.0)
			.into_rgb8()
	})
}

pub fn get_count_background() -> &'static ImageBuffer<Rgba<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgba<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		image::open(get_assets_dir().join("count_background.png"))
			.expect("Could not open count background")
			.into_rgba8()
	})
}

pub fn get_score_background() -> &'static ImageBuffer<Rgba<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgba<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		image::open(get_assets_dir().join("score_background.png"))
			.expect("Could not open score background")
			.into_rgba8()
	})
}

pub fn get_status_background() -> &'static ImageBuffer<Rgba<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgba<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		image::open(get_assets_dir().join("status_background.png"))
			.expect("Could not open status background")
			.into_rgba8()
	})
}

pub fn get_grade_background() -> &'static ImageBuffer<Rgba<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgba<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		image::open(get_assets_dir().join("grade_background.png"))
			.expect("Could not open grade background")
			.into_rgba8()
	})
}

pub fn get_top_backgound() -> &'static ImageBuffer<Rgb<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgb<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		image::open(get_assets_dir().join("top_background.png"))
			.expect("Could not open top background")
			.into_rgb8()
	})
}

pub fn get_name_backgound() -> &'static ImageBuffer<Rgb<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgb<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		image::open(get_assets_dir().join("name_background.png"))
			.expect("Could not open name background")
			.into_rgb8()
	})
}

pub fn get_ptt_emblem() -> &'static ImageBuffer<Rgba<u8>, Vec<u8>> {
	static CELL: OnceLock<ImageBuffer<Rgba<u8>, Vec<u8>>> = OnceLock::new();
	CELL.get_or_init(|| {
		image::open(get_assets_dir().join("ptt_emblem.png"))
			.expect("Could not open ptt emblem")
			.into_rgba8()
	})
}

pub fn get_difficulty_background(
	difficulty: Difficulty,
) -> &'static ImageBuffer<Rgba<u8>, Vec<u8>> {
	static CELL: OnceLock<[ImageBuffer<Rgba<u8>, Vec<u8>>; 5]> = OnceLock::new();
	&CELL.get_or_init(|| {
		let assets_dir = get_assets_dir();
		Difficulty::DIFFICULTY_SHORTHANDS.map(|shorthand| {
			image::open(assets_dir.join(format!("diff_{}.png", shorthand.to_lowercase())))
				.expect(&format!(
					"Could not get background for difficulty {:?}",
					shorthand
				))
				.into_rgba8()
		})
	})[difficulty.to_index()]
}
