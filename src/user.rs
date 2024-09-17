use anyhow::anyhow;
use rusqlite::Row;

use crate::commands::discord::MessageContext;
use crate::context::{Error, UserContext};

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

	pub fn create_from_context(ctx: &impl MessageContext) -> Result<Self, Error> {
		let discord_id = ctx.author_id().to_string();
		let user_id: u32 = ctx
			.data()
			.db
			.get()?
			.prepare_cached(
				"
            INSERT INTO users(discord_id) VALUES (?)
            RETURNING id
        ",
			)?
			.query_map([&discord_id], |row| row.get("id"))?
			.next()
			.ok_or_else(|| anyhow!("Failed to create user"))??;

		Ok(Self {
			discord_id,
			id: user_id,
			is_pookie: false,
		})
	}

	pub fn from_context(ctx: &impl MessageContext) -> Result<Self, Error> {
		let id = ctx.author_id();
		let user = ctx
			.data()
			.db
			.get()?
			.prepare_cached("SELECT * FROM users WHERE discord_id = ?")?
			.query_map([id], Self::from_row)?
			.next()
			.ok_or_else(|| anyhow!("You are not an user in my database, sowwy ^~^"))??;

		Ok(user)
	}

	pub fn by_id(ctx: &UserContext, id: u32) -> Result<Self, Error> {
		let user = ctx
			.db
			.get()?
			.prepare_cached("SELECT * FROM users WHERE id = ?")?
			.query_map([id], Self::from_row)?
			.next()
			.ok_or_else(|| anyhow!("You are not an user in my database, sowwy ^~^"))??;

		Ok(user)
	}
}
