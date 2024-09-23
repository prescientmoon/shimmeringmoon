//! This file implements the "rotation as shearing" algorithm.
//!
//! The algorithm can rotate images without making use of any trigonometric
//! functions (or working with floats altogether, assuming you don't care
//! about antialiasing).
//!
//! For more information, consult this article:
//! https://www.ocf.berkeley.edu/~fricke/projects/israel/paeth/rotation_by_shearing.html

use image::{DynamicImage, GenericImage, GenericImageView};

use crate::bitmap::{Position, Rect};

fn unsigned_in_bounds(image: &DynamicImage, x: i32, y: i32) -> bool {
	x >= 0 && y >= 0 && image.in_bounds(x as u32, y as u32)
}

/// Performs a horizontal shear operation, without performing anti-aliasing
pub fn xshear(image: &mut DynamicImage, rect: Rect, center: Position, shear: f32) {
	let width = rect.width as i32;
	for y in rect.y..rect.y + rect.height as i32 {
		let skew = (shear * ((y - center.1) as f32)) as i32;
		for i in rect.x..rect.x + width {
			let x = if skew < 0 {
				i
			} else {
				2 * rect.x + width - 1 - i
			};

			if unsigned_in_bounds(image, x, y) {
				let pixel = image.get_pixel(x as u32, y as u32);
				if unsigned_in_bounds(image, x + skew, y) {
					image.put_pixel((x + skew) as u32, y as u32, pixel);
				};
			};
		}
	}
}

/// Performs a vertical shear operation, without performing anti-aliasing
pub fn yshear(image: &mut DynamicImage, rect: Rect, center: Position, shear: f32) {
	let height = rect.height as i32;
	for x in rect.x..rect.x + rect.width as i32 {
		let skew = (shear * ((x - center.0) as f32)) as i32;
		for i in rect.y..rect.y + height {
			let y = if skew < 0 {
				i
			} else {
				2 * rect.y + height - 1 - i
			};

			if unsigned_in_bounds(image, x, y) {
				let pixel = image.get_pixel(x as u32, y as u32);
				if unsigned_in_bounds(image, x, y + skew) {
					image.put_pixel(x as u32, (y + skew) as u32, pixel);
				};
			};
		}
	}
}

/// Performs a rotation as a series of three shear operations.
/// Does not perform anti-aliasing.
pub fn rotate(image: &mut DynamicImage, rect: Rect, center: Position, angle: f32) {
	let alpha = -f32::tan(angle / 2.0);
	let beta = f32::sin(angle);
	xshear(image, rect, center, alpha);
	yshear(image, rect, center, beta);
	xshear(image, rect, center, alpha);
}
