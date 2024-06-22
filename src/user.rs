use crate::context::{Context, Error};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
	pub id: u32,
	pub discord_id: String,
	pub nickname: Option<String>,
}

impl User {
	pub async fn from_context(ctx: &Context<'_>) -> Result<Self, Error> {
		let id = ctx.author().id.get().to_string();
		let user = sqlx::query_as("SELECT * FROM users WHERE discord_id = ?")
			.bind(id)
			.fetch_one(&ctx.data().db)
			.await?;

		Ok(user)
	}
}
