// {{{ Imports
use std::fs;

use anyhow::Context;
use faer::{Mat, MatRef};
use image::{GenericImageView, Pixel};
use num::{Integer, ToPrimitive};
use serde::{Deserialize, Serialize};

use crate::arcaea::chart::{Difficulty, Jacket, SongCache};
use crate::assets::get_asset_dir;
use crate::context::Error;
// }}}

/// How many sub-segments to split each side into
pub const SPLIT_FACTOR: u32 = 8;
pub const IMAGE_VEC_DIM: usize = (SPLIT_FACTOR * SPLIT_FACTOR * 3) as usize;
pub const BITMAP_IMAGE_SIZE: u32 = 174;
pub const JACKET_RECOGNITITION_DIMENSIONS: usize = 10;

// {{{ (Image => vector) encoding
#[allow(clippy::identity_op)]
pub fn image_to_vec(image: &impl GenericImageView) -> MVec<f32> {
	let mut colors = MVec::zeros(IMAGE_VEC_DIM, 1);
	let chunk_width = image.width() / SPLIT_FACTOR;
	let chunk_height = image.height() / SPLIT_FACTOR;
	for i in 0..(SPLIT_FACTOR * SPLIT_FACTOR) {
		let (iy, ix) = i.div_rem(&SPLIT_FACTOR);
		let cropped = image.view(
			chunk_width * ix,
			chunk_height * iy,
			chunk_width,
			chunk_height,
		);

		let mut r = 0;
		let mut g = 0;
		let mut b = 0;
		let mut count = 0;

		for (_, _, pixel) in cropped.pixels() {
			let channels = pixel.channels();

			// I'm not sure this does what it's supposed to do for non rgb(a) pixels...
			r += channels[0].to_u64().unwrap().pow(2);
			g += channels[1].to_u64().unwrap().pow(2);
			b += channels[2].to_u64().unwrap().pow(2);

			count += 1;
		}

		let count = count as f64;
		let r = (r as f64 / count).sqrt();
		let g = (g as f64 / count).sqrt();
		let b = (b as f64 / count).sqrt();
		colors[(i as usize * 3 + 0, 0)] = r as f32;
		colors[(i as usize * 3 + 1, 0)] = g as f32;
		colors[(i as usize * 3 + 2, 0)] = b as f32;
	}

	colors
}
// }}}

/// A column vector
pub type MVec<T> = Mat<T>;

/// This struct holds:
/// - a set of (song_id, vec) pairs of different images projected through the
///   aforementioned transform.
/// - an projection matrix for dimensionality reduction
#[derive(Clone, Serialize, Deserialize)]
pub struct JacketCache {
	/// A matrix with each column corresponding to the result of passing a jacket
	/// through [[image_to_vec]], and then projecting it through `transform_matrix`
	pub jacket_matrix: Mat<f32>,

	/// Assigns each column of `jacket_matrix` a song id.
	pub jacket_ids: Vec<u32>,

	/// A projection matrix for dimensionality reduction.
	pub transform_matrix: Mat<f32>,
}

// {{{ Read jackets
pub fn read_jackets(song_cache: &mut SongCache) -> Result<(), Error> {
	let suffix = format!("_{BITMAP_IMAGE_SIZE}.jpg");
	let songs_dir = get_asset_dir().join("songs/by_id");
	let entries = fs::read_dir(songs_dir).with_context(|| "Couldn't read songs directory")?;

	for entry in entries {
		let dir = entry?;
		let raw_dir_name = dir.file_name();
		let dir_name = raw_dir_name.to_str().unwrap();
		let song_id = dir_name
			.parse()
			.with_context(|| format!("Dir name {dir_name} could not be parsed as `u32` song id"))?;

		let entries = fs::read_dir(dir.path()).with_context(|| "Couldn't read song directory")?;
		for entry in entries {
			let file = entry?;
			let raw_name = file.file_name();
			let name = raw_name.to_str().unwrap();
			if !name.ends_with(&suffix) {
				continue;
			}

			let name = name.strip_suffix(&suffix).unwrap();

			let difficulty = Difficulty::DIFFICULTY_SHORTHANDS
				.iter()
				.zip(Difficulty::DIFFICULTIES)
				.find_map(|(s, d)| Some(d).filter(|_| name == s.to_lowercase()));

			let contents: &'static _ = fs::read(file.path())
				.with_context(|| "Coult not read prepared jacket image")?
				.leak();

			let image = image::load_from_memory(contents)
				.with_context(|| "Could not load jacket image from prepared bytes")?;
			let bitmap: &'static _ = Box::leak(Box::new(image.into_rgb8()));

			if let Some(difficulty) = difficulty {
				let chart = song_cache
					.lookup_by_difficulty_mut(song_id, difficulty)
					.unwrap();
				chart.jacket_source = Some(difficulty);
				chart.cached_jacket = Some(Jacket {
					raw: contents,
					bitmap,
				});
			} else {
				for (_, chart_id) in song_cache.lookup_song(song_id)?.charts() {
					let chart = song_cache.lookup_chart_mut(chart_id)?;
					if chart.jacket_source.is_none() {
						chart.cached_jacket = Some(Jacket {
							raw: contents,
							bitmap,
						});
						chart.jacket_source = None;
					}
				}
			}
		}
	}

	Ok(())
}
// }}}

impl JacketCache {
	// {{{ Generate
	pub fn new() -> Result<Self, Error> {
		let bytes = fs::read(get_asset_dir().join("songs/recognition_matrix"))
			.with_context(|| "Could not read jacket recognition matrix")?;

		let result = postcard::from_bytes(&bytes)?;
		// .with_context(|| "Could not decode jacket recognition matrix")?;

		Ok(result)
	}
	// }}}
	// {{{ Recognise
	/// Transforms a vector from image space to recognition space.
	#[inline]
	pub fn transform_vec(&self, vec: MatRef<f32>) -> MVec<f32> {
		&self.transform_matrix * vec
	}

	#[inline]
	pub fn recognise(&self, image: &impl GenericImageView) -> Option<(f32, u32)> {
		let vec = self.transform_vec(image_to_vec(image).as_ref());
		self.jacket_ids
			.iter()
			.enumerate()
			.map(|(idx, id)| {
				(id, {
					(self.jacket_matrix.subcols(idx, 1) - &vec).squared_norm_l2()
				})
			})
			.min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).expect("NaN distance encountered"))
			.map(|(i, d)| (d.sqrt(), *i))
	}
	// }}}
}
