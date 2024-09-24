use context::AppContext;
use routes::jacket::get_jacket_image;
use routes::recent_plays::get_recent_play;
use shimmeringmoon::assets::get_var;
use shimmeringmoon::context::{Error, UserContext};

mod context;
mod error;
mod routes;

#[tokio::main]
async fn main() -> Result<(), Error> {
	let ctx = Box::leak(Box::new(UserContext::new().await?));

	let app = axum::Router::new()
		.route("/plays/latest", axum::routing::get(get_recent_play))
		.route(
			"/jackets/by_chart_id/:chart_id",
			axum::routing::get(get_jacket_image),
		)
		.with_state(AppContext::new(ctx));

	let port: u32 = get_var("SHIMMERING_SERVER_PORT").parse()?;
	let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
		.await
		.unwrap();

	println!("listening on {}", listener.local_addr().unwrap());

	axum::serve(listener, app).await?;

	Ok(())
}
