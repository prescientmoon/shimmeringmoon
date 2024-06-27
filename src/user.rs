use std::str::FromStr;

use poise::serenity_prelude::UserId;

use crate::context::{Context, Error};

#[derive(Debug, Clone)]
pub struct User {
	pub id: u32,
	pub discord_id: String,
	pub nickname: Option<String>,
}

impl User {
	pub async fn from_context(ctx: &Context<'_>) -> Result<Self, Error> {
		let id = ctx.author().id.get().to_string();
		let user = sqlx::query!("SELECT * FROM users WHERE discord_id = ?", id)
			.fetch_one(&ctx.data().db)
			.await?;

		Ok(User {
			id: user.id as u32,
			discord_id: user.discord_id,
			nickname: user.nickname,
		})
	}
}

#[inline]
pub async fn discord_it_to_discord_user(
	&ctx: &Context<'_>,
	discord_id: &str,
) -> Result<poise::serenity_prelude::User, Error> {
	UserId::from_str(discord_id)?
		.to_user(ctx.http())
		.await
		.map_err(|e| e.into())
}
