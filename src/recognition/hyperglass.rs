//! Hyperglass is my own specialized OCR system.
//!
//! Hyperglass was created as a result of my annoyance with how unreliable
//! tesseract is. Assuming we know the font, OCR should be almost perfect,
//! even when faced with stange kerning. This is what this module achieves!
//!
//! The algorithm is pretty simple:
//! 1. Find the connected components (i.e., "black areas") in the image.
//! 2. Finds the bounding box of each connected component.
//! 3. Discard connected components which are too large (these are likely bars,
//!    or other artifacts).
//! 4. Sort the components by x-position.
//! 5. Compute the largest width & height of the connected components.
//! 5. Split each component (more precisely, start at its top-left corner and
//!    split an area equal to the aforementioned width & height) into a grid of
//!    N^2 chunks (N=5 at the moment), and use that to generate a vector whose
//!    elements represent the percentage of black pixels in each chunk which
//!    belong to the connected component at hand.
//! 6. Normalise the vectors to remain font-weight independent.
//! 7. Find the nearest neighbour of each vector among a list of precomputed
//!    vectors for the font in the image, thus reconstructing the string! The
//!    aforementioned precomputed vectors are generated using almost the exact
//!    procedure described in steps 1-6, except the images are generated at
//!    startup using my very own bitmap rendering module (`crate::bitmap`).
// {{{ Imports
use anyhow::{anyhow, bail};
use freetype::Face;
use image::{DynamicImage, ImageBuffer, Luma};
use imageproc::contrast::{threshold, ThresholdType};
use imageproc::region_labelling::{connected_components, Connectivity};
use num::traits::Euclid;

use crate::bitmap::{Align, BitmapCanvas, Color, TextStyle};
use crate::context::Error;
use crate::logs::{debug_image_buffer_log, debug_image_log};
// }}}

// {{{ ConponentVec
/// How many sub-segments to split each side into
const SPLIT_FACTOR: u32 = 5;
const IMAGE_VEC_DIM: usize = (SPLIT_FACTOR * SPLIT_FACTOR) as usize;

#[derive(Debug, Clone)]
struct ComponentVec {
	chunks: [f32; IMAGE_VEC_DIM],
}

impl ComponentVec {
	// {{{ (Component => vector) encoding
	fn from_component(
		components: &ComponentsWithBounds,
		area: (u32, u32),
		component: u32,
	) -> Result<Self, Error> {
		let mut chunks = [0.0; IMAGE_VEC_DIM];
		let bounds = components
			.bounds
			.get(component as usize - 1)
			.and_then(|o| o.as_ref())
			.ok_or_else(|| anyhow!("Missing bounds for given connected component"))?;

		for i in 0..(SPLIT_FACTOR * SPLIT_FACTOR) {
			let (iy, ix) = i.div_rem_euclid(&SPLIT_FACTOR);

			let x_start = bounds.x_min + ix * area.0 / SPLIT_FACTOR;
			let x_end = bounds.x_min + (ix + 1) * area.0 / SPLIT_FACTOR;
			let y_start = bounds.y_min + iy * area.1 / SPLIT_FACTOR;
			let y_end = bounds.y_min + (iy + 1) * area.1 / SPLIT_FACTOR;
			let mut count = 0;

			for x in x_start..x_end {
				for y in y_start..y_end {
					if let Some(p) = components.components.get_pixel_checked(x, y)
						&& p.0[0] == component
					{
						count += 255 - components.image[(x, y)].0[0] as u32;
					}
				}
			}

			let size = (x_end + 1 - x_start) * (y_end + 1 - y_start);

			if size == 0 {
				bail!("Got zero size for chunk [{x_start},{x_end}]x[{y_start},{y_end}]");
			}

			chunks[i as usize] = count as f32 / size as f32;

			// print!("{} ", chunks[i as usize]);
			// if i % SPLIT_FACTOR == SPLIT_FACTOR - 1 {
			// 	print!("\n");
			// }
		}

		let mut result = Self { chunks };
		result.normalise();
		Ok(result)
	}
	// }}}
	// {{{ Distance
	#[inline]
	fn distance_squared_to(&self, other: &Self) -> f32 {
		let mut total = 0.0;

		for i in 0..IMAGE_VEC_DIM {
			let d = self.chunks[i] - other.chunks[i];
			total += d * d;
		}

		total
	}

	#[inline]
	fn norm_squared(&self) -> f32 {
		let mut total = 0.0;

		for i in 0..IMAGE_VEC_DIM {
			total += self.chunks[i] * self.chunks[i];
		}

		total
	}

	#[inline]
	fn normalise(&mut self) {
		let len = self.norm_squared().sqrt();

		for i in 0..IMAGE_VEC_DIM {
			self.chunks[i] /= len;
		}
	}
	// }}}
}
// }}}
// {{{ Component bounds
#[derive(Clone, Copy)]
struct ComponentBounds {
	x_min: u32,
	y_min: u32,
	x_max: u32,
	y_max: u32,
}

struct ComponentsWithBounds {
	image: ImageBuffer<Luma<u8>, Vec<u8>>,
	components: ImageBuffer<Luma<u32>, Vec<u32>>,

	// NOTE: the index is (the id of the component) - 1
	// This is because the zero component represents the background,
	// but we don't want to waste a place in this vector.
	bounds: Vec<Option<ComponentBounds>>,

	/// Stores the indices of `self.bounds` sorted based on their min position.
	bounds_by_position: Vec<usize>,
}

