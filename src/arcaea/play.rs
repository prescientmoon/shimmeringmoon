use std::str::FromStr;

use num::traits::Euclid;
use poise::serenity_prelude::{
	Attachment, AttachmentId, CreateAttachment, CreateEmbed, CreateEmbedAuthor, Timestamp,
};
use sqlx::{query_as, SqlitePool};

use crate::arcaea::chart::{Chart, Song};
use crate::context::{Error, UserContext};
use crate::user::User;

use super::chart::SongCache;
use super::score::{Score, ScoringSystem};

// {{{ Create play
#[derive(Debug, Clone)]
pub struct CreatePlay {
	chart_id: u32,
	user_id: u32,
	discord_attachment_id: Option<AttachmentId>,

	// Actual score data
	score: Score,
	zeta_score: Score,

	// Optional score details
	max_recall: Option<u32>,
	far_notes: Option<u32>,

	// Creation data
	creation_ptt: Option<u32>,
	creation_zeta_ptt: Option<u32>,
}

impl CreatePlay {
	#[inline]
	pub fn new(score: Score, chart: &Chart, user: &User) -> Self {
		Self {
			chart_id: chart.id,
			user_id: user.id,
			discord_attachment_id: None,
			score,
			zeta_score: score.to_zeta(chart.note_count as u32),
			max_recall: None,
			far_notes: None,
			// TODO: populate these
			creation_ptt: None,
			creation_zeta_ptt: None,
		}
	}

	#[inline]
	pub fn with_attachment(mut self, attachment: &Attachment) -> Self {
		self.discord_attachment_id = Some(attachment.id);
		self
	}

	#[inline]
	pub fn with_fars(mut self, far_count: Option<u32>) -> Self {
		self.far_notes = far_count;
		self
	}

	#[inline]
	pub fn with_max_recall(mut self, max_recall: Option<u32>) -> Self {
		self.max_recall = max_recall;
		self
	}

	// {{{ Save
	pub async fn save(self, ctx: &UserContext) -> Result<Play, Error> {
		let attachment_id = self.discord_attachment_id.map(|i| i.get() as i64);
		let play = sqlx::query!(
			"
        INSERT INTO plays(
        user_id,chart_id,discord_attachment_id,
        score,zeta_score,max_recall,far_notes
        )
        VALUES(?,?,?,?,?,?,?)
        RETURNING id, created_at
      ",
			self.user_id,
			self.chart_id,
			attachment_id,
			self.score.0,
			self.zeta_score.0,
			self.max_recall,
			self.far_notes
		)
		.fetch_one(&ctx.db)
		.await?;

		Ok(Play {
			id: play.id as u32,
			created_at: play.created_at,
			chart_id: self.chart_id,
			user_id: self.user_id,
			discord_attachment_id: self.discord_attachment_id,
			score: self.score,
			zeta_score: self.zeta_score,
			max_recall: self.max_recall,
			far_notes: self.far_notes,
			creation_ptt: self.creation_ptt,
			creation_zeta_ptt: self.creation_zeta_ptt,
		})
	}
	// }}}
}
// }}}
// {{{ DbPlay
/// Version of `Play` matching the format sqlx expects
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DbPlay {
	pub id: i64,
	pub chart_id: i64,
	pub user_id: i64,
	pub discord_attachment_id: Option<String>,
	pub score: i64,
	pub zeta_score: i64,
	pub max_recall: Option<i64>,
	pub far_notes: Option<i64>,
	pub created_at: chrono::NaiveDateTime,
	pub creation_ptt: Option<i64>,
	pub creation_zeta_ptt: Option<i64>,
}

impl DbPlay {
	#[inline]
	pub fn into_play(self) -> Play {
		Play {
			id: self.id as u32,
			chart_id: self.chart_id as u32,
			user_id: self.user_id as u32,
			score: Score(self.score as u32),
			zeta_score: Score(self.zeta_score as u32),
			max_recall: self.max_recall.map(|r| r as u32),
			far_notes: self.far_notes.map(|r| r as u32),
			created_at: self.created_at,
			discord_attachment_id: self
				.discord_attachment_id
				.and_then(|s| AttachmentId::from_str(&s).ok()),
			creation_ptt: self.creation_ptt.map(|r| r as u32),
			creation_zeta_ptt: self.creation_zeta_ptt.map(|r| r as u32),
		}
	}
}
// }}}
// {{{ Play
#[derive(Debug, Clone)]
pub struct Play {
	pub id: u32,
	pub chart_id: u32,
	pub user_id: u32,

