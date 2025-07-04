use std::fmt::Debug;

use anyhow::{anyhow, Context};
use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::{
	arcaea::{
		chart::Difficulty,
		play::{Play, ScoreCollection},
		score::Score,
	},
	context::{ErrorKind, TagError, TaggedError, UserContext},
	user::User,
};

// {{{ Generic response types
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum MaybeData<T> {
	SomeData(T),
	NoData {},
}

#[derive(Deserialize, Debug, Clone)]
struct PrivateServerResult<T> {
	code: i32,
	msg: String,
	data: MaybeData<T>,
}

#[derive(Deserialize, Default, Debug, Clone, Copy)]
pub struct EmptyResponse {}
// }}}
// {{{ User query types
#[derive(Serialize, Default, Debug)]
pub struct UsersQuery<'a> {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<&'a str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub user_id: Option<u32>,
}

#[derive(Serialize, Default, Debug)]
pub struct UsersQueryOptions<'a> {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub query: Option<UsersQuery<'a>>,
}
// }}}
// {{{ User query response types
#[derive(Deserialize, Debug, Clone)]
pub struct RawUser {
	pub user_id: u32,
	pub user_code: String,
	pub name: String,

	#[serde(rename = "ticket")]
	pub memories: Option<u32>,
}

// }}}
// {{{ User put requests
#[derive(Serialize, Default, Debug)]
pub struct UserPutOptions<'a> {
	#[serde(skip_serializing_if = "Option::is_none", rename = "ticket")]
	pub memories: Option<u32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<&'a str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email: Option<&'a str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub password: Option<&'a str>,
}
// }}}
// {{{ Best score query types
#[derive(Serialize, Debug)]
pub struct BestScoreQuery<'a> {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub song_id: Option<&'a str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub difficulty: Option<u8>,
}

#[derive(Serialize, Default, Debug)]
pub struct BestOptions<'a> {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub query: Option<BestScoreQuery<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub limit: Option<u32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub offset: Option<u32>,
}
// }}}
// {{{ Best score response types
#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
struct RawBestScore {
	best_clear_type: u8,
	clear_type: u8,
	difficulty: u8,
	health: i16,
	modifier: u8, // wtf is this?

	miss_count: u16,
	near_count: u16,
	perfect_count: u16,
	shiny_perfect_count: u16,

	rating: f32,
	score: u32,
	song_id: String,
	time_played: i64,
}

#[derive(Deserialize, Debug, Clone)]
struct RawBestScores {
	data: Vec<RawBestScore>,

	#[allow(unused)]
	user_id: u32,
}
// }}}
// {{{ Helpers
pub fn api_url() -> Result<String, TaggedError> {
	std::env::var("SHIMMERING_PRIVATE_SERVER_URL").map_err(|_| {
		anyhow!("This instance of `shimmeringmoon` is not connected to a private server.")
			.tag(ErrorKind::User)
	})
}

pub fn encode_difficulty(difficulty: Difficulty) -> u8 {
	match difficulty {
		Difficulty::PST => 0,
		Difficulty::PRS => 1,
		Difficulty::FTR => 2,
		Difficulty::BYD => 3,
		Difficulty::ETR => 4,
	}
}

pub fn decode_difficulty(difficulty: u8) -> Option<Difficulty> {
	match difficulty {
		0 => Some(Difficulty::PST),
		1 => Some(Difficulty::PRS),
		2 => Some(Difficulty::FTR),
		3 => Some(Difficulty::BYD),
		4 => Some(Difficulty::ETR),
		_ => None,
	}
}

// }}}
// {{{ Request helper
async fn mk_request<R: serde::de::DeserializeOwned + Debug + Clone>(
	ctx: &UserContext,
	method: reqwest::Method,
	path: &str,
	options: impl serde::Serialize,
) -> Result<R, TaggedError> {
	let url = api_url()?;
	let token = std::env::var("SHIMMERING_PRIVATE_SERVER_TOKEN")
		.map_err(|_| anyhow!("No api token found"))?;

	let mut req = ctx
		.http_client
		.request(method.clone(), format!("{url}/api/v1/{path}"));

	if method == reqwest::Method::GET {
		let mut query_param = BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_string(&options)?);
		query_param.push_str("=="); // Maximum padding, as otherwise python screams at me
		req = req.query(&[("query", query_param)]);
	} else {
		req = req.json(&options)
	}

	let bytes = req
		.header("Token", token)
		.send()
		.await
		.context("Failed to send request")?
		.error_for_status()
		.context("Request has non-ok status")?
		.bytes()
		.await
		.context("Failed to get body bytes")?;

	let decoded = serde_json::from_slice::<PrivateServerResult<R>>(&bytes)
		.context("Failed to decode response")?;

	// TODO: get rid of this .clone
	if let (true, MaybeData::SomeData(inner)) = (decoded.code == 0, decoded.data.clone()) {
		return Ok(inner);
	}

	println!("Raw error response: {}", String::from_utf8_lossy(&bytes));
	Err(anyhow!(
		"The server returned an error: \"{}\". Full response:\n```\n{:?}\n```",
		&decoded.msg,
		&decoded
	)
	.tag(ErrorKind::Internal))
}
// }}}
// {{{ Perform best score request
pub async fn best(
	ctx: &UserContext,
	user: &User,
	options: BestOptions<'_>,
) -> Result<Vec<Play>, TaggedError> {
	let private_user_id = user.private_server_id()?;
	let decoded: RawBestScores = mk_request(
		ctx,
		Method::GET,
		&format!("users/{}/best", private_user_id),
		options,
	)
	.await?;

	let plays = decoded
		.data
		.iter()
		.filter(|raw_play| raw_play.health >= 0 && raw_play.rating > 0.0)
		.map(|raw_play| -> Result<Play, TaggedError> {
			let chart = ctx
				.song_cache
				.charts()
				.find(|chart| {
					let Some(cached_song) = ctx.song_cache.lookup_song(chart.song_id).ok() else {
						return false;
					};

					cached_song.song.shorthand == raw_play.song_id
						&& raw_play.difficulty == encode_difficulty(chart.difficulty)
				})
				.ok_or_else(|| {
					anyhow!("The server returned an unknown song: {}", raw_play.song_id)
						.tag(ErrorKind::User)
				})?;

			Ok(Play {
				id: 0, // External
				created_at: chrono::DateTime::from_timestamp(raw_play.time_played, 0)
					.unwrap()
					.naive_utc(),
				scores: ScoreCollection::from_standard_score(Score(raw_play.score), chart),
				chart_id: chart.id,
				user_id: user.id,
				far_notes: Some(raw_play.near_count as u32),
				max_recall: None,
			})
		})
		.collect::<Result<Vec<_>, _>>()?;

	Ok(plays)
}
// }}}
// {{{ Find usesr
pub async fn users(
	ctx: &UserContext,
	options: UsersQueryOptions<'_>,
) -> Result<Vec<RawUser>, TaggedError> {
	mk_request(ctx, Method::GET, "users", options).await
}

pub async fn get_user(ctx: &UserContext, user: &User) -> Result<RawUser, TaggedError> {
	let private_server_id = user.private_server_id()?;
	mk_request(ctx, Method::GET, &format!("users/{private_server_id}"), ()).await
}
// }}}
// {{{ Update user
pub async fn update_user(
	ctx: &UserContext,
	user: &User,
	options: UserPutOptions<'_>,
) -> Result<EmptyResponse, TaggedError> {
	let private_server_id = user.private_server_id()?;
	mk_request(
		ctx,
		Method::PUT,
		&format!("users/{private_server_id}"),
		options,
	)
	.await
}
// }}}
