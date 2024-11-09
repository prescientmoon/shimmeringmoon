// {{{ Imports
use std::cell::RefCell;
use std::sync::LazyLock;
use std::thread::LocalKey;

use freetype::{Face, Library};
use image::{DynamicImage, RgbaImage};

use crate::arcaea::chart::Difficulty;
// }}}

// {{{ Font helpers
pub type Font = Face<&'static [u8]>;

macro_rules! get_font {
	($name: literal) => {{
		static FONT_CONTENTS: &[u8] =
			include_bytes!(concat!(env!("SHIMMERING_FONT_DIR"), "/", $name));
		let face = FREETYPE_LIB.with(|lib| {
			lib.new_memory_face2(FONT_CONTENTS, 0)
				.unwrap_or_else(|_| panic!("Could not load {} font", $name))
		});
		RefCell::new(face)
	}};
}

#[inline]
pub fn with_font<T>(
	primary: &'static LocalKey<RefCell<Font>>,
	f: impl FnOnce(&mut [&mut Font]) -> T,
) -> T {
	UNI_FONT.with_borrow_mut(|uni| primary.with_borrow_mut(|primary| f(&mut [primary, uni])))
}
// }}}
// {{{ Font loading
thread_local! {
pub static FREETYPE_LIB: Library = Library::init().unwrap();
pub static EXO_FONT: RefCell<Font> = get_font!("Exo[wght].ttf");
pub static GEOSANS_FONT: RefCell<Font> = get_font!("GeosansLight.ttf");
pub static KAZESAWA_FONT: RefCell<Font> = get_font!("Kazesawa-Regular.ttf");
pub static KAZESAWA_BOLD_FONT: RefCell<Font> = get_font!("Kazesawa-Bold.ttf");
pub static UNI_FONT: RefCell<Font> = get_font!("unifont.otf");
}
// }}}
// {{{ Asset art helpers
macro_rules! get_asset {
	($name: ident, $path:expr) => {
		get_asset!($name, $path, "SHIMMERING_ASSET_DIR", |d: DynamicImage| d);
	};
	($name: ident, $path:expr, $env_var: literal, $f:expr) => {
		pub static $name: LazyLock<RgbaImage> = LazyLock::new(move || {
			static IMAGE_BYTES: &[u8] = include_bytes!(concat!(env!($env_var), "/", $path));

			let image = image::load_from_memory(&IMAGE_BYTES)
				.unwrap_or_else(|_| panic!("Could no read asset `{}`", $path));

			let f = $f;
			f(image).into_rgba8()
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
	"SHIMMERING_PRIVATE_CONFIG_DIR",
	|image: DynamicImage| image.blur(7.0)
);

pub fn get_difficulty_background(difficulty: Difficulty) -> &'static RgbaImage {
	get_asset!(PST_BACKGROUND, "diff_pst.png");
	get_asset!(PRS_BACKGROUND, "diff_prs.png");
	get_asset!(FTR_BACKGROUND, "diff_ftr.png");
	get_asset!(ETR_BACKGROUND, "diff_etr.png");
	get_asset!(BYD_BACKGROUND, "diff_byd.png");

	[
		&PST_BACKGROUND,
		&PRS_BACKGROUND,
		&FTR_BACKGROUND,
		&ETR_BACKGROUND,
		&BYD_BACKGROUND,
	][difficulty.to_index()]
}
// }}}
