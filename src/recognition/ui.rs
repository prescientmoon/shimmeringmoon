use std::fs;

use anyhow::anyhow;
use image::GenericImage;

use crate::{assets::get_config_dir, bitmap::Rect, context::Error};

// {{{ Rects
#[derive(Debug, Clone, Copy)]
pub enum ScoreScreenRect {
	Score,
	Jacket,
	Difficulty,
	Pure,
	Far,
	Lost,
	MaxRecall,
	Title,
}

#[derive(Debug, Clone, Copy)]
pub enum SongSelectRect {
	Score,
	Jacket,
	Past,
	Present,
	Future,
	Beyond,
}

#[derive(Debug, Clone, Copy)]
pub enum UIMeasurementRect {
	PlayKind,
	ScoreScreen(ScoreScreenRect),
	SongSelect(SongSelectRect),
}

impl UIMeasurementRect {
	#[inline]
	pub fn to_index(self) -> usize {
		match self {
			Self::PlayKind => 0,
			Self::ScoreScreen(ScoreScreenRect::Score) => 1,
			Self::ScoreScreen(ScoreScreenRect::Jacket) => 2,
			Self::ScoreScreen(ScoreScreenRect::Difficulty) => 3,
			Self::ScoreScreen(ScoreScreenRect::Pure) => 4,
			Self::ScoreScreen(ScoreScreenRect::Far) => 5,
			Self::ScoreScreen(ScoreScreenRect::Lost) => 6,
			Self::ScoreScreen(ScoreScreenRect::MaxRecall) => 7,
			Self::ScoreScreen(ScoreScreenRect::Title) => 8,
			Self::SongSelect(SongSelectRect::Score) => 9,
			Self::SongSelect(SongSelectRect::Jacket) => 10,
			Self::SongSelect(SongSelectRect::Past) => 11,
			Self::SongSelect(SongSelectRect::Present) => 12,
			Self::SongSelect(SongSelectRect::Future) => 13,
			Self::SongSelect(SongSelectRect::Beyond) => 14,
		}
	}
}

pub const UI_RECT_COUNT: usize = 15;
// }}}
// {{{ Measurement
#[derive(Debug, Clone)]
pub struct UIMeasurement {
	dimensions: [u32; 2],
	datapoints: [u32; UI_RECT_COUNT * 4],
}

impl Default for UIMeasurement {
	fn default() -> Self {
		Self::new([0; 2], [0; UI_RECT_COUNT * 4])
	}
}

impl UIMeasurement {
	pub fn new(dimensions: [u32; 2], datapoints: [u32; UI_RECT_COUNT * 4]) -> Self {
		Self {
			dimensions,
			datapoints,
		}
	}

	#[inline]
	pub fn aspect_ratio(&self) -> f32 {
		self.dimensions[0] as f32 / self.dimensions[1] as f32
	}
}
// }}}
// {{{ Measurements
#[derive(Debug, Clone)]
pub struct UIMeasurements {
	pub measurements: Vec<UIMeasurement>,
}

impl UIMeasurements {
	// {{{ Read
	pub fn read() -> Result<Self, Error> {
		let mut measurements = Vec::new();
		let mut measurement = UIMeasurement::default();

		let path = get_config_dir().join("ui.txt");
		let contents = fs::read_to_string(path)?;

		// {{{ Parse measurement file
		for (i, line) in contents.split('\n').enumerate() {
			let i = i % (UI_RECT_COUNT + 2);
			if i == 0 {
				for (j, str) in line.split_whitespace().enumerate().take(2) {
					measurement.dimensions[j] = u32::from_str_radix(str, 10)?;
				}
			} else if i == UI_RECT_COUNT + 1 {
				measurements.push(measurement);
				measurement = UIMeasurement::default();
			} else {
				for (j, str) in line.split_whitespace().enumerate().take(4) {
					measurement.datapoints[(i - 1) * 4 + j] = u32::from_str_radix(str, 10)?;
				}
			}
		}
		// }}}

		measurements.sort_by_key(|r| (r.aspect_ratio() * 1000.0) as u32);

		// {{{ Filter datapoints that are close together
		let mut i = 0;
		while i < measurements.len() - 1 {
			let low = &measurements[i];
			let high = &measurements[i + 1];

			if (low.aspect_ratio() - high.aspect_ratio()).abs() < 0.001 {
				// TODO: we could interpolate here but oh well
				measurements.remove(i + 1);
			}

			i += 1;
		}
		// }}}

		println!("Read {} UI measurements", measurements.len());
		Ok(Self { measurements })
	}
	// }}}
	// {{{ Interpolate
	pub fn interpolate(
		&self,
		rect: UIMeasurementRect,
		image: &impl GenericImage,
	) -> Result<Rect, Error> {
		let aspect_ratio = image.width() as f32 / image.height() as f32;
		let r = rect.to_index();

		for i in 0..(self.measurements.len() - 1) {
			let low = &self.measurements[i];
			let high = &self.measurements[i + 1];

			let low_ratio = low.aspect_ratio();
			let high_ratio = high.aspect_ratio();

			if (i == 0 || low_ratio <= aspect_ratio)
				&& (aspect_ratio <= high_ratio || i == self.measurements.len() - 2)
			{
				let dimensions = [image.width(), image.height()];
				let p = (aspect_ratio - low_ratio) / (high_ratio - low_ratio);
				let mut out = [0; 4];
				for j in 0..4 {
					let l = low.datapoints[4 * r + j] as f32 / low.dimensions[j % 2] as f32;
					let h = high.datapoints[4 * r + j] as f32 / high.dimensions[j % 2] as f32;
					out[j] = ((l + (h - l) * p) * dimensions[j % 2] as f32) as u32;
				}

				return Ok(Rect::new(out[0] as i32, out[1] as i32, out[2], out[3]));
			}
		}

		Err(anyhow!("Could no find rect for {rect:?} in image"))
	}
	// }}}
}
// }}}
