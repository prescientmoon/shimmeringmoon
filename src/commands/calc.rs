// {{{ Imports
use num::{FromPrimitive, Rational32};

use crate::arcaea::play::{compute_b30_ptt, get_best_plays};
use crate::arcaea::rating::{rating_as_float, rating_from_fixed, Rating};
use crate::context::{Error, PoiseContext, TaggedError};
use crate::recognition::fuzzy_song_name::guess_song_and_chart;
use crate::user::User;

use crate::arcaea::score::{Score, ScoringSystem};

use super::discord::MessageContext;
// }}}

// {{{ Top command
/// Compute various things
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("expected", "rating"),
	subcommand_required
)]
pub async fn calc(_ctx: PoiseContext<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Expected
// {{{ Implementation
async fn expected_impl(
	ctx: &mut impl MessageContext,
	ptt: Option<Rational32>,
	name: &str,
) -> Result<Score, TaggedError> {
	let (song, chart) = guess_song_and_chart(ctx.data(), name)?;

	let ptt = if let Some(ptt) = ptt {
		ptt
	} else {
		let user = User::from_context(ctx)?;
		compute_b30_ptt(
			ScoringSystem::Standard,
			&get_best_plays(ctx.data(), user.id, ScoringSystem::Standard, 30, 30, None)?,
		)
	};

	let cc = rating_from_fixed(chart.chart_constant as i32);

	let score = if ptt >= cc + 2 {
		Rational32::from_integer(chart.note_count as i32 + 10_000_000)
	} else if ptt >= cc + 1 {
		Rational32::from_integer(9_800_000)
			+ (ptt - cc - 1).reduced() * Rational32::from_integer(200_000)
	} else {
		Rational32::from_integer(9_500_000)
			+ (ptt - cc).reduced() * Rational32::from_integer(300_000)
	};
	let score = Score(score.to_integer().max(0) as u32);

	ctx.reply(&format!(
		"The expected score for a player of potential {:.2} on {} [{}] is {}",
		rating_as_float(ptt),
		song,
		chart.difficulty,
		score
	))
	.await?;

	Ok(score)
}
// }}}
// {{{ Tests
#[cfg(test)]
mod expected_tests {
	use crate::{
		commands::discord::mock::MockContext, context::testing::get_mock_context, golden_test,
	};

	use super::*;

	#[tokio::test]
	async fn consistent_with_rating() -> Result<(), Error> {
		let (mut ctx, _guard) = get_mock_context().await?;
		ctx.save_messages = false; // We don't want to waste time writing to a vec

		for i in 0..1_000 {
			let score = Score(i * 10_000);
			let rating = score.play_rating(1140);
			let res = expected_impl(&mut ctx, Some(rating), "Pentiment [BYD]")
				.await
				.map_err(|e| e.error)?;
			assert_eq!(
				score, res,
				"Wrong expected score for starting score {score} and rating {rating}"
			);
		}

		Ok(())
	}

	golden_test!(basic_usage, "commands/calc/expected/basic_usage");
	async fn basic_usage(ctx: &mut MockContext) -> Result<(), TaggedError> {
		expected_impl(
			ctx,
			Some(Rational32::from_f32(12.27).unwrap()),
			"Vicious anti heorism",
		)
		.await?;

		Ok(())
	}
}
// }}}
// {{{ Discord wrapper
/// Computes the expected score for a player of some potential on a given chart.
#[poise::command(prefix_command, slash_command, user_cooldown = 1)]
async fn expected(
	mut ctx: PoiseContext<'_>,
	#[description = "The potential to compute the expected score for"] ptt: Option<f32>,
	#[rest]
	#[description = "Name of chart (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let res = expected_impl(&mut ctx, ptt.and_then(Rational32::from_f32), &name).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
// {{{ Rating
// {{{ Implementation
async fn rating_impl(
	ctx: &mut impl MessageContext,
	score: Score,
	name: &str,
) -> Result<Rating, TaggedError> {
	let (song, chart) = guess_song_and_chart(ctx.data(), name)?;

	let rating = score.play_rating(chart.chart_constant);

	ctx.reply(&format!(
		"The score {} on {} [{}] yields a rating of {:.2}",
		score,
		song,
		chart.difficulty,
		rating_as_float(rating),
	))
	.await?;

	Ok(rating)
}
// }}}
// {{{ Tests
#[cfg(test)]
mod rating_tests {
	use crate::{commands::discord::mock::MockContext, golden_test};

	use super::*;

	golden_test!(basic_usage, "commands/calc/rating/basic_usage");
	async fn basic_usage(ctx: &mut MockContext) -> Result<(), TaggedError> {
		rating_impl(ctx, Score(9_349_070), "Arcana Eden [PRS]").await?;

		Ok(())
	}
}
// }}}
// {{{ Discord wrapper
/// Computes the rating (potential) of a play on a given chart.
#[poise::command(prefix_command, slash_command, user_cooldown = 1)]
async fn rating(
	mut ctx: PoiseContext<'_>,
	score: u32,
	#[rest]
	#[description = "Name of chart (difficulty at the end)"]
	name: String,
) -> Result<(), Error> {
	let res = rating_impl(&mut ctx, Score(score), &name).await;
	ctx.handle_error(res).await?;

	Ok(())
}
// }}}
// }}}
