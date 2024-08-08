use std::{fs, path::PathBuf, str::FromStr};

use image::{imageops::FilterType, GenericImageView, Rgba};
use num::Integer;

use crate::{
	arcaea::chart::{Difficulty, Jacket, SongCache},
	assets::{get_assets_dir, should_skip_jacket_art},
	context::Error,
	recognition::fuzzy_song_name::guess_chart_name,
};

/// How many sub-segments to split each side into
pub const SPLIT_FACTOR: u32 = 8;
pub const IMAGE_VEC_DIM: usize = (SPLIT_FACTOR * SPLIT_FACTOR * 3) as usize;
pub const BITMAP_IMAGE_SIZE: u32 = 174;

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

pub struct JacketCache {
	jackets: Vec<(u32, ImageVec)>,
}

impl JacketCache {
	// {{{ Generate
	// This is a bit inefficient (using a hash set), but only runs once
	pub fn new(data_dir: &PathBuf, song_cache: &mut SongCache) -> Result<Self, Error> {
		let jacket_dir = data_dir.join("jackets");

		if jacket_dir.exists() {
			fs::remove_dir_all(&jacket_dir).expect("Could not delete jacket dir");
		}

		fs::create_dir_all(&jacket_dir).expect("Could not create jacket dir");

		let jacket_vectors = if should_skip_jacket_art() {
			let path = get_assets_dir().join("placeholder_jacket.jpg");
			let contents: &'static _ = fs::read(path)?.leak();
			let image = image::load_from_memory(contents)?;
			let bitmap: &'static _ = Box::leak(Box::new(
				image
					.resize(BITMAP_IMAGE_SIZE, BITMAP_IMAGE_SIZE, FilterType::Nearest)
					.into_rgb8(),
			));

			for song in song_cache.songs_mut() {
				for chart in song.charts_mut() {
					chart.cached_jacket = Some(Jacket {
						raw: contents,
						bitmap,
					});
				}
			}

			Vec::new()
		} else {
			let entries =
				fs::read_dir(data_dir.join("songs")).expect("Couldn't read songs directory");
			let mut jacket_vectors = vec![];

			for entry in entries {
				let dir = entry?;
				let raw_dir_name = dir.file_name();
				let dir_name = raw_dir_name.to_str().unwrap();
				for entry in fs::read_dir(dir.path()).expect("Couldn't read song directory") {
					let file = entry?;
					let raw_name = file.file_name();
					let name = raw_name.to_str().unwrap().strip_suffix(".jpg").unwrap();

					if !name.ends_with("_256") {
						continue;
					}
					let name = name.strip_suffix("_256").unwrap();

					let difficulty = match name {
						"0" => Some(Difficulty::PST),
						"1" => Some(Difficulty::PRS),
						"2" => Some(Difficulty::FTR),
						"3" => Some(Difficulty::BYD),
						"4" => Some(Difficulty::ETR),
						"base" => None,
						"base_night" => None,
						"base_ja" => None,
						_ => Err(format!("Unknown jacket suffix {}", name))?,
					};

					let (song, chart) = guess_chart_name(dir_name, &song_cache, difficulty, true)?;

					let contents: &'static _ = fs::read(file.path())?.leak();

					let image = image::load_from_memory(contents)?;
					jacket_vectors.push((song.id, ImageVec::from_image(&image)));

					let bitmap: &'static _ = Box::leak(Box::new(
						image
							.resize(BITMAP_IMAGE_SIZE, BITMAP_IMAGE_SIZE, FilterType::Nearest)
							.into_rgb8(),
					));

					if name == "base" {
						let item = song_cache.lookup_mut(song.id).unwrap();

						for chart in item.charts_mut() {
							let difficulty_num = match chart.difficulty {
								Difficulty::PST => "0",
								Difficulty::PRS => "1",
								Difficulty::FTR => "2",
								Difficulty::BYD => "3",
								Difficulty::ETR => "4",
							};

							// We only want to create this path if there's no overwrite for this
							// jacket.
							let specialized_path = PathBuf::from_str(
								&file
									.path()
									.to_str()
									.unwrap()
									.replace("base_night", difficulty_num)
									.replace("base", difficulty_num),
							)
							.unwrap();

							let dest = chart.jacket_path(data_dir);
							if !specialized_path.exists() && !dest.exists() {
								std::os::unix::fs::symlink(file.path(), dest)
									.expect("Could not symlink jacket");
								chart.cached_jacket = Some(Jacket {
									raw: contents,
									bitmap,
								});
							}
						}
					} else if difficulty.is_some() {
						std::os::unix::fs::symlink(file.path(), chart.jacket_path(data_dir))
							.expect("Could not symlink jacket");
						let chart = song_cache.lookup_chart_mut(chart.id).unwrap();
						chart.cached_jacket = Some(Jacket {
							raw: contents,
							bitmap,
						});
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
