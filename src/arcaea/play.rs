use std::array;
use std::num::NonZeroU64;

use chrono::NaiveDateTime;
use chrono::Utc;
use num::traits::Euclid;
use num::CheckedDiv;
use num::Rational32;
use num::Zero;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed, CreateEmbedAuthor, Timestamp};
use rusqlite::Row;

use crate::arcaea::chart::{Chart, Song};
use crate::context::{Error, UserContext};
use crate::user::User;

use super::rating::{rating_as_fixed, rating_as_float};
use super::score::{Score, ScoringSystem};

// {{{ Create play
#[derive(Debug, Clone)]
pub struct CreatePlay {
	discord_attachment_id: Option<NonZeroU64>,

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
	pub fn with_attachment(mut self, attachment_id: NonZeroU64) -> Self {
		self.discord_attachment_id = Some(attachment_id);
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
	pub fn save(self, ctx: &UserContext, user: &User, chart: &Chart) -> Result<Play, Error> {
		let conn = ctx.db.get()?;
		let attachment_id = self.discord_attachment_id.map(|i| i.get() as i64);

		// {{{ Save current data to play
		let (id, created_at) = conn
			.prepare_cached(
				"
        INSERT INTO plays(
            user_id,chart_id,discord_attachment_id,
            max_recall,far_notes
        )
        VALUES(?,?,?,?,?)
        RETURNING id, created_at
      ",
			)?
			.query_row(
				(
					user.id,
					chart.id,
					attachment_id,
					self.max_recall,
					self.far_notes,
				),
				|row| Ok((row.get("id")?, row.get("created_at")?)),
			)?;
		// }}}
		// {{{ Update creation ptt data
		let scores = ScoreCollection::from_standard_score(self.score, chart);

		for system in ScoringSystem::SCORING_SYSTEMS {
			let i = system.to_index();
			let plays = get_best_plays(ctx, user.id, system, 30, 30, None)?.ok();

			let creation_ptt: Option<_> = try { rating_as_fixed(compute_b30_ptt(system, &plays?)) };

			conn.prepare_cached(
				"
          INSERT INTO scores(play_id, score, creation_ptt, scoring_system)
          VALUES (?,?,?,?)
        ",
			)?
			.execute((
				id,
				scores.0[i].0,
				creation_ptt,
				ScoringSystem::SCORING_SYSTEM_DB_STRINGS[i],
			))?;
		}

		// }}}

		Ok(Play {
			id,
			created_at,
			scores,
			chart_id: chart.id,
			user_id: user.id,
			max_recall: self.max_recall,
			far_notes: self.far_notes,
		})
	}
	// }}}
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
	// {{{ Row parsing
	#[inline]
	pub fn from_sql(chart: &Chart, row: &Row) -> Result<Self, rusqlite::Error> {
		Ok(Play {
			id: row.get("id")?,
			chart_id: row.get("chart_id")?,
			user_id: row.get("user_id")?,
			created_at: row.get("created_at")?,
			max_recall: row.get("max_recall")?,
			far_notes: row.get("far_notes")?,
			scores: ScoreCollection::from_standard_score(Score(row.get("score")?), chart),
		})
	}
	// }}}
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
	pub fn to_embed(
		&self,
		ctx: &UserContext,
		user: &User,
		song: &Song,
		chart: &Chart,
		index: usize,
		author: Option<&poise::serenity_prelude::User>,
	) -> Result<(CreateEmbed, Option<CreateAttachment>), Error> {
		// {{{ Get previously best score
		let prev_play = ctx
			.db
			.get()?
			.prepare_cached(
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
			)?
			.query_row((user.id, chart.id, self.created_at), |row| {
				Self::from_sql(chart, row)
			})
			.ok();

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

pub fn get_best_plays<'a>(
	ctx: &'a UserContext,
	user_id: u32,
	scoring_system: ScoringSystem,
	min_amount: usize,
	max_amount: usize,
	before: Option<NaiveDateTime>,
) -> Result<Result<PlayCollection<'a>, String>, Error> {
	let conn = ctx.db.get()?;
	// {{{ DB data fetching
	let mut plays = conn
		.prepare_cached(
			"
        SELECT 
          p.id, p.chart_id, p.user_id, p.created_at,
          p.max_recall, p.far_notes, s.score,
          MAX(cs.score) as _cscore 
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
		)?
		.query_and_then(
			(
				ScoringSystem::SCORING_SYSTEM_DB_STRINGS[scoring_system.to_index()],
				user_id,
				before.unwrap_or_else(|| Utc::now().naive_utc()),
			),
			|row| {
				let (song, chart) = ctx.song_cache.lookup_chart(row.get("chart_id")?)?;
				let play = Play::from_sql(chart, row)?;
				Ok((play, song, chart))
			},
		)?
		.collect::<Result<Vec<_>, Error>>()?;
	// }}}

	if plays.len() < min_amount {
		return Ok(Err(format!(
			"Not enough plays found ({} out of a minimum of {min_amount})",
			plays.len()
		)));
	}

	// {{{ B30 computation
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
	let conn = ctx.db.get()?;
	let mut query = conn.prepare_cached(
		"
      SELECT 
        p.id, p.chart_id, p.user_id, p.created_at,
        p.max_recall, p.far_notes, s.score
      FROM plays p
      JOIN scores s ON s.play_id = p.id
      WHERE s.scoring_system='standard'
      ORDER BY p.created_at ASC
    ",
	)?;

	let plays = query.query_and_then((), |row| -> Result<_, Error> {
		let (_, chart) = ctx.song_cache.lookup_chart(row.get("chart_id")?)?;
		let play = Play::from_sql(chart, row)?;
		Ok(play)
	})?;

	let mut i = 0;

	for play in plays {
		let play = play?;
		for system in ScoringSystem::SCORING_SYSTEMS {
			let i = system.to_index();
			let plays =
				get_best_plays(&ctx, play.user_id, system, 30, 30, Some(play.created_at))?.ok();

			let creation_ptt: Option<_> = try { rating_as_fixed(compute_b30_ptt(system, &plays?)) };
			let raw_score = play.scores.0[i].0;

			conn.prepare_cached(
				"
	          INSERT INTO scores(play_id, score, creation_ptt, scoring_system)
	          VALUES ($1, $2, $3, $4)
            ON CONFLICT(play_id, scoring_system)
              DO UPDATE SET
                score=$2, creation_ptt=$3
              WHERE play_id = $1
              AND scoring_system = $4
	      ",
			)?
			.execute((
				play.id,
				raw_score,
				creation_ptt,
				ScoringSystem::SCORING_SYSTEM_DB_STRINGS[i],
			))?;
		}

		i += 1;
		println!("Processed {i} plays");
	}
	Ok(())
}
// }}}
