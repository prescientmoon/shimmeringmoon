// {{{ Imports
use std::fmt::Write;
use std::fs;
use std::io::{stdout, Write as IOWrite};

use anyhow::{anyhow, bail, Context};
use faer::Mat;
use image::imageops::FilterType;

use crate::arcaea::chart::{Difficulty, SongCache};
use crate::arcaea::jacket::{
	image_to_vec, read_jackets, JacketCache, BITMAP_IMAGE_SIZE, IMAGE_VEC_DIM,
	JACKET_RECOGNITITION_DIMENSIONS,
};
use crate::context::paths::create_empty_directory;
use crate::recognition::fuzzy_song_name::guess_chart_name;

use super::paths::ShimmeringPaths;
// }}}

/// Runs the entire jacket processing pipeline:
/// 1. Read all the jackets in the input directory, and infer
///    what song/chart they belong to.
/// 2. Save the jackets under a new file structure. The jackets
///    are saved in multiple qualities, together with a blurred version.
/// 3. Ensure we can read the entire jacket tree from the new location.
/// 4. Ensure no charts are missing a jacket.
/// 5. Create a matrix we can use for image recognition.
/// 6. Compress said matrix using singular value decomposition.
/// 7. Ensure the recognition matrix correctly detects every jacket it's given.
/// 8. Finally, save the recognition matrix on disk for future use.
pub fn process_jackets(paths: &ShimmeringPaths, conn: &rusqlite::Connection) -> anyhow::Result<()> {
	let mut song_cache = SongCache::new(conn)?;

	let mut jacket_vector_ids = vec![];
	let mut jacket_vectors = vec![];

	// Contains a dir_name -> song_name map that's useful when debugging
	// name recognition. This will get written to disk in case a missing
	// jacket is detected.
	let mut debug_name_mapping = String::new();

	// {{{ Prepare directories
	let jackets_dir = paths.jackets_path();
	let raw_jackets_dir = paths.raw_jackets_path();

	create_empty_directory(&jackets_dir)?;
	// }}}
	// {{{ Traverse raw songs directory
	let entries = fs::read_dir(&raw_jackets_dir)
		.with_context(|| "Could not list contents of $SHIMMERING_PRIVATE_CONFIG/jackets")?
		.collect::<Result<Vec<_>, _>>()
		.with_context(|| "Could not read member of $SHIMMERING_PRIVATE_CONFIG/jackets")?;

	for (i, dir) in entries.iter().enumerate() {
		let raw_dir_name = dir.file_name();
		let dir_name = raw_dir_name.to_str().unwrap();

		// {{{ Update progress live
		if i != 0 {
			clear_line();
		}

		print!("  🕒 {}/{}: {dir_name}", i, entries.len());
		stdout().flush()?;
		// }}}

		let entries = fs::read_dir(dir.path())
			.with_context(|| "Couldn't read song directory")?
			.map(|f| f.unwrap())
			.filter(|f| !f.file_name().to_str().unwrap().ends_with("_256.jpg"))
			.collect::<Vec<_>>();

		for file in &entries {
			let raw_name = file.file_name();
			let name = raw_name
				.to_str()
				.unwrap()
				.strip_suffix(".jpg")
				.ok_or_else(|| anyhow!("No '.jpg' suffix to remove from filename {raw_name:?}"))?;

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

			let (song, _) = guess_chart_name(dir_name, &song_cache, difficulty, true)
				.with_context(|| format!("Could not recognise chart name from '{dir_name}'"))?;

			writeln!(debug_name_mapping, "{dir_name} -> {}", song.title)?;

			let out_dir = jackets_dir.join(song.id.to_string());
			fs::create_dir_all(&out_dir).with_context(|| {
				format!("Could not create jacket dir for song '{}'", song.title)
			})?;

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
	println!("  ✅ Successfully processed jackets");

	read_jackets(paths, &mut song_cache)?;
	println!("  ✅ Successfully read processed jackets");

	// {{{ Error out on missing jackets
	for chart in song_cache.charts() {
		if chart.cached_jacket.is_none() {
			let out_path = paths.log_dir().join("name_mapping.txt");
			std::fs::write(&out_path, debug_name_mapping)?;

			bail!(
				"No jacket found for '{} [{:?}]'. A complete name map has been written to {out_path:?}",
				song_cache.lookup_song(chart.song_id)?.song,
				chart.difficulty
			)
		}
	}

	println!("  ✅ No missing jackets detected");
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

		print!("  {}/{}: {song}", i, chart_count);

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
	println!("  ✅ Successfully tested jacket recognition");

	// {{{ Save recognition matrix to disk
	{
		println!("  ✅ Encoded {} images", jacket_vectors.len());
		let bytes = postcard::to_allocvec(&jacket_cache)
			.with_context(|| "Coult not encode jacket matrix")?;
		fs::write(paths.recognition_matrix_path(), bytes)
			.with_context(|| "Could not write jacket matrix")?;
	}
	// }}}

	Ok(())
}

/// Hacky function which "clears" the current line of the standard output.
#[inline]
fn clear_line() {
	print!("\r                                                                        \r");
}
