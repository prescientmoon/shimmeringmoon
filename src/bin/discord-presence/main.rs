use std::time::Duration;

use anyhow::anyhow;
// {{{ Imports
use discord_rich_presence::activity::{Activity, Assets};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use shimmeringmoon::arcaea::chart::Difficulty;
use shimmeringmoon::arcaea::play::PlayWithDetails;
use shimmeringmoon::arcaea::score::ScoringSystem;
use shimmeringmoon::context::paths::get_var;
use shimmeringmoon::context::Error;
// }}}

#[tokio::main]
async fn main() -> Result<(), Error> {
	let server_url = get_var("SHIMMERING_SERVER_URL")?;
	let client_id = get_var("SHIMMERING_DISCORD_ID")?;

	println!("Connecting to discord...");
	let mut ipc = DiscordIpcClient::new(&client_id).map_err(|e| anyhow!("{}", e))?;
	ipc.connect().map_err(|e| anyhow!("{}", e))?;

	println!("Starting presence loop...");
	loop {
		println!("Getting most recent score...");
		let res = reqwest::get(format!("{}/plays/latest", server_url)).await;

		let res = match res.and_then(|r| r.error_for_status()) {
			Ok(v) => v,
			Err(e) => {
				ipc.clear_activity().map_err(|e| anyhow!("{}", e))?;
				println!("{e}");

				tokio::time::sleep(Duration::from_secs(10)).await;
				continue;
			}
		};

		let triplet = res.json::<PlayWithDetails>().await?;

		let jacket_url = format!(
			"{}/jackets/by_chart_id/{}.png",
			server_url, &triplet.chart.id
		);
		println!("Jacket url: {}", jacket_url);

		let jacket_text = format!("{} — {}", &triplet.song.title, &triplet.song.artist);

		let assets = Assets::new()
			.large_image(&jacket_url)
			.large_text(&jacket_text);

		let details = format!(
			"{} [{} {}]",
			&triplet.song.title,
			Difficulty::DIFFICULTY_SHORTHANDS[triplet.chart.difficulty.to_index()],
			&triplet.chart.level,
		);

		let state = format!("{}", &triplet.play.score(ScoringSystem::Standard));
		let activity = Activity::new()
			.assets(assets)
			.details(&details)
			.state(&state);

		println!("Sending activity");
		ipc.set_activity(activity).map_err(|e| anyhow!("{}", e))?;
		tokio::time::sleep(Duration::from_secs(30)).await;
	}
}
