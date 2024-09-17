use std::{
	fs,
	io::{stdout, Write},
};

use anyhow::{anyhow, bail, Context};
use image::imageops::FilterType;

use crate::{
	arcaea::{
		chart::{Difficulty, SongCache},
		jacket::{ImageVec, BITMAP_IMAGE_SIZE},
	},
	assets::{get_asset_dir, get_data_dir},
	context::{connect_db, Error},
	recognition::fuzzy_song_name::guess_chart_name,
};

#[inline]
fn clear_line() {
	print!("\r                                       \r");
}

pub fn run() -> Result<(), Error> {
	let db = connect_db(&get_data_dir());
	let song_cache = SongCache::new(&db)?;

	let songs_dir = get_asset_dir().join("songs");
	let raw_songs_dir = songs_dir.join("raw");

	let by_id_dir = songs_dir.join("by_id");
	if by_id_dir.exists() {
		fs::remove_dir_all(&by_id_dir).with_context(|| "Could not remove `by_id` dir")?;
	}
	fs::create_dir_all(&by_id_dir).with_context(|| "Could not create `by_id` dir")?;

	let mut jacket_vectors = vec![];

	let entries = fs::read_dir(&raw_songs_dir)
		.with_context(|| "Couldn't read songs directory")?
		.collect::<Result<Vec<_>, _>>()
		.with_context(|| format!("Could not read member of `songs/raw`"))?;

	for (i, dir) in entries.iter().enumerate() {
		let raw_dir_name = dir.file_name();
		let dir_name = raw_dir_name.to_str().unwrap();

		// {{{ Update progress live
		if i != 0 {
			clear_line();
		}

		print!("{}/{}: {dir_name}", i, entries.len());

		if i % 5 == 0 {
			stdout().flush()?;
		}
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
			let search_difficulty =
				if entries.len() > 1 && difficulty.is_none() && dir_name != "tutorial" {
					Some(Difficulty::FTR)
				} else {
					difficulty
				};

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

			jacket_vectors.push((song.id, ImageVec::from_image(&image)));

			let image = image.resize(BITMAP_IMAGE_SIZE, BITMAP_IMAGE_SIZE, FilterType::Gaussian);
			let image_out_path =
				out_dir.join(format!("{difficulty_string}_{BITMAP_IMAGE_SIZE}.jpg"));
			image
				.save(&image_out_path)
				.with_context(|| format!("Could not save image to {image_out_path:?}"))?;
		}
	}

	clear_line();

	// NOTE: this is N^2, but it's a one-off warning thing, so it's fine
	for chart in song_cache.charts() {
		if jacket_vectors.iter().all(|(i, _)| chart.song_id != *i) {
			println!(
				"No jacket found for '{} [{:?}]'",
				song_cache.lookup_song(chart.song_id)?.song.title,
				chart.difficulty
			)
		}
	}

	{
		println!("Encoded {} images", jacket_vectors.len());
		let bytes = postcard::to_allocvec(&jacket_vectors)
			.with_context(|| format!("Coult not encode jacket matrix"))?;
		fs::write(songs_dir.join("recognition_matrix"), bytes)
			.with_context(|| format!("Could not write jacket matrix"))?;
	}

	Ok(())
}
