// {{{ Imports
use std::num::NonZeroU64;
use std::str::FromStr;

use poise::serenity_prelude::futures::future::join_all;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed};
use poise::CreateReply;

use crate::arcaea::play::Play;
use crate::context::{Error, ErrorKind, TaggedError, UserContext};
use crate::timed;
// }}}

// {{{ Trait
pub trait MessageContext {
	/// Get the user context held by the message
	fn data(&self) -> &UserContext;
	fn author_id(&self) -> u64;

	/// Fetch info about a user given it's id.
	async fn fetch_user(&self, discord_id: &str) -> Result<poise::serenity_prelude::User, Error>;

	/// Reply to the current message
	async fn reply(&mut self, text: &str) -> Result<(), Error>;

	/// Deliver a message
	async fn send(&mut self, message: CreateReply) -> Result<(), Error>;

	// {{{ Input attachments
	type Attachment;

	fn is_image(attachment: &Self::Attachment) -> bool;
	fn filename(attachment: &Self::Attachment) -> &str;
	fn attachment_id(attachment: &Self::Attachment) -> NonZeroU64;

	/// Downloads a single file.
	async fn download(&self, attachment: &Self::Attachment) -> Result<Vec<u8>, Error>;

	/// Downloads every image
	async fn download_images<'a>(
		&self,
		attachments: &'a [Self::Attachment],
	) -> Result<Vec<(&'a Self::Attachment, Vec<u8>)>, Error> {
		let download_tasks = attachments
			.iter()
			.filter(|file| Self::is_image(file))
			.map(|file| async move { (file, self.download(file).await) });

		let downloaded = timed!("dowload_files", { join_all(download_tasks).await });
		downloaded
			.into_iter()
			.map(|(file, bytes)| Ok((file, bytes?)))
			.collect::<Result<_, Error>>()
	}
	// }}}
	// {{{ Erorr handling
	async fn handle_error<V>(&mut self, res: Result<V, TaggedError>) -> Result<Option<V>, Error> {
		match res {
			Ok(v) => Ok(Some(v)),
			Err(e) => match e.kind {
				ErrorKind::Internal => Err(e.error),
				ErrorKind::User => {
					self.reply(&format!("{}", e.error)).await?;
					Ok(None)
				}
			},
		}
	}
	// }}}
}
// }}}
// {{{ Poise implementation
impl MessageContext for poise::Context<'_, UserContext, Error> {
	type Attachment = poise::serenity_prelude::Attachment;

	fn data(&self) -> &UserContext {
		Self::data(*self)
	}

	fn author_id(&self) -> u64 {
		self.author().id.get()
	}

	async fn fetch_user(&self, discord_id: &str) -> Result<poise::serenity_prelude::User, Error> {
		poise::serenity_prelude::UserId::from_str(discord_id)?
			.to_user(self.http())
			.await
			.map_err(|e| e.into())
	}

	async fn reply(&mut self, text: &str) -> Result<(), Error> {
		Self::reply(*self, text).await?;
		Ok(())
	}

	async fn send(&mut self, message: CreateReply) -> Result<(), Error> {
		poise::send_reply(*self, message).await?;
        poise::say_repl
		Ok(())
	}

	// {{{ Input attachments
	fn attachment_id(attachment: &Self::Attachment) -> NonZeroU64 {
		NonZeroU64::new(attachment.id.get()).unwrap()
	}

	fn filename(attachment: &Self::Attachment) -> &str {
		&attachment.filename
	}

	fn is_image(attachment: &Self::Attachment) -> bool {
		attachment.dimensions().is_some()
	}

	async fn download(&self, attachment: &Self::Attachment) -> Result<Vec<u8>, Error> {
		let res = poise::serenity_prelude::Attachment::download(attachment).await?;
		Ok(res)
	}
	// }}}
}
// }}}
// {{{ Testing context
pub mod mock {
	use std::{
		env, fs,
		path::{Path, PathBuf},
	};

	use anyhow::Context;
	use poise::serenity_prelude::CreateEmbed;
	use serde::{Deserialize, Serialize};
	use sha2::{Digest, Sha256};

	use super::*;

	// {{{ Message essences
	/// Holds test-relevant data about an attachment.
	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct AttachmentEssence {
		filename: String,
		description: Option<String>,
		/// SHA-256 hash of the file
		hash: String,
	}

	impl AttachmentEssence {
		pub fn new(filename: String, description: Option<String>, data: &[u8]) -> Self {
			Self {
				filename,
				description,
				hash: {
					let hash = Sha256::digest(data);
					let string = base16ct::lower::encode_string(&hash);

					// We allocate twice, but it's only for testing,
					// so it should be fineeeeeeee
					format!("sha256_{string}")
				},
			}
		}
	}

