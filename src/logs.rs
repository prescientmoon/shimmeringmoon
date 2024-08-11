use std::{env, ops::Deref};

use image::{DynamicImage, EncodableLayout, ImageBuffer, PixelWithColorType};
use poise::serenity_prelude::Timestamp;

use crate::context::Error;

#[inline]
fn should_save_debug_images() -> bool {
	env::var("SHIMMERING_DEBUG_IMGS")
		.map(|s| s == "1")
		.unwrap_or(false)
}

#[inline]
pub fn debug_image_log(image: &DynamicImage) -> Result<(), Error> {
	if should_save_debug_images() {
		image.save(format!("./logs/{}.png", Timestamp::now()))?;
	}

	Ok(())
}

#[inline]
pub fn debug_image_buffer_log<P, C>(image: &ImageBuffer<P, C>) -> Result<(), Error>
where
	P: PixelWithColorType,
	[P::Subpixel]: EncodableLayout,
	C: Deref<Target = [P::Subpixel]>,
{
	if should_save_debug_images() {
		image.save(format!("./logs/{}.png", Timestamp::now()))?;
	}

	Ok(())
}
