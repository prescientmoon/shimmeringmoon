use std::{collections::HashSet, path::PathBuf};

use image::{GenericImageView, Rgba};
use kd_tree::{KdMap, KdPoint};
use num::Integer;

use crate::{chart::SongCache, context::Error};

/// How many sub-segments to split each side into
const SPLIT_FACTOR: u32 = 5;
const IMAGE_VEC_DIM: usize = (SPLIT_FACTOR * SPLIT_FACTOR * 3) as usize;

#[derive(Debug, Clone)]
pub struct ImageVec {
	pub colors: [f32; IMAGE_VEC_DIM],
}

impl ImageVec {
	// {{{ (Image => vector) encoding
	fn from_image(image: &impl GenericImageView<Pixel = Rgba<u8>>) -> ImageVec {
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
				r += pixel.0[0] as u64;
				g += pixel.0[1] as u64;
				b += pixel.0[2] as u64;
				count += 1;
			}

			let count = count as f64;
			let r = r as f64 / count;
			let g = g as f64 / count;
			let b = b as f64 / count;
			colors[i as usize * 3 + 0] = r as f32;
			colors[i as usize * 3 + 1] = g as f32;
			colors[i as usize * 3 + 2] = b as f32;
		}

		Self { colors }
	}
	// }}}
}

impl KdPoint for ImageVec {
	type Dim = typenum::U75;
	type Scalar = f32;

	fn dim() -> usize {
		IMAGE_VEC_DIM
	}

	fn at(&self, i: usize) -> Self::Scalar {
		self.colors[i]
	}
}

pub struct JacketCache {
	// TODO: make this private
	pub tree: KdMap<ImageVec, u32>,
}

impl JacketCache {
	// {{{ Generate tree
	// This is a bit inefficient (using a hash set), but only runs once
	pub fn new(song_cache: &SongCache) -> Result<Self, Error> {
		let mut entries = vec![];
		let mut jackets: HashSet<(&PathBuf, u32)> = HashSet::new();

		for item in song_cache.songs() {
			for chart in item.charts() {
				if let Some(jacket) = &chart.jacket {
					jackets.insert((jacket, item.song.id));
				}
			}
		}

		for (path, song_id) in jackets {
			let image = image::io::Reader::open(path)?.decode()?;
			entries.push((ImageVec::from_image(&image), song_id))
		}

		let result = Self {
			tree: KdMap::build_by_ordered_float(entries),
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
		self.tree
			.nearest(&ImageVec::from_image(image))
			.map(|p| (p.squared_distance.sqrt(), &p.item.1))
	}
	// }}}
}
