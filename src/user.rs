use anyhow::anyhow;
use rusqlite::Row;

use crate::commands::discord::MessageContext;
use crate::context::{ErrorKind, TagError, TaggedError, UserContext};

#[derive(Debug, Default, Clone)]
pub struct User {
	pub id: u32,
	pub private_server_id: Option<u32>,
	pub discord_id: String,
	pub is_pookie: bool,
	pub is_admin: bool,
}

impl User {
	#[inline]
	fn from_row(row: &Row<'_>) -> Result<Self, rusqlite::Error> {
		Ok(Self {
			id: row.get("id")?,
			private_server_id: row.get("private_server_id")?,
			discord_id: row.get("discord_id")?,
			is_pookie: row.get("is_pookie")?,
			is_admin: row.get("is_admin")?,
		})
	}

	pub fn create_from_context(ctx: &impl MessageContext) -> Result<Self, TaggedError> {
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
			.ok_or_else(|| anyhow!("No id returned from user creation"))??;

		Ok(Self {
			discord_id,
			private_server_id: None,
			id: user_id,
			is_pookie: false,
			is_admin: false,
		})
	}

	pub fn from_context(ctx: &impl MessageContext) -> Result<Self, TaggedError> {
		let id = ctx.author_id();
		let user = ctx
			.data()
			.db
			.get()?
			.prepare_cached("SELECT * FROM users WHERE discord_id = ?")?
			.query_map([id], Self::from_row)?
			.next()
			.ok_or_else(|| {
				anyhow!("You are not an user in my database, sowwy ^~^. Please ask someone on my pookie list to let you in.").tag(ErrorKind::User)
			})??;

		Ok(user)
	}

	pub fn by_id(ctx: &UserContext, id: u32) -> Result<Self, TaggedError> {
		let user = ctx
			.db
			.get()?
			.prepare_cached("SELECT * FROM users WHERE id = ?")?
			.query_map([id], Self::from_row)?
			.next()
			.ok_or_else(|| {
				anyhow!("You are not an user in my database, sowwy ^~^").tag(ErrorKind::User)
			})??;

		Ok(user)
	}

	pub fn by_discord_id(
		ctx: &UserContext,
		id: poise::serenity_prelude::UserId,
	) -> Result<Self, TaggedError> {
		let user = ctx
			.db
			.get()?
			.prepare_cached("SELECT * FROM users WHERE discord_id = ?")?
			.query_map([id.to_string()], Self::from_row)?
			.next()
			.ok_or_else(|| {
				anyhow!("This person is not in my database, sowwy ^~^").tag(ErrorKind::User)
			})??;

		Ok(user)
	}

	#[inline]
	pub fn assert_is_pookie(&self) -> Result<(), TaggedError> {
		if !self.is_pookie && !self.is_admin {
			return Err(
				anyhow!("This feature is reserved for my pookies. Sowwy :3").tag(ErrorKind::User)
			);
		}

		Ok(())
	}

	#[inline]
	pub fn assert_is_admin(&self) -> Result<(), TaggedError> {
		if !self.is_admin {
			return Err(
				anyhow!("This feature is reserved for admins. Sowwy :3").tag(ErrorKind::User)
			);
		}

		Ok(())
	}
}
