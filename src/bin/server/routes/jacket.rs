use std::io::Cursor;

use axum::extract::{Path, State};
use axum::http::{header, HeaderName, StatusCode};

use crate::{context::AppContext, error::AppError};

pub async fn get_jacket_image(
	State(state): State<AppContext>,
	Path(filename): Path<String>,
) -> Result<([(HeaderName, String); 2], Vec<u8>), AppError> {
	let chart_id = filename
		.strip_suffix(".png")
		.unwrap_or(&filename)
		.parse::<u32>()
		.map_err(|e| AppError::new(e.into(), StatusCode::NOT_FOUND))?;

	let (_song, chart) = state
		.ctx
		.song_cache
		.lookup_chart(chart_id)
		.map_err(|e| AppError::new(e, StatusCode::NOT_FOUND))?;

	let headers = [
		(header::CONTENT_TYPE, "image/png".to_owned()),
		(
			header::HeaderName::from_static("pngrok-skip-browser-warning"),
			"-".to_owned(),
		),
		// (
		// 	header::CONTENT_DISPOSITION,
		// 	format!("attachment; filename=\"chart_{}.jpg\"", chart_id),
		// ),
	];
	let mut buffer = Vec::new();
	let mut cursor = Cursor::new(&mut buffer);
	chart
		.cached_jacket
		.unwrap()
		.bitmap
		.write_to(&mut cursor, image::ImageFormat::Png)?;

	Ok((headers, buffer))
}