	#[allow(unused)]
	pub discord_attachment_id: Option<AttachmentId>,

	// Actual score data
	pub score: Score,
	pub zeta_score: Score,

	// Optional score details
	pub max_recall: Option<u32>,
	pub far_notes: Option<u32>,

	// Creation data
	pub created_at: chrono::NaiveDateTime,

	#[allow(unused)]
	pub creation_ptt: Option<u32>,

	#[allow(unused)]
	pub creation_zeta_ptt: Option<u32>,
}

impl Play {
	// {{{ Query the underlying score
	#[inline]
	pub fn score(&self, system: ScoringSystem) -> Score {
		match system {
			ScoringSystem::Standard => self.score,
			ScoringSystem::EX => self.zeta_score,
		}
	}

	#[inline]
	pub fn play_rating(&self, system: ScoringSystem, chart_constant: u32) -> i32 {
		self.score(system).play_rating(chart_constant)
	}
	// }}}
	// {{{ Play => distribution
	pub fn distribution(&self, note_count: u32) -> Option<(u32, u32, u32, u32)> {
		if let Some(fars) = self.far_notes {
			let (_, shinies, units) = self.score.analyse(note_count);
			let (pures, rem) = units.checked_sub(fars)?.div_rem_euclid(&2);
			if rem == 1 {
				println!("The impossible happened: got an invalid amount of far notes!");
				return None;
			}

			let lost = note_count.checked_sub(fars + pures)?;
			let non_max_pures = pures.checked_sub(shinies)?;
			Some((shinies, non_max_pures, fars, lost))
		} else {
			None
		}
	}
	// }}}
	// {{{ Play => status
	#[inline]
	pub fn status(&self, chart: &Chart) -> Option<String> {
		let score = self.score.0;
		if score >= 10_000_000 {
			if score > chart.note_count + 10_000_000 {
				return None;
			}

			let non_max_pures = (chart.note_count + 10_000_000).checked_sub(score)?;
			if non_max_pures == 0 {
				Some("MPM".to_string())
			} else {
				Some(format!("PM (-{})", non_max_pures))
			}
		} else if let Some(distribution) = self.distribution(chart.note_count) {
			// if no lost notes...
			if distribution.3 == 0 {
				Some(format!("FR (-{}/-{})", distribution.1, distribution.2))
			} else {
				Some(format!(
					"C (-{}/-{}/-{})",
					distribution.1, distribution.2, distribution.3
				))
			}
		} else {
			None
		}
	}

