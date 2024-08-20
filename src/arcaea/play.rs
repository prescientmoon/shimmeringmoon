use std::array;

use chrono::NaiveDateTime;
use chrono::Utc;
use num::traits::Euclid;
use num::CheckedDiv;
use num::Rational32;
use num::Zero;
use poise::serenity_prelude::{
	Attachment, AttachmentId, CreateAttachment, CreateEmbed, CreateEmbedAuthor, Timestamp,
};
use sqlx::query_as;
use sqlx::{query, SqlitePool};

use crate::arcaea::chart::{Chart, Song};
use crate::context::{Error, UserContext};
use crate::user::User;

use super::chart::SongCache;
use super::rating::{rating_as_fixed, rating_as_float};
use super::score::{Score, ScoringSystem};

// {{{ Create play
#[derive(Debug, Clone)]
pub struct CreatePlay {
	discord_attachment_id: Option<AttachmentId>,

	// Scoring details
	score: Score,
	max_recall: Option<u32>,
	far_notes: Option<u32>,
}

impl CreatePlay {
	#[inline]
	pub fn new(score: Score) -> Self {
		Self {
			discord_attachment_id: None,
			score,
			max_recall: None,
			far_notes: None,
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
	pub async fn save(self, ctx: &UserContext, user: &User, chart: &Chart) -> Result<Play, Error> {
		let attachment_id = self.discord_attachment_id.map(|i| i.get() as i64);

		// {{{ Save current data to play
		let play = sqlx::query!(
			"
        INSERT INTO plays(
            user_id,chart_id,discord_attachment_id,
            max_recall,far_notes
        )
        VALUES(?,?,?,?,?)
        RETURNING id, created_at
      ",
			user.id,
			chart.id,
			attachment_id,
			self.max_recall,
			self.far_notes
		)
		.fetch_one(&ctx.db)
		.await?;
		// }}}
		// {{{ Update creation ptt data
		let scores = ScoreCollection::from_standard_score(self.score, chart);
		for system in ScoringSystem::SCORING_SYSTEMS {
			let i = system.to_index();
			let plays = get_best_plays(&ctx.db, &ctx.song_cache, user.id, system, 30, 30, None)
				.await?
				.ok();

			let creation_ptt: Option<_> = try { rating_as_fixed(compute_b30_ptt(system, &plays?)) };

			query!(
				"
          INSERT INTO scores(play_id, score, creation_ptt, scoring_system)
          VALUES (?,?,?,?)
        ",
				play.id,
				scores.0[i].0,
				creation_ptt,
				ScoringSystem::SCORING_SYSTEM_DB_STRINGS[i]
			)
			.execute(&ctx.db)
			.await?;
		}

		// }}}

		Ok(Play {
			id: play.id as u32,
			created_at: play.created_at,
			chart_id: chart.id,
			user_id: user.id,
			scores,
			max_recall: self.max_recall,
			far_notes: self.far_notes,
		})
	}
	// }}}
}
// }}}
// {{{ DbPlay
/// Construct a `Play` from a sqlite return record.
#[macro_export]
macro_rules! play_from_db_record {
	($chart:expr, $record:expr) => {{
		use crate::arcaea::play::{Play, ScoreCollection};
		use crate::arcaea::score::Score;
		Play {
			id: $record.id as u32,
			chart_id: $record.chart_id as u32,
			user_id: $record.user_id as u32,
			scores: ScoreCollection::from_standard_score(Score($record.score as u32), $chart),
			max_recall: $record.max_recall.map(|r| r as u32),
			far_notes: $record.far_notes.map(|r| r as u32),
			created_at: $record.created_at,
		}
	}};
}

/// Typed version of the input to the macro above.
/// Useful when using the non-macro version of the sqlx functions.
#[derive(Debug, sqlx::FromRow)]
pub struct DbPlay {
	pub id: i64,
	pub chart_id: i64,
	pub user_id: i64,
	pub created_at: chrono::NaiveDateTime,

	// Score details
	pub max_recall: Option<i64>,
	pub far_notes: Option<i64>,
	pub score: i64,
}

// }}}
// {{{ Score data
#[derive(Debug, Clone, Copy)]
pub struct ScoreCollection([Score; ScoringSystem::SCORING_SYSTEMS.len()]);

impl ScoreCollection {
	pub fn from_standard_score(score: Score, chart: &Chart) -> Self {
		ScoreCollection(array::from_fn(|i| {
			score.convert_to(ScoringSystem::SCORING_SYSTEMS[i], chart)
		}))
	}
}
// }}}
// {{{ Play
#[derive(Debug, Clone)]
pub struct Play {
	pub id: u32,
	#[allow(unused)]
	pub chart_id: u32,
	pub user_id: u32,
	pub created_at: chrono::NaiveDateTime,

	// Score details
	pub max_recall: Option<u32>,
	pub far_notes: Option<u32>,
	pub scores: ScoreCollection,
}

impl Play {
	// {{{ Query the underlying score
	#[inline]
	pub fn score(&self, system: ScoringSystem) -> Score {
		self.scores.0[system.to_index()]
	}

	#[inline]
	pub fn play_rating(&self, system: ScoringSystem, chart_constant: u32) -> Rational32 {
		self.score(system).play_rating(chart_constant)
	}

	#[inline]
	pub fn play_rating_f32(&self, system: ScoringSystem, chart_constant: u32) -> f32 {
		rating_as_float(self.score(system).play_rating(chart_constant))
	}
	// }}}
	// {{{ Play => distribution
	pub fn distribution(&self, note_count: u32) -> Option<(u32, u32, u32, u32)> {
		if let Some(fars) = self.far_notes {
			let (_, shinies, units) = self.score(ScoringSystem::Standard).analyse(note_count);
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
	pub fn status(&self, scoring_system: ScoringSystem, chart: &Chart) -> Option<String> {
		let score = self.score(scoring_system).0;
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
	pub fn short_status(&self, scoring_system: ScoringSystem, chart: &Chart) -> Option<char> {
		let score = self.score(scoring_system).0;
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
		let prev_play = query!(
			"
        SELECT 
          p.id, p.chart_id, p.user_id, p.created_at,
          p.max_recall, p.far_notes, s.score
        FROM plays p
        JOIN scores s ON s.play_id = p.id
        WHERE s.scoring_system='standard'
        AND p.user_id=?
        AND p.chart_id=?
        AND p.created_at<?
        ORDER BY s.score DESC
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
		.map(|p| play_from_db_record!(chart, p));

		let prev_score = prev_play.as_ref().map(|p| p.score(ScoringSystem::Standard));
		let prev_zeta_score = prev_play.as_ref().map(|p| p.score(ScoringSystem::EX));
		// }}}

		let attachement_name = format!(
			"{:?}-{:?}-{:?}.png",
			song.id,
			self.score(ScoringSystem::Standard).0,
			index
		);
		let icon_attachement = match chart.cached_jacket.as_ref() {
			Some(jacket) => Some(CreateAttachment::bytes(jacket.raw, &attachement_name)),
			None => None,
		};

		let mut embed = CreateEmbed::default()
			.title(format!(
				"{} [{:?} {}]",
				&song.title, chart.difficulty, chart.level
			))
			.field(
				"Score",
				self.score(ScoringSystem::Standard)
					.display_with_diff(prev_score)?,
				true,
			)
			.field(
				"Rating",
				self.score(ScoringSystem::Standard)
					.display_play_rating(prev_score, chart)?,
				true,
			)
			.field(
				"Grade",
				format!("{}", self.score(ScoringSystem::Standard).grade()),
				true,
			)
			.field(
				"ξ-Score",
				self.score(ScoringSystem::EX)
					.display_with_diff(prev_zeta_score)?,
				true,
			)
			// {{{ ξ-Rating
			.field(
				"ξ-Rating",
				self.score(ScoringSystem::EX)
					.display_play_rating(prev_zeta_score, chart)?,
				true,
			)
			// }}}
			.field(
				"ξ-Grade",
				format!("{}", self.score(ScoringSystem::EX).grade()),
				true,
			)
			.field(
				"Status",
				self.status(ScoringSystem::Standard, chart)
					.unwrap_or("-".to_string()),
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
	user_id: u32,
	scoring_system: ScoringSystem,
	min_amount: usize,
	max_amount: usize,
	before: Option<NaiveDateTime>,
) -> Result<Result<PlayCollection<'a>, String>, Error> {
	// {{{ DB data fetching
	let plays: Vec<DbPlay> = query_as(
		"
      SELECT 
        p.id, p.chart_id, p.user_id, p.created_at,
        p.max_recall, p.far_notes, s.score,
        MAX(s.score) as _cscore 
        -- ^ This is only here to make sqlite pick the correct row for the bare columns
      FROM plays p
      JOIN scores s ON s.play_id = p.id
      JOIN scores cs ON cs.play_id = p.id
      WHERE s.scoring_system='standard'
      AND cs.scoring_system=?
      AND p.user_id=?
      AND p.created_at<=?
      GROUP BY p.chart_id
    ",
	)
	.bind(ScoringSystem::SCORING_SYSTEM_DB_STRINGS[scoring_system.to_index()])
	.bind(user_id)
	.bind(before.unwrap_or_else(|| Utc::now().naive_utc()))
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
			let (song, chart) = song_cache.lookup_chart(play.chart_id as u32)?;
			let play = play_from_db_record!(chart, play);
			Ok((play, song, chart))
		})
		.collect::<Result<Vec<_>, Error>>()?;

	plays.sort_by_key(|(play, _, chart)| -play.play_rating(scoring_system, chart.chart_constant));
	plays.truncate(max_amount);
	// }}}

	Ok(Ok(plays))
}

#[inline]
pub fn compute_b30_ptt(scoring_system: ScoringSystem, plays: &PlayCollection<'_>) -> Rational32 {
	plays
		.iter()
		.map(|(play, _, chart)| play.play_rating(scoring_system, chart.chart_constant))
		.sum::<Rational32>()
		.checked_div(&Rational32::from_integer(plays.len() as i32))
		.unwrap_or(Rational32::zero())
}
// }}}
// {{{ Maintenance functions
pub async fn generate_missing_scores(ctx: &UserContext) -> Result<(), Error> {
	let plays = query!(
		"
      SELECT 
        p.id, p.chart_id, p.user_id, p.created_at,
        p.max_recall, p.far_notes, s.score
      FROM plays p
      JOIN scores s ON s.play_id = p.id
      WHERE s.scoring_system='standard'
      ORDER BY p.created_at ASC
    "
	)
	// Can't use the stream based version because of db locking...
	.fetch_all(&ctx.db)
	.await?;

	let mut i = 0;

	for play in plays {
		let (_, chart) = ctx.song_cache.lookup_chart(play.chart_id as u32)?;
		let play = play_from_db_record!(chart, play);

		for system in ScoringSystem::SCORING_SYSTEMS {
			let i = system.to_index();
			let plays = get_best_plays(
				&ctx.db,
				&ctx.song_cache,
				play.user_id,
				system,
				30,
				30,
				Some(play.created_at),
			)
			.await?
			.ok();

			let creation_ptt: Option<_> = try { rating_as_fixed(compute_b30_ptt(system, &plays?)) };
			let raw_score = play.scores.0[i].0;

			query!(
				"
	        INSERT INTO scores(play_id, score, creation_ptt, scoring_system)
	        VALUES ($1, $2, $3, $4)
          ON CONFLICT(play_id, scoring_system)
            DO UPDATE SET
              score=$2, creation_ptt=$3
            WHERE play_id = $1
            AND scoring_system = $4

	      ",
				play.id,
				raw_score,
				creation_ptt,
				ScoringSystem::SCORING_SYSTEM_DB_STRINGS[i],
			)
			.execute(&ctx.db)
			.await?;
		}

		i += 1;
		println!("Processed {i} plays");
	}
	Ok(())
}
// }}}
