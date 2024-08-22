use std::str::FromStr;

use poise::serenity_prelude::UserId;
use rusqlite::Row;

use crate::context::{Context, Error, UserContext};

#[derive(Debug, Clone)]
pub struct User {
	pub id: u32,
	pub discord_id: String,
	pub is_pookie: bool,
}

impl User {
	#[inline]
	fn from_row<'a, 'b>(row: &'a Row<'b>) -> Result<Self, rusqlite::Error> {
		Ok(Self {
			id: row.get("id")?,
			discord_id: row.get("discord_id")?,
			is_pookie: row.get("is_pookie")?,
		})
	}

	pub fn from_context(ctx: &Context<'_>) -> Result<Self, Error> {
		let id = ctx.author().id.get().to_string();
		let user = ctx
			.data()
			.db
			.get()?
			.prepare_cached("SELECT * FROM users WHERE discord_id = ?")?
			.query_map([id], Self::from_row)?
			.next()
			.ok_or_else(|| "You are not an user in my database, sowwy ^~^")??;

		Ok(user)
	}

	pub fn by_id(ctx: &UserContext, id: u32) -> Result<Self, Error> {
		let user = ctx
			.db
			.get()?
			.prepare_cached("SELECT * FROM users WHERE id = ?")?
			.query_map([id], Self::from_row)?
			.next()
			.ok_or_else(|| "You are not an user in my database, sowwy ^~^")??;

		Ok(user)
	}
}

#[inline]
pub async fn discord_id_to_discord_user(
	&ctx: &Context<'_>,
	discord_id: &str,
) -> Result<poise::serenity_prelude::User, Error> {
	UserId::from_str(discord_id)?
		.to_user(ctx.http())
		.await
		.map_err(|e| e.into())
}