	#[inline]
	pub fn short_status(&self, chart: &Chart) -> Option<char> {
		let score = self.score.0;
		if score >= 10_000_000 {
			let non_max_pures = (chart.note_count + 10_000_000).checked_sub(score)?;
			if non_max_pures == 0 {
				Some('M')
			} else {
				Some('P')
			}
		} else if let Some(distribution) = self.distribution(chart.note_count)
			&& distribution.3 == 0
		{
			Some('F')
		} else {
			Some('C')
		}
	}
	// }}}
	// {{{ Play to embed
	/// Creates a discord embed for this play.
	///
	/// The `index` variable is only used to create distinct filenames.
	pub async fn to_embed(
		&self,
		db: &SqlitePool,
		user: &User,
		song: &Song,
		chart: &Chart,
		index: usize,
		author: Option<&poise::serenity_prelude::User>,
	) -> Result<(CreateEmbed, Option<CreateAttachment>), Error> {
		// {{{ Get previously best score
		let prev_play = query_as!(
			DbPlay,
			"
        SELECT * FROM plays
        WHERE user_id=?
        AND chart_id=?
        AND created_at<?
        ORDER BY score DESC
        LIMIT 1
    ",
			user.id,
			chart.id,
			self.created_at
		)
		.fetch_optional(db)
		.await
		.map_err(|_| {
			format!(
				"Could not find any scores for {} [{:?}]",
				song.title, chart.difficulty
			)
		})?
		.map(|p| p.into_play());

		let prev_score = prev_play.as_ref().map(|p| p.score);
		let prev_zeta_score = prev_play.as_ref().map(|p| p.zeta_score);
		// }}}

		let attachement_name = format!("{:?}-{:?}-{:?}.png", song.id, self.score.0, index);
		let icon_attachement = match chart.cached_jacket.as_ref() {
			Some(jacket) => Some(CreateAttachment::bytes(jacket.raw, &attachement_name)),
			None => None,
		};

		let mut embed = CreateEmbed::default()
			.title(format!(
				"{} [{:?} {}]",
				&song.title, chart.difficulty, chart.level
			))
			.field("Score", self.score.display_with_diff(prev_score)?, true)
			.field(
				"Rating",
				self.score.display_play_rating(prev_score, chart)?,
				true,
			)
			.field("Grade", format!("{}", self.score.grade()), true)
			.field(
				"ξ-Score",
				self.zeta_score.display_with_diff(prev_zeta_score)?,
				true,
			)
			// {{{ ξ-Rating
			.field(
				"ξ-Rating",
				self.zeta_score
					.display_play_rating(prev_zeta_score, chart)?,
				true,
			)
			// }}}
			.field("ξ-Grade", format!("{}", self.zeta_score.grade()), true)
			.field(
				"Status",
				self.status(chart).unwrap_or("-".to_string()),
				true,
			)
			.field(
				"Max recall",
				if let Some(max_recall) = self.max_recall {
					format!("{}", max_recall)
				} else {
					format!("-")
				},
				true,
			)
			.field("ID", format!("{}", self.id), true);

		if icon_attachement.is_some() {
			embed = embed.thumbnail(format!("attachment://{}", &attachement_name));
		}

		if let Some(user) = author {
			let mut embed_author = CreateEmbedAuthor::new(&user.name);
			if let Some(url) = user.avatar_url() {
				embed_author = embed_author.icon_url(url);
			}

			embed = embed
				.timestamp(Timestamp::from_millis(
					self.created_at.and_utc().timestamp_millis(),
				)?)
				.author(embed_author);
		}

		Ok((embed, icon_attachement))
	}
	// }}}
}
// }}}
// {{{ General functions
pub type PlayCollection<'a> = Vec<(Play, &'a Song, &'a Chart)>;

pub async fn get_best_plays<'a>(
	db: &SqlitePool,
	song_cache: &'a SongCache,
	user: &User,
	scoring_system: ScoringSystem,
	min_amount: usize,
	max_amount: usize,
) -> Result<Result<PlayCollection<'a>, String>, Error> {
	// {{{ DB data fetching
	let plays: Vec<DbPlay> = query_as(
		"
        SELECT id, chart_id, user_id,
        created_at, MAX(score) as score, zeta_score,
        creation_ptt, creation_zeta_ptt, far_notes, max_recall, discord_attachment_id
        FROM plays p
        WHERE user_id = ?
        GROUP BY chart_id
        ORDER BY score DESC
    ",
	)
	.bind(user.id)
	.fetch_all(db)
	.await?;
	// }}}

	if plays.len() < min_amount {
		return Ok(Err(format!(
			"Not enough plays found ({} out of a minimum of {min_amount})",
			plays.len()
		)));
	}

	// {{{ B30 computation
	// NOTE: we reallocate here, although we do not have much of a choice,
	// unless we want to be lazy about things
	let mut plays: Vec<(Play, &Song, &Chart)> = plays
		.into_iter()
		.map(|play| {
			let play = play.into_play();
			let (song, chart) = song_cache.lookup_chart(play.chart_id)?;
			Ok((play, song, chart))
		})
		.collect::<Result<Vec<_>, Error>>()?;

	plays.sort_by_key(|(play, _, chart)| -play.play_rating(scoring_system, chart.chart_constant));
	plays.truncate(max_amount);
	// }}}

	Ok(Ok(plays))
}

#[inline]
pub fn compute_b30_ptt(scoring_system: ScoringSystem, plays: &PlayCollection<'_>) -> i32 {
	plays
		.iter()
		.map(|(play, _, chart)| play.play_rating(scoring_system, chart.chart_constant))
		.sum::<i32>()
		.checked_div(plays.len() as i32)
		.unwrap_or(0)
}
// }}}
