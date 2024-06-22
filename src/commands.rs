use std::fmt::Display;

use crate::context::{Context, Error};
use crate::score::{CreatePlay, ImageCropper};
use crate::user::User;
use image::imageops::FilterType;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed, CreateMessage};
use poise::{serenity_prelude as serenity, CreateReply};

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

// {{{ Send error embed with image
async fn error_with_image(
	ctx: Context<'_>,
	bytes: &[u8],
	filename: &str,
	message: &str,
	err: impl Display,
) -> Result<(), Error> {
	let error_attachement = CreateAttachment::bytes(bytes, filename);
	let msg = CreateMessage::default().embed(
		CreateEmbed::default()
			.title(message)
			.attachment(filename)
			.description(format!("{}", err)),
	);

	ctx.channel_id()
		.send_files(ctx.http(), [error_attachement], msg)
		.await?;

	Ok(())
}
// }}}

/// Identify scores from attached images.
#[poise::command(prefix_command, slash_command)]
pub async fn magic(
	ctx: Context<'_>,
	#[description = "Images containing scores"] files: Vec<serenity::Attachment>,
) -> Result<(), Error> {
	let user = match User::from_context(&ctx).await {
		Ok(user) => user,
		Err(_) => {
			ctx.say("You are not an user in my database, sorry!")
				.await?;
			return Ok(());
		}
	};

	if files.len() == 0 {
		ctx.reply("No images found attached to message").await?;
	} else {
		let mut embeds: Vec<CreateEmbed> = vec![];
		let mut attachements: Vec<CreateAttachment> = vec![];
		let handle = ctx
			.reply(format!("Processed 0/{} scores", files.len()))
			.await?;

		for (i, file) in files.iter().enumerate() {
			if let Some(_) = file.dimensions() {
				// Download image and guess it's format
				let bytes = file.download().await?;
				let format = image::guess_format(&bytes)?;

				// Image pre-processing
				let image = image::load_from_memory_with_format(&bytes, format)?.resize(
					1024,
					1024,
					FilterType::Nearest,
				);

				// // {{{ Table experiment
				// let table_format = FormatBuilder::new()
				// 	.separators(
				// 		&[LinePosition::Title],
				// 		LineSeparator::new('─', '┬', '┌', '┐'),
				// 	)
				// 	.padding(1, 1)
				// 	.build();
				// let mut table = Table::new();
				// table.set_format(table_format);
				// table.set_titles(row!["Chart", "Level", "Score", "Rating"]);
				// table.add_row(row!["Quon", "BYD 10", "10000807", "12.3 (-132)"]);
				// table.add_row(row!["Monochrome princess", "FTR 9+", " 9380807", "10.2"]);
				// table.add_row(row!["Grievous lady", "FTR 11", " 9286787", "11.2"]);
				// table.add_row(row!["Fracture ray", "FTR 11", " 8990891", "11.0"]);
				// table.add_row(row!["Shades of Light", "FTR 9+", "10000976", " 9.3 (-13)"]);
				// ctx.say(format!("```\n{}\n```", table.to_string())).await?;
				// // }}}
				// // {{{ Embed experiment
				// let icon_attachement = CreateAttachment::file(
				// 	&tokio::fs::File::open("./data/jackets/grievous.png").await?,
				// 	"grievous.png",
				// )
				// .await?;
				// let msg = CreateMessage::default().embed(
				// 	CreateEmbed::default()
				// 		.title("Grievous lady [FTR 11]")
				// 		.thumbnail("attachment://grievous.png")
				// 		.field("Score", "998302 (+8973)", true)
				// 		.field("Rating", "12.2 (+.6)", true)
				// 		.field("Grade", "EX+", true)
				// 		.field("ζ-Score", "982108 (+347)", true)
				// 		.field("ζ-Rating", "11.5 (+.45)", true)
				// 		.field("ζ-Grade", "EX", true)
				// 		.field("Status", "FR (-243F)", true)
				// 		.field("Max recall", "308/1073", true)
				// 		.field("Breakdown", "894/342/243/23", true),
				// );
				//
				// ctx.channel_id()
				// 	.send_files(ctx.http(), [icon_attachement], msg)
				// 	.await?;
				// // }}}

				// Create cropper and run OCR
				let mut cropper = ImageCropper::default();

				let (jacket, cached_song) = match cropper.read_jacket(ctx.data(), &image) {
					// {{{ Jacket recognition error handling
					Err(err) => {
						error_with_image(
							ctx,
							&cropper.bytes,
							&file.filename,
							"Error while detecting jacket",
							err,
						)
						.await?;

						continue;
					}
					// }}}
					Ok(j) => j,
				};

				let mut image = image.grayscale().blur(1.);

				let difficulty = match cropper.read_difficulty(&image) {
					// {{{ OCR error handling
					Err(err) => {
						error_with_image(
							ctx,
							&cropper.bytes,
							&file.filename,
							"Could not read score from picture",
							&err,
						)
						.await?;

						continue;
					}
					// }}}
					Ok(d) => d,
				};

				image.invert();

				let score = match cropper.read_score(&image) {
					// {{{ OCR error handling
					Err(err) => {
						error_with_image(
							ctx,
							&cropper.bytes,
							&file.filename,
							"Could not read score from picture",
							&err,
						)
						.await?;

						continue;
					}
					// }}}
					Ok(score) => score,
				};

				let song = &cached_song.song;
				let chart = cached_song.lookup(difficulty).ok_or_else(|| {
					format!(
						"Could not find difficulty {:?} for song {}",
						difficulty, song.title
					)
				})?;

				let play = CreatePlay::new(score, chart, &user)
					.with_attachment(file)
					.save(&ctx.data())
					.await?;

				let (embed, attachement) = play.to_embed(&song, &chart, &jacket).await?;
				embeds.push(embed);
				attachements.push(attachement);
			} else {
				ctx.reply("One of the attached files is not an image!")
					.await?;
				continue;
			}

			let edited = CreateReply::default().reply(true).content(format!(
				"Processed {}/{} scores",
				i + 1,
				files.len()
			));

			handle.edit(ctx, edited).await?;
		}

		handle.delete(ctx).await?;

		let msg = CreateMessage::new().embeds(embeds);

		ctx.channel_id()
			.send_files(ctx.http(), attachements, msg)
			.await?;
	}

	Ok(())
}
