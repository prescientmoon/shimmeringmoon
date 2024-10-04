// {{{ Imports
use std::fs;
use std::io::{stdout, Write};

use anyhow::{anyhow, bail, Context};
use faer::Mat;
use image::imageops::FilterType;

use shimmeringmoon::arcaea::chart::{Difficulty, SongCache};
use shimmeringmoon::arcaea::jacket::{
	image_to_vec, read_jackets, JacketCache, BITMAP_IMAGE_SIZE, IMAGE_VEC_DIM,
	JACKET_RECOGNITITION_DIMENSIONS,
};
use shimmeringmoon::assets::{get_asset_dir, get_data_dir};
use shimmeringmoon::context::{connect_db, Error};
use shimmeringmoon::recognition::fuzzy_song_name::guess_chart_name;
// }}}

/// Hacky function which clears the current line of the standard output.
#[inline]
fn clear_line() {
	print!("\r                                                                        \r");
}

pub fn run() -> Result<(), Error> {
	let db = connect_db(&get_data_dir());
	let mut song_cache = SongCache::new(&db)?;

	let mut jacket_vector_ids = vec![];
	let mut jacket_vectors = vec![];

	// {{{ Prepare directories
	let songs_dir = get_asset_dir().join("songs");
	let raw_songs_dir = songs_dir.join("raw");

	let by_id_dir = songs_dir.join("by_id");
	if by_id_dir.exists() {
		fs::remove_dir_all(&by_id_dir).with_context(|| "Could not remove `by_id` dir")?;
	}
	fs::create_dir_all(&by_id_dir).with_context(|| "Could not create `by_id` dir")?;
	// }}}
	// {{{ Traverse raw songs directory
	let entries = fs::read_dir(&raw_songs_dir)
		.with_context(|| "Couldn't read songs directory")?
		.collect::<Result<Vec<_>, _>>()
		.with_context(|| "Could not read member of `songs/raw`")?;

	for (i, dir) in entries.iter().enumerate() {
		let raw_dir_name = dir.file_name();
		let dir_name = raw_dir_name.to_str().unwrap();

		// {{{ Update progress live
		if i != 0 {
			clear_line();
		}

		print!("{}/{}: {dir_name}", i, entries.len());
		stdout().flush()?;
		// }}}

		let entries = fs::read_dir(dir.path())
			.with_context(|| "Couldn't read song directory")?
			.map(|f| f.unwrap())
			.filter(|f| f.file_name().to_str().unwrap().ends_with("_256.jpg"))
			.collect::<Vec<_>>();

		for file in &entries {
			let raw_name = file.file_name();
			let name = raw_name
				.to_str()
				.unwrap()
				.strip_suffix("_256.jpg")
				.ok_or_else(|| {
					anyhow!("No '_256.jpg' suffix to remove from filename {raw_name:?}")
				})?;

			let difficulty = match name {
				"0" => Some(Difficulty::PST),
				"1" => Some(Difficulty::PRS),
				"2" => Some(Difficulty::FTR),
				"3" => Some(Difficulty::BYD),
				"4" => Some(Difficulty::ETR),
				"base" => None,
				"base_night" => None,
				"base_ja" => None,
				_ => bail!("Unknown jacket suffix {}", name),
			};

			// Sometimes it's useful to distinguish between separate (but related)
			// charts like "Vicious Heroism" and "Vicious [ANTi] Heroism" being in
			// the same directory. To do this, we only allow the base jacket to refer
			// to the FUTURE difficulty, unless it's the only jacket present
			// (or unless we are parsing the tutorial)
			let search_difficulty = difficulty;

			let (song, _) = guess_chart_name(dir_name, &song_cache, search_difficulty, true)
				.with_context(|| format!("Could not recognise chart name from '{dir_name}'"))?;

			// {{{ Set up `out_dir` paths
			let out_dir = {
				let out = by_id_dir.join(song.id.to_string());
				if !out.exists() {
					fs::create_dir_all(&out).with_context(|| {
						format!(
							"Could not create parent dir for song '{}' inside `by_id`",
							song.title
						)
					})?;
				}

				out
			};
			// }}}

			let difficulty_string = if let Some(difficulty) = difficulty {
				&Difficulty::DIFFICULTY_SHORTHANDS[difficulty.to_index()].to_lowercase()
			} else {
				"def"
			};

			let contents: &'static _ = fs::read(file.path())
				.with_context(|| format!("Could not read image for file {:?}", file.path()))?
				.leak();
			let image = image::load_from_memory(contents)?;
			let small_image =
				image.resize(BITMAP_IMAGE_SIZE, BITMAP_IMAGE_SIZE, FilterType::Gaussian);

			jacket_vector_ids.push(song.id);
			jacket_vectors.push(image_to_vec(&image));

			{
				let image_small_path =
					out_dir.join(format!("{difficulty_string}_{BITMAP_IMAGE_SIZE}.jpg"));
				small_image
					.save(&image_small_path)
					.with_context(|| format!("Could not save image to {image_small_path:?}"))?;
			}

			{
				let image_full_path = out_dir.join(format!("{difficulty_string}_full.jpg"));
				image
					.save(&image_full_path)
					.with_context(|| format!("Could not save image to {image_full_path:?}"))?;
			}

			{
				let blurred_out_path = out_dir.join(format!("{difficulty_string}_blurred.jpg"));
				small_image
					.blur(27.5)
					.save(&blurred_out_path)
					.with_context(|| format!("Could not save image to {blurred_out_path:?}"))?;
			}
		}
	}
	// }}}

	clear_line();
	println!("Successfully processed jackets");

	read_jackets(&mut song_cache)?;
	println!("Successfully read jackets");

	// {{{ Warn on missing jackets
	for chart in song_cache.charts() {
		if chart.cached_jacket.is_none() {
			println!(
				"No jacket found for '{} [{:?}]'",
				song_cache.lookup_song(chart.song_id)?.song,
				chart.difficulty
			)
		}
	}

	println!("No missing jackets detected");
	// }}}
	// {{{ Compute jacket vec matrix
	let mut jacket_matrix: Mat<f32> = Mat::zeros(IMAGE_VEC_DIM, jacket_vectors.len());

	for (i, v) in jacket_vectors.iter().enumerate() {
		jacket_matrix.subcols_mut(i, 1).copy_from(v);
	}
	// }}}
	// {{{ Compute transform matrix
	let transform_matrix = {
		let svd = jacket_matrix.thin_svd();

		svd.u()
			.transpose()
			.submatrix(0, 0, JACKET_RECOGNITITION_DIMENSIONS, IMAGE_VEC_DIM)
			.to_owned()
	};
	// }}}
	// {{{ Build jacket cache
	let jacket_cache = JacketCache {
		jacket_ids: jacket_vector_ids,
		jacket_matrix: &transform_matrix * &jacket_matrix,
		transform_matrix,
	};
	// }}}

	// {{{ Perform jacket recognition test
	let chart_count = song_cache.charts().count();
	for (i, chart) in song_cache.charts().enumerate() {
		let song = &song_cache.lookup_song(chart.song_id)?.song;

		// {{{ Update console display
		if i != 0 {
			clear_line();
		}

		print!("{}/{}: {song}", i, chart_count);

		if i % 5 == 0 {
			stdout().flush()?;
		}
		// }}}

		if let Some(jacket) = chart.cached_jacket {
			if let Some((_, song_id)) = jacket_cache.recognise(jacket.bitmap) {
				if song_id != song.id {
					let mistake = &song_cache.lookup_song(song_id)?.song;
					bail!(
						"Could not recognise jacket for {song} [{}]. Found song {mistake} instead.",
						chart.difficulty
					)
				}
			} else {
				bail!(
					"Could not recognise jacket for {song} [{}].",
					chart.difficulty
				)
			}
		}
	}
	// }}}

	clear_line();
	println!("Successfully tested jacket recognition");

	// {{{ Save recognition matrix to disk
	{
		println!("Encoded {} images", jacket_vectors.len());
		let bytes = postcard::to_allocvec(&jacket_cache)
			.with_context(|| "Coult not encode jacket matrix")?;
		fs::write(songs_dir.join("recognition_matrix"), bytes)
			.with_context(|| "Could not write jacket matrix")?;
	}
	// }}}

	Ok(())
}
