use std::fs;

use anyhow::Context;
use image::{imageops::FilterType, GenericImageView, Rgba};
use num::Integer;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
	arcaea::chart::{Difficulty, Jacket, SongCache},
	assets::{get_asset_dir, should_skip_jacket_art},
	context::Error,
};

/// How many sub-segments to split each side into
pub const SPLIT_FACTOR: u32 = 8;
pub const IMAGE_VEC_DIM: usize = (SPLIT_FACTOR * SPLIT_FACTOR * 3) as usize;
pub const BITMAP_IMAGE_SIZE: u32 = 174;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageVec {
	#[serde_as(as = "[_; IMAGE_VEC_DIM]")]
	pub colors: [f32; IMAGE_VEC_DIM],
}

impl ImageVec {
	// {{{ (Image => vector) encoding
	pub fn from_image(image: &impl GenericImageView<Pixel = Rgba<u8>>) -> Self {
		let mut colors = [0.0; IMAGE_VEC_DIM];
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
				r += (pixel.0[0] as u64).pow(2);
				g += (pixel.0[1] as u64).pow(2);
				b += (pixel.0[2] as u64).pow(2);
				count += 1;
			}

			let count = count as f64;
			let r = (r as f64 / count).sqrt();
			let g = (g as f64 / count).sqrt();
			let b = (b as f64 / count).sqrt();
			colors[i as usize * 3 + 0] = r as f32;
			colors[i as usize * 3 + 1] = g as f32;
			colors[i as usize * 3 + 2] = b as f32;
		}

		Self { colors }
	}

	#[inline]
	pub fn distance_squared_to(&self, other: &Self) -> f32 {
		let mut total = 0.0;

		for i in 0..IMAGE_VEC_DIM {
			let d = self.colors[i] - other.colors[i];
			total += d * d;
		}

		total
	}
	// }}}
}

#[derive(Clone)]
pub struct JacketCache {
	jackets: Vec<(u32, ImageVec)>,
}

impl JacketCache {
	// {{{ Generate
	// This is a bit inefficient (using a hash set), but only runs once
	pub fn new(song_cache: &mut SongCache) -> Result<Self, Error> {
		let jacket_vectors = if should_skip_jacket_art() {
			let path = get_asset_dir().join("placeholder_jacket.jpg");
			let contents: &'static _ = fs::read(path)?.leak();
			let image = image::load_from_memory(contents)?;
			let bitmap: &'static _ = Box::leak(Box::new(
				image
					.resize(BITMAP_IMAGE_SIZE, BITMAP_IMAGE_SIZE, FilterType::Nearest)
					.into_rgb8(),
			));

			for chart in song_cache.charts_mut() {
				chart.cached_jacket = Some(Jacket {
					raw: contents,
					bitmap,
				});
			}

			Vec::new()
		} else {
			let songs_dir = get_asset_dir().join("songs/by_id");
			let entries =
				fs::read_dir(songs_dir).with_context(|| "Couldn't read songs directory")?;
			let bytes = fs::read(get_asset_dir().join("songs/recognition_matrix"))
				.with_context(|| "Could not read jacket recognition matrix")?;
			let jacket_vectors = postcard::from_bytes(&bytes)
				.with_context(|| "Could not decode jacket recognition matrix")?;

			for entry in entries {
				let dir = entry?;
				let raw_dir_name = dir.file_name();
				let dir_name = raw_dir_name.to_str().unwrap();
				let song_id = dir_name.parse().with_context(|| {
					format!("Dir name {dir_name} could not be parsed as `u32` song id")
				})?;

				let entries =
					fs::read_dir(dir.path()).with_context(|| "Couldn't read song directory")?;
				for entry in entries {
					let file = entry?;
					let raw_name = file.file_name();
					let name = raw_name.to_str().unwrap().strip_suffix(".jpg").unwrap();

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
						chart.cached_jacket = Some(Jacket {
							raw: contents,
							bitmap,
						});
					} else {
						for chart_id in song_cache.lookup_song(song_id)?.charts() {
							let chart = song_cache.lookup_chart_mut(chart_id)?;
							if chart.cached_jacket.is_none() {
								chart.cached_jacket = Some(Jacket {
									raw: contents,
									bitmap,
								});
							}
						}
					}
				}
			}

			jacket_vectors
		};

		let result = Self {
			jackets: jacket_vectors,
		};

		Ok(result)
	}
	// }}}
	// {{{ Recognise
	#[inline]
	pub fn recognise(
		&self,
		image: &impl GenericImageView<Pixel = Rgba<u8>>,
	) -> Option<(f32, &u32)> {
		let vec = ImageVec::from_image(image);
		self.jackets
			.iter()
			.map(|(i, v)| (i, v, v.distance_squared_to(&vec)))
			.min_by(|(_, _, d1), (_, _, d2)| d1.partial_cmp(d2).expect("NaN distance encountered"))
			.map(|(i, _, d)| (d.sqrt(), i))
	}
	// }}}
}