impl ComponentsWithBounds {
	fn from_image(
		image: &DynamicImage,
		binarisation_threshold: u8,
		max_sizes: (f32, f32),
	) -> Result<Self, Error> {
		let luma_image = image.to_luma8();
		let binarized_image = threshold(&luma_image, binarisation_threshold, ThresholdType::Binary);
		debug_image_buffer_log(&binarized_image);

		let background = Luma([u8::MAX]);
		let components = connected_components(&binarized_image, Connectivity::Eight, background);

		let mut bounds: Vec<Option<ComponentBounds>> = Vec::new();
		for x in 0..components.width() {
			for y in 0..components.height() {
				// {{{ Retrieve pixel if it's not background
				let component = components[(x, y)].0[0];
				if component == 0 {
					continue;
				}

				let index = component as usize - 1;
				if index >= bounds.len() {
					bounds.resize(index + 1, None);
				}
				// }}}
				// {{{ Update bounds
				if let Some(bounds) = (&mut bounds)[index].as_mut() {
					bounds.x_min = bounds.x_min.min(x);
					bounds.x_max = bounds.x_max.max(x);
					bounds.y_min = bounds.y_min.min(y);
					bounds.y_max = bounds.y_max.max(y);
				} else {
					bounds[index] = Some(ComponentBounds {
						x_min: x,
						x_max: x,
						y_min: y,
						y_max: y,
					});
				}
				// }}}
			}
		}

		// {{{ Remove components that are too large
		for bound in &mut bounds {
			if bound.map_or(false, |b| {
				(b.x_max - b.x_min) as f32 >= max_sizes.0 * image.width() as f32
					|| (b.y_max - b.y_min) as f32 >= max_sizes.1 * image.height() as f32
			}) {
				*bound = None;
			}
		}
		// }}}

		let mut bounds_by_position: Vec<usize> = (0..(bounds.len()))
			.filter(|i| bounds[*i].is_some())
			.collect();
		bounds_by_position.sort_by_key(|i| bounds[*i].unwrap().x_min);

		Ok(Self {
			image: luma_image,
			components,
			bounds,
			bounds_by_position,
		})
	}
}
// }}}
// {{{ Char measurements
#[derive(Clone)]
pub struct CharMeasurements {
	chars: Vec<(char, ComponentVec)>,

	max_width: u32,
	max_height: u32,
}

impl CharMeasurements {
	// {{{ Creation
	pub fn from_text(face: &mut Face, string: &str, weight: Option<u32>) -> Result<Self, Error> {
		// These are bad estimates lol
		let style = TextStyle {
			stroke: None,
			drop_shadow: None,
			align: (Align::Start, Align::Start),
			size: 60,
			color: Color::BLACK,
			// TODO: do we want to use the weight hint for resilience?
			weight,
		};
		let padding = (5, 5);
		let planned = BitmapCanvas::plan_text_rendering(padding, &mut [face], style, string)?;

		let mut canvas = BitmapCanvas::new(
			(planned.0 .0) as u32 + planned.1.width + 2 * padding.0 as u32,
			(planned.0 .1) as u32 + planned.1.height + 2 * padding.0 as u32,
		);

		canvas.text(padding, &mut [face], style, string)?;
		let buffer = ImageBuffer::from_raw(canvas.width, canvas.height(), canvas.buffer.to_vec())
			.ok_or_else(|| anyhow!("Failed to turn buffer into canvas"))?;
		let image = DynamicImage::ImageRgb8(buffer);

		debug_image_log(&image);

		let components = ComponentsWithBounds::from_image(&image, 100, (1.0, 1.0))?;

		// {{{ Compute max width/height
		let max_width = components
			.bounds
			.iter()
			.filter_map(|o| o.as_ref())
			.map(|b| b.x_max - b.x_min)
			.max()
			.ok_or_else(|| anyhow!("No connected components found"))?;
		let max_height = components
			.bounds
			.iter()
			.filter_map(|o| o.as_ref())
			.map(|b| b.y_max - b.y_min)
			.max()
			.ok_or_else(|| anyhow!("No connected components found"))?;
		// }}}

		let mut chars = Vec::with_capacity(string.len());
		for (i, char) in string.chars().enumerate() {
			chars.push((
				char,
				ComponentVec::from_component(
					&components,
					(max_width, max_height),
					components.bounds_by_position[i] as u32 + 1,
				)?,
			))
		}

		Ok(Self {
			chars,
			max_width,
			max_height,
		})
	}
	// }}}
	// {{{ Recognition
	pub fn recognise(
		&self,
		image: &DynamicImage,
		whitelist: &str,
		binarisation_threshold: Option<u8>,
		max_sizes: Option<(f32, f32)>,
	) -> Result<String, Error> {
		let components = ComponentsWithBounds::from_image(
			image,
			binarisation_threshold.unwrap_or(100),
			max_sizes.unwrap_or((0.9, 1.0)),
		)?;
		let mut result = String::with_capacity(components.bounds.len());

		let max_height = components
			.bounds
			.iter()
			.filter_map(|o| o.as_ref())
			.map(|b| b.y_max - b.y_min)
			.max()
			.ok_or_else(|| anyhow!("No connected components found"))?;
		let max_width = self.max_width * max_height / self.max_height;

		for i in &components.bounds_by_position {
			let vec =
				ComponentVec::from_component(&components, (max_width, max_height), *i as u32 + 1)?;

			let best_match = self
				.chars
				.iter()
				.filter(|(c, _)| whitelist.contains(*c))
				.map(|(i, v)| (*i, v, v.distance_squared_to(&vec)))
				.min_by(|(_, _, d1), (_, _, d2)| {
					d1.partial_cmp(d2).expect("NaN distance encountered")
				})
				.map(|(i, _, d)| (d.sqrt(), i))
				.ok_or_else(|| anyhow!("No chars in cache"))?;

			// println!("char '{}', distance {}", best_match.1, best_match.0);
			if best_match.0 <= 0.75 {
				result.push(best_match.1);
			}
		}

		Ok(result)
	}
	// }}}
}
// }}}
