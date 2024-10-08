// {{{ Imports
use std::cell::RefCell;
use std::env::var;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{LazyLock, OnceLock};
use std::thread::LocalKey;

use freetype::{Face, Library};
use image::{DynamicImage, RgbaImage};

use crate::arcaea::chart::Difficulty;
use crate::timed;
// }}}

// {{{ Path helpers
#[inline]
pub fn get_var(name: &str) -> String {
	var(name).unwrap_or_else(|_| panic!("Missing `{name}` environment variable"))
}

#[inline]
pub fn get_path(name: &str) -> PathBuf {
	PathBuf::from_str(&get_var(name))
		.unwrap_or_else(|_| panic!("`{name}` environment variable is not a valid path"))
}

#[inline]
pub fn get_data_dir() -> PathBuf {
	get_path("SHIMMERING_DATA_DIR")
}

#[inline]
pub fn get_config_dir() -> PathBuf {
	get_path("SHIMMERING_CONFIG_DIR")
}

#[inline]
pub fn get_asset_dir() -> PathBuf {
	get_path("SHIMMERING_ASSET_DIR")
}
// }}}
// {{{ Font helpers
#[inline]
fn get_font(name: &str) -> RefCell<Face> {
	let fonts_dir = get_path("SHIMMERING_FONTS_DIR");
	let face = FREETYPE_LIB.with(|lib| {
		lib.new_face(fonts_dir.join(name), 0)
			.unwrap_or_else(|_| panic!("Could not load {} font", name))
	});
	RefCell::new(face)
}

#[inline]
pub fn with_font<T>(
	primary: &'static LocalKey<RefCell<Face>>,
	f: impl FnOnce(&mut [&mut Face]) -> T,
) -> T {
	UNI_FONT.with_borrow_mut(|uni| primary.with_borrow_mut(|primary| f(&mut [primary, uni])))
}
// }}}
// {{{ Font loading
// TODO: I might want to embed those into the binary 🤔
thread_local! {
pub static FREETYPE_LIB: Library = Library::init().unwrap();
pub static EXO_FONT: RefCell<Face> = get_font("Exo[wght].ttf");
pub static GEOSANS_FONT: RefCell<Face> = get_font("GeosansLight.ttf");
pub static KAZESAWA_FONT: RefCell<Face> = get_font("Kazesawa-Regular.ttf");
pub static KAZESAWA_BOLD_FONT: RefCell<Face> = get_font("Kazesawa-Bold.ttf");
pub static UNI_FONT: RefCell<Face> = get_font("unifont.otf");
}
// }}}
// {{{ Asset art helpers
#[inline]
#[allow(dead_code)]
pub fn should_blur_jacket_art() -> bool {
	var("SHIMMERING_BLUR_JACKETS").unwrap_or_default() == "1"
}

macro_rules! get_asset {
	($name: ident, $path:expr) => {
		get_asset!($name, $path, |d: DynamicImage| d);
	};
	($name: ident, $path:expr, $f:expr) => {
		pub static $name: LazyLock<RgbaImage> = LazyLock::new(move || {
			timed!($path, {
				let image = image::open(get_asset_dir().join($path))
					.unwrap_or_else(|_| panic!("Could no read asset `{}`", $path));
				let f = $f;
				f(image).into_rgba8()
			})
		});
	};
}
// }}}
// {{{ Asset art loading
get_asset!(COUNT_BACKGROUND, "count_background.png");
get_asset!(SCORE_BACKGROUND, "score_background.png");
get_asset!(STATUS_BACKGROUND, "status_background.png");
get_asset!(GRADE_BACKGROUND, "grade_background.png");
get_asset!(TOP_BACKGROUND, "top_background.png");
get_asset!(NAME_BACKGROUND, "name_background.png");
get_asset!(PTT_EMBLEM, "ptt_emblem.png");
get_asset!(
	B30_BACKGROUND,
	"b30_background.jpg",
	|image: DynamicImage| image.blur(7.0)
);

pub fn get_difficulty_background(difficulty: Difficulty) -> &'static RgbaImage {
	static CELL: OnceLock<[RgbaImage; 5]> = OnceLock::new();
	&CELL.get_or_init(|| {
		timed!("load_difficulty_background", {
			let assets_dir = get_asset_dir();
			Difficulty::DIFFICULTY_SHORTHANDS.map(|shorthand| {
				image::open(assets_dir.join(format!("diff_{}.png", shorthand.to_lowercase())))
					.unwrap_or_else(|_| {
						panic!("Could not get background for difficulty {shorthand:?}")
					})
					.into_rgba8()
			})
		})
	})[difficulty.to_index()]
}
// }}}
