use poise::serenity_prelude::{CreateAttachment, CreateEmbed, CreateMessage};
use sqlx::query;

use crate::{
	chart::Side,
	context::{Context, Error},
	score::guess_song_and_chart,
};

// {{{ Chart
/// Show a chart given it's name
#[poise::command(prefix_command, slash_command)]
pub async fn chart(
	ctx: Context<'_>,
	#[rest]
	#[description = "Name of chart to show (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let (song, chart) = guess_song_and_chart(&ctx.data(), &name)?;

	let attachement_name = "chart.png";
	let icon_attachement = match chart.cached_jacket.as_ref() {
		Some(jacket) => Some(CreateAttachment::bytes(jacket.raw, attachement_name)),
		None => None,
	};

	let play_count = query!(
		"
            SELECT COUNT(*) as count
            FROM plays
            WHERE chart_id=?
        ",
		chart.id
	)
	.fetch_one(&ctx.data().db)
	.await?;

	let mut embed = CreateEmbed::default()
		.title(format!(
			"{} [{:?} {}]",
			&song.title, chart.difficulty, chart.level
		))
		.field("Note count", format!("{}", chart.note_count), true)
		.field(
			"Chart constant",
			format!("{:.1}", chart.chart_constant as f32 / 100.0),
			true,
		)
		.field("Total plays", format!("{}", play_count.count), true)
		.field("BPM", &song.bpm, true)
		.field("Side", Side::SIDE_STRINGS[song.side.to_index()], true)
		.field("Artist", &song.title, true);

	if let Some(note_design) = &chart.note_design {
		embed = embed.field("Note design", note_design, true);
	}

	if let Some(pack) = &song.pack {
		embed = embed.field("Pack", pack, true);
	}

	if icon_attachement.is_some() {
		embed = embed.thumbnail(format!("attachment://{}", &attachement_name));
	}

	ctx.channel_id()
		.send_files(
			ctx.http(),
			icon_attachement,
			CreateMessage::new().embed(embed),
		)
		.await?;

	Ok(())
}
// }}}
