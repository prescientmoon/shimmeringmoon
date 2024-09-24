// {{{ Imports
use crate::context::AppContext;
use crate::error::AppError;
use anyhow::anyhow;
use axum::{extract::State, http::StatusCode, Json};
use chrono::{TimeDelta, Utc};
use shimmeringmoon::arcaea::play::{Play, PlayWithDetails};
// }}}

pub async fn get_recent_play(
	State(state): State<AppContext>,
) -> Result<Json<PlayWithDetails>, AppError> {
	let after = Utc::now()
		.checked_sub_signed(TimeDelta::minutes(30))
		.unwrap()
		.naive_utc();

	let (play, song, chart) = state
		.ctx
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
        AND p.created_at>=?
        ORDER BY p.created_at DESC
        LIMIT 1
    ",
		)?
		.query_and_then((2, after), |row| -> Result<_, AppError> {
			let (song, chart) = state.ctx.song_cache.lookup_chart(row.get("chart_id")?)?;
			let play = Play::from_sql(chart, row)?;
			Ok((play, song, chart))
		})?
		.next()
		.ok_or_else(|| AppError::new(anyhow!("No recent plays found"), StatusCode::NOT_FOUND))??;

	// Perhaps I need to make a Serialize-only version of this type which takes refs?
	Ok(axum::response::Json(PlayWithDetails {
		play,
		song: song.clone(),
		chart: chart.clone(),
	}))
}