	/// Holds test-relevant data about a reply.
	#[derive(Debug, Clone, Serialize)]
	pub struct ReplyEssence {
		reply: bool,
		ephermal: Option<bool>,
		content: Option<String>,
		embeds: Vec<CreateEmbed>,
		attachments: Vec<AttachmentEssence>,
	}

	impl ReplyEssence {
		pub fn from_reply(message: CreateReply) -> Self {
			ReplyEssence {
				reply: message.reply,
				ephermal: message.ephemeral,
				content: message.content,
				embeds: message.embeds,
				attachments: message
					.attachments
					.into_iter()
					.map(|attachment| {
						AttachmentEssence::new(
							attachment.filename,
							attachment.description,
							&attachment.data,
						)
					})
					.collect(),
			}
		}
	}
	// }}}
	// {{{ Mock context
	/// A mock context usable for testing. Messages and attachments are
	/// accumulated inside a vec, and can be used for golden testing
	/// (see [MockContext::golden]).
	pub struct MockContext {
		pub user_id: u64,
		pub data: UserContext,

		/// If true, messages will be saved in a vec.
		pub save_messages: bool,

		messages: Vec<ReplyEssence>,
	}

	impl MockContext {
		pub fn new(data: UserContext) -> Self {
			Self {
				data,
				user_id: 666,
				save_messages: true,
				messages: vec![],
			}
		}

		// {{{ golden
		/// This function implements the logic for "golden testing". We essentially
		/// make sure a command's output doesn't change, by writing it to disk,
		/// and comparing new outputs to the "golden" copy.
		///
		/// 1. This will attempt to write the data to disk (at the given path)
		/// 2. If the data already exists on disk, the two copies will be
		///    compared. A panic will occur on disagreements.
		/// 3. `SHIMMERING_TEST_REGEN=1` can be passed to overwrite disagreements.
		pub fn golden(&self, path: &PathBuf) -> Result<(), Error> {
			if env::var("SHIMMERING_TEST_REGEN").unwrap_or_default() == "1" {
				fs::remove_dir_all(path)?;
			}

			fs::create_dir_all(path)?;

			for (i, message) in self.messages.iter().enumerate() {
				let file = path.join(format!("{i}.toml"));
				Self::golden_impl(&file, message)?;
			}

			Ok(())
		}

		/// Runs the golden testing logic for a single file.
		/// See [Self::golden] for more details.
		fn golden_impl(path: &Path, message: &impl Serialize) -> Result<(), Error> {
			if path.exists() {
				assert_eq!(toml::to_string_pretty(message)?, fs::read_to_string(path)?);
			} else {
				fs::write(path, toml::to_string_pretty(message)?)?;
			}

			Ok(())
		}
		// }}}
	}

	impl MessageContext for MockContext {
		fn author_id(&self) -> u64 {
			self.user_id
		}

		async fn fetch_user(
			&self,
			discord_id: &str,
		) -> Result<poise::serenity_prelude::User, Error> {
			let mut user = poise::serenity_prelude::User::default();
			user.id = poise::serenity_prelude::UserId::from_str(discord_id)?;
			user.name = "testinguser".to_string();
			Ok(user)
		}

		fn data(&self) -> &UserContext {
			&self.data
		}

		async fn reply(&mut self, text: &str) -> Result<(), Error> {
			self.send(CreateReply::default().content(text).reply(true))
				.await
		}

		async fn send(&mut self, message: CreateReply) -> Result<(), Error> {
			if self.save_messages {
				self.messages.push(ReplyEssence::from_reply(message));
			}

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
			let res = tokio::fs::read(attachment)
				.await
				.with_context(|| format!("Could not download attachment {attachment:?}"))?;

			Ok(res)
		}
		// }}}
	}
	// }}}
}
// }}}
// {{{ Helpers
#[inline]
#[allow(dead_code)] // Currently only used for testing
pub fn play_song_title<'a>(ctx: &'a impl MessageContext, play: &'a Play) -> Result<&'a str, Error> {
	Ok(&ctx.data().song_cache.lookup_chart(play.chart_id)?.0.title)
}

pub trait CreateReplyExtra {
	fn attachments(self, attachments: impl IntoIterator<Item = CreateAttachment>) -> Self;
	fn embeds(self, embeds: impl IntoIterator<Item = CreateEmbed>) -> Self;
}

impl CreateReplyExtra for CreateReply {
	fn attachments(mut self, attachments: impl IntoIterator<Item = CreateAttachment>) -> Self {
		for attachment in attachments.into_iter() {
			self = self.attachment(attachment);
		}

		self
	}

	fn embeds(mut self, embeds: impl IntoIterator<Item = CreateEmbed>) -> Self {
		for embed in embeds.into_iter() {
			self = self.embed(embed);
		}

		self
	}
}
// }}}
