// {{{ Imports
use std::num::NonZeroU64;
use std::str::FromStr;

use poise::serenity_prelude::futures::future::join_all;
use poise::serenity_prelude::{CreateAttachment, CreateMessage};

use crate::arcaea::play::Play;
use crate::context::{Error, UserContext};
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

	/// Deliver a message containing references to files.
	async fn send_files(
		&mut self,
		attachments: impl IntoIterator<Item = CreateAttachment>,
		message: CreateMessage,
	) -> Result<(), Error>;

	/// Deliver a message
	async fn send(&mut self, message: CreateMessage) -> Result<(), Error> {
		self.send_files([], message).await
	}

	// {{{ Input attachments
	type Attachment;

	fn is_image(attachment: &Self::Attachment) -> bool;
	fn filename(attachment: &Self::Attachment) -> &str;
	fn attachment_id(attachment: &Self::Attachment) -> NonZeroU64;

	/// Downloads a single file
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
}
// }}}
// {{{ Poise implementation
impl<'a> MessageContext for poise::Context<'a, UserContext, Error> {
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

	async fn send_files(
		&mut self,
		attachments: impl IntoIterator<Item = CreateAttachment>,
		message: CreateMessage,
	) -> Result<(), Error> {
		self.channel_id()
			.send_files(self.http(), attachments, message)
			.await?;
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
	use std::{env, fs, path::PathBuf};

	use super::*;

	/// A mock context usable for testing. Messages and attachments are
	/// accumulated inside a vec, and can be used for golden testing
	/// (see [MockContext::golden])
	pub struct MockContext {
		pub user_id: u64,
		pub data: UserContext,
		pub messages: Vec<(CreateMessage, Vec<CreateAttachment>)>,
	}

	impl MockContext {
		pub fn new(data: UserContext) -> Self {
			Self {
				data,
				user_id: 666,
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
			for (i, (message, attachments)) in self.messages.iter().enumerate() {
				let dir = path.join(format!("{i}"));
				fs::create_dir_all(&dir)?;
				let message_file = dir.join("message.toml");

				if message_file.exists() {
					assert_eq!(
						toml::to_string_pretty(message)?,
						fs::read_to_string(message_file)?
					);
				} else {
					fs::write(&message_file, toml::to_string_pretty(message)?)?;
				}

				for attachment in attachments {
					let path = dir.join(&attachment.filename);

					if path.exists() {
						if &attachment.data != &fs::read(&path)? {
							panic!("Attachment differs from {path:?}");
						}
					} else {
						fs::write(&path, &attachment.data)?;
					}
				}

				// Ensure there's no extra attachments on disk
				let file_count = fs::read_dir(dir)?.count();
				if file_count != attachments.len() + 1 {
					panic!(
						"Only {} attachments found instead of {}",
						attachments.len(),
						file_count - 1
					);
				}
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
			self.messages
				.push((CreateMessage::new().content(text), Vec::new()));
			Ok(())
		}

		async fn send_files(
			&mut self,
			attachments: impl IntoIterator<Item = CreateAttachment>,
			message: CreateMessage,
		) -> Result<(), Error> {
			self.messages
				.push((message, attachments.into_iter().collect()));
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
}
// }}}
// {{{ Helpers
#[inline]
#[allow(dead_code)] // Currently only used for testing
pub fn play_song_title<'a>(ctx: &'a impl MessageContext, play: &'a Play) -> Result<&'a str, Error> {
	Ok(&ctx.data().song_cache.lookup_chart(play.chart_id)?.0.title)
}
// }}}
