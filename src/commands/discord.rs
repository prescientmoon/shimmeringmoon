use std::num::NonZeroU64;

use poise::serenity_prelude::{futures::future::join_all, CreateAttachment, CreateMessage};

use crate::{
	context::{Error, UserContext},
	timed,
};

// {{{ Trait
pub trait MessageContext {
	/// Get the user context held by the message
	fn data(&self) -> &UserContext;
	fn author_id(&self) -> u64;

	/// Reply to the current message
	async fn reply(&mut self, text: &str) -> Result<(), Error>;

	/// Deliver a message containing references to files.
	async fn send_files(
		&mut self,
		attachments: impl IntoIterator<Item = CreateAttachment>,
		message: CreateMessage,
	) -> Result<(), Error>;

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
		attachments: &'a Vec<Self::Attachment>,
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

		pub fn write_to(&self, path: &PathBuf) -> Result<(), Error> {
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
						assert_eq!(&attachment.data, &fs::read(path)?);
					} else {
						fs::write(&path, &attachment.data)?;
					}
				}
			}

			Ok(())
		}
	}

	impl MessageContext for MockContext {
		fn author_id(&self) -> u64 {
			self.user_id
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
