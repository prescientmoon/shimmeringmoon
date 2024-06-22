use std::path::PathBuf;

use image::{GenericImageView, Rgba};
use kd_tree::{KdMap, KdPoint};
use num::Integer;

use crate::context::Error;

/// How many sub-segments to split each side into
const SPLIT_FACTOR: u32 = 5;
const IMAGE_VEC_DIM: usize = (SPLIT_FACTOR * SPLIT_FACTOR * 3) as usize;

#[derive(Debug, Clone)]
pub struct ImageVec {
	pub colors: [f32; IMAGE_VEC_DIM],
}

#[derive(Debug, Clone)]
pub struct Jacket {
	pub song_id: u32,
	pub path: PathBuf,
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
	tree: KdMap<ImageVec, Jacket>,
}

impl JacketCache {
	// {{{ Generate tree
	pub fn new(data_dir: &PathBuf) -> Result<Self, Error> {
		let jacket_csv_path = data_dir.join("jackets.csv");
		let mut reader = csv::Reader::from_path(jacket_csv_path)?;

		let mut entries = vec![];

		for record in reader.records() {
			let record = record?;
			let filename = &record[0];
			let song_id = u32::from_str_radix(&record[1], 10)?;
			let image_path = data_dir.join(format!("jackets/{}.png", filename));
			let image = image::io::Reader::open(&image_path)?.decode()?;
			let jacket = Jacket {
				song_id,
				path: image_path,
			};

			entries.push((ImageVec::from_image(&image), jacket))
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
	) -> Option<(f32, &Jacket)> {
		self.tree
			.nearest(&ImageVec::from_image(image))
			.map(|p| (p.squared_distance.sqrt(), &p.item.1))
	}
	// }}}
}
