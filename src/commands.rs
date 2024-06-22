use crate::context::{Context, Error};
use crate::score::ImageCropper;
use crate::user::User;
use image::imageops::FilterType;
use poise::serenity_prelude::{
	CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage, Timestamp,
};
use poise::{serenity_prelude as serenity, CreateReply};
use prettytable::format::{FormatBuilder, LinePosition, LineSeparator};
use prettytable::{row, Table};

/// Show this help menu
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn help(
	ctx: Context<'_>,
	#[description = "Specific command to show help about"]
	#[autocomplete = "poise::builtins::autocomplete_command"]
	command: Option<String>,
) -> Result<(), Error> {
	poise::builtins::help(
		ctx,
		command.as_deref(),
		poise::builtins::HelpConfiguration {
			extra_text_at_bottom: "For additional support, message @prescientmoon",
			..Default::default()
		},
	)
	.await?;
	Ok(())
}

/// Score management
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("magic"),
	subcommand_required
)]
pub async fn score(_ctx: Context<'_>) -> Result<(), Error> {
	Ok(())
}

/// Identify scores from attached images.
#[poise::command(prefix_command, slash_command)]
pub async fn magic(
	ctx: Context<'_>,
	#[description = "Images containing scores"] files: Vec<serenity::Attachment>,
) -> Result<(), Error> {
	println!("{:?}", User::from_context(&ctx).await);

	if files.len() == 0 {
		ctx.reply("No images found attached to message").await?;
	} else {
		let handle = ctx
			.reply(format!("Processing: 0/{} images", files.len()))
			.await?;

		for (i, file) in files.iter().enumerate() {
			if let Some(_) = file.dimensions() {
				// Download image and guess it's format
				let bytes = file.download().await?;
				let format = image::guess_format(&bytes)?;

				// Image pre-processing
				let mut image = image::load_from_memory_with_format(&bytes, format)?
					.resize(1024, 1024, FilterType::Nearest)
					.grayscale()
					.blur(1.);
				image.invert();

				// {{{ Table experiment
				let table_format = FormatBuilder::new()
					.separators(
						&[LinePosition::Title],
						LineSeparator::new('─', '┬', '┌', '┐'),
					)
					.padding(1, 1)
					.build();
				let mut table = Table::new();
				table.set_format(table_format);
				table.set_titles(row!["Chart", "Level", "Score", "Rating"]);
				table.add_row(row!["Quon", "BYD 10", "10000807", "12.3 (-132)"]);
				table.add_row(row!["Monochrome princess", "FTR 9+", " 9380807", "10.2"]);
				table.add_row(row!["Grievous lady", "FTR 11", " 9286787", "11.2"]);
				table.add_row(row!["Fracture ray", "FTR 11", " 8990891", "11.0"]);
				table.add_row(row!["Shades of Light", "FTR 9+", "10000976", " 9.3 (-13)"]);
				ctx.say(format!("```\n{}\n```", table.to_string())).await?;
				// }}}

				let icon_attachement = CreateAttachment::file(
					&tokio::fs::File::open("./data/jackets/grievous.png").await?,
					"grievous.png",
				)
				.await?;
				let msg = CreateMessage::default().embed(
					CreateEmbed::default()
						.title("Grievous lady [FTR 11]")
						.thumbnail("attachment://grievous.png")
						.field("Score", "998302 (+8973)", true)
						.field("Rating", "12.2 (+.6)", true)
						.field("Grade", "EX+", true)
						.field("ζ-Score", "982108 (+347)", true)
						.field("ζ-Rating", "11.5 (+.45)", true)
						.field("ζ-Grade", "EX", true)
						.field("Status", "FR (-243F)", true)
						.field("Max recall", "308/1073", true)
						.field("Breakdown", "894/342/243/23", true),
				);

				ctx.channel_id()
					.send_files(ctx.http(), [icon_attachement], msg)
					.await?;

				// Create cropper and run OCR
				let mut cropper = ImageCropper::default();
				let score_readout = match cropper.read_score(&image) {
					// {{{ OCR error handling
					Err(err) => {
						let error_attachement =
							CreateAttachment::bytes(cropper.bytes, &file.filename);
						let msg = CreateMessage::default().embed(
							CreateEmbed::default()
								.title("Could not read score from picture")
								.attachment(&file.filename)
								.description(format!("{}", err))
								.author(
									CreateEmbedAuthor::new(&ctx.author().name)
										.icon_url(ctx.author().face()),
								)
								.timestamp(Timestamp::now()),
						);
						ctx.channel_id()
							.send_files(ctx.http(), [error_attachement], msg)
							.await?;

						continue;
					}
					// }}}
					Ok(score) => score,
				};

				// Reply with attachement & readout
				let attachement = CreateAttachment::bytes(cropper.bytes, &file.filename);
				let reply = CreateReply::default()
					.attachment(attachement)
					.content(format!("Score: {:?}", score_readout))
					.reply(true);
				ctx.send(reply).await?;

				// Edit progress reply
				let progress_reply = CreateReply::default()
					.content(format!("Processing: {}/{} images", i + 1, files.len()))
					.reply(true);
				handle.edit(ctx, progress_reply).await?;
			} else {
				ctx.reply("One of the attached files is not an image!")
					.await?;
				continue;
			}
		}

		// Finish off progress reply
		let progress_reply = CreateReply::default()
			.content(format!("All images have been processed!"))
			.reply(true);
		handle.edit(ctx, progress_reply).await?;
	}

	Ok(())
}
