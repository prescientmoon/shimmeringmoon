// {{{ Imports
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::str::FromStr;

extern crate shimmeringmoon;
use poise::CreateReply;
use shimmeringmoon::assets::get_var;
use shimmeringmoon::commands::discord::mock::ReplyEssence;
use shimmeringmoon::context::Error;
use shimmeringmoon::{commands::discord::MessageContext, context::UserContext};
// }}}

/// Similar in scope to [crate::commands::discord::mock::MockContext],
/// except replies and messages are printed to the standard output.
///
/// Attachments are ignored, and [CreateMessage] values are printed
/// as TOML.
pub struct CliContext {
	pub user_id: u64,
	pub data: UserContext,
}

impl CliContext {
	pub fn new(data: UserContext) -> Self {
		Self {
			data,
			user_id: get_var("SHIMMERING_DISCORD_USER_ID")
				.parse()
				.expect("invalid user id"),
		}
	}
}

impl MessageContext for CliContext {
	fn author_id(&self) -> u64 {
		self.user_id
	}

	async fn fetch_user(&self, discord_id: &str) -> Result<poise::serenity_prelude::User, Error> {
		let mut user = poise::serenity_prelude::User::default();
		user.id = poise::serenity_prelude::UserId::from_str(discord_id)?;
		user.name = "shimmeringuser".to_string();
		Ok(user)
	}

	fn data(&self) -> &UserContext {
		&self.data
	}

	async fn reply(&mut self, text: &str) -> Result<(), Error> {
		println!("[Reply] {text}");
		Ok(())
	}

	async fn send(&mut self, message: CreateReply) -> Result<(), Error> {
		let all = toml::to_string(&ReplyEssence::from_reply(message)).unwrap();
		println!("\n========== Message ==========");
		println!("{all}");
		Ok(())
	}

	// {{{ Input attachments
	type Attachment = PathBuf;

	fn filename(attachment: &Self::Attachment) -> &str {
		attachment.file_name().unwrap().to_str().unwrap()
	}

	// This is a dumb implementation, but it works for testing...
	fn is_image(attachment: &Self::Attachment) -> bool {
		let ext = attachment.extension().unwrap();
		ext == "png" || ext == "jpg" || ext == "webp"
	}

	fn attachment_id(_attachment: &Self::Attachment) -> NonZeroU64 {
		NonZeroU64::new(666).unwrap()
	}

	async fn download(&self, attachment: &Self::Attachment) -> Result<Vec<u8>, Error> {
		let res = tokio::fs::read(attachment).await?;
		Ok(res)
	}
	// }}}
}
