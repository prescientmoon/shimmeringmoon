//! One of the goals of the bot is to never save user-images to disk (for
//! performance and safety reasons), opting to perform operations in-memory
//! instead.
//!
//! While great in practice, this makes debugging much harder. This module
//! allows for a convenient way to throw images into a `logs` directory with
//! a simple env var.

use std::{env, ops::Deref, path::PathBuf, sync::OnceLock, time::Instant};

use image::{DynamicImage, EncodableLayout, ImageBuffer, PixelWithColorType};

use crate::context::paths::get_env_dir_path;

#[inline]
fn should_save_debug_images() -> bool {
	env::var("SHIMMERING_DEBUG_IMGS")
		.map(|s| s == "1")
		.unwrap_or(false)
}

#[inline]
fn get_log_dir() -> PathBuf {
	get_env_dir_path("SHIMMERING_LOG_DIR", "LOGS_DIRECTORY").unwrap()
}

#[inline]
fn get_startup_time() -> Instant {
	static CELL: OnceLock<Instant> = OnceLock::new();
	*CELL.get_or_init(|| Instant::now())
}

#[inline]
pub fn debug_image_log(image: &DynamicImage) {
	if should_save_debug_images() {
		image
			.save(get_log_dir().join(format!(
				"{:0>15}.png",
				get_startup_time().elapsed().as_nanos()
			)))
			.unwrap();
	}
}

#[inline]
pub fn debug_image_buffer_log<P, C>(image: &ImageBuffer<P, C>)
where
	P: PixelWithColorType,
	[P::Subpixel]: EncodableLayout,
	C: Deref<Target = [P::Subpixel]>,
{
	if should_save_debug_images() {
		image
			.save(get_log_dir().join(format!(
				"{:0>15}.png",
				get_startup_time().elapsed().as_nanos()
			)))
			.unwrap();
	}
}
