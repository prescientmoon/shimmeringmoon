use axum::{
	http::StatusCode,
	response::{IntoResponse, Response},
};

pub struct AppError {
	pub error: anyhow::Error,
	pub status_code: StatusCode,
}

impl AppError {
	pub fn new(error: anyhow::Error, status_code: StatusCode) -> Self {
		Self { error, status_code }
	}
}

impl IntoResponse for AppError {
	fn into_response(self) -> Response {
		(
			self.status_code,
			format!("Something went wrong: {}", self.error),
		)
			.into_response()
	}
}

impl<E> From<E> for AppError
where
	E: Into<anyhow::Error>,
{
	fn from(err: E) -> Self {
		Self::new(err.into(), StatusCode::INTERNAL_SERVER_ERROR)
	}
}
