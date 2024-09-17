use std::{
	cell::RefCell,
	env::var,
	path::PathBuf,
	str::FromStr,
	sync::{LazyLock, OnceLock},
	thread::LocalKey,
};

use freetype::{Face, Library};
use image::{DynamicImage, RgbaImage};

use crate::{arcaea::chart::Difficulty, timed};

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
	let face = FREETYPE_LIB.with(|lib| {
		lib.new_face(get_asset_dir().join("fonts").join(name), 0)
			.expect(&format!("Could not load {} font", name))
	});
	RefCell::new(face)
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
// }}}
// {{{ Font loading
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
// }}}
// {{{ Asset art helpers
#[inline]
pub fn should_skip_jacket_art() -> bool {
	var("SHIMMERING_NO_JACKETS").unwrap_or_default() == "1"
}

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
