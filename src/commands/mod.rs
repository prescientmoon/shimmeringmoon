use crate::context::{Context, Error};

pub mod chart;
pub mod score;
pub mod stats;
pub mod utils;

// {{{ Help
/// Show this help menu
#[poise::command(prefix_command, slash_command, subcommands("scoring", "scoringz"))]
pub async fn help(
	ctx: Context<'_>,
	#[description = "Specific command to show help about"]
	#[autocomplete = "poise::builtins::autocomplete_command"]
	#[rest]
	command: Option<String>,
) -> Result<(), Error> {
	poise::builtins::help(
		ctx,
		command.as_deref(),
		poise::builtins::HelpConfiguration {
			extra_text_at_bottom: "For additional support, message @prescientmoon",
			show_subcommands: true,
			..Default::default()
		},
	)
	.await?;
	Ok(())
}
// }}}
// {{{ Scoring help
/// Explains the different scoring systems
#[poise::command(prefix_command, slash_command)]
async fn scoring(ctx: Context<'_>) -> Result<(), Error> {
	static CONTENT: &'static str = "
## 1. Standard scoring (`standard`):
This is the base-game Arcaea scoring system we all know and love! Points are awarded for each note, with a `2:1` pure:far ratio. The score is then scaled up such that `10_000_000` is the maximum. Last but not least, the number of max pures is added to the total.

## 2. ξ scoring (`ex`):
This is a stricter scoring method inspired by EX-scoring in sdvx. The scoring algorithm works almost the same as in standard scoring, except a `5:4:2` max-pure:pure:far ratio is used (the number of max pures is no longer added to the scaled up total). This means shinies (i.e. max pures) are worth 1.25x as much as non-max pures. 

Use this scoring method if you want to focus on shiny accuracy. ξ-scoring has the added property that ξ-PMs correspond to standard FPMs.

## 3. Single-digit-forgiveness scoring (`sdf`):
This is a slightly more lax version of ξ-scoring which overlooks up to 9 non-max pures. SDF-scoring has the added property that SDF-PMs correspond to standard SDPMs.


Most commands take an optional parameter specifying what scoring system to use. For instance, `stats b30 ex` will produce a b30 image with scores computed using SDF scoring. This makes the system extremely versatile — for instance, all the standard PM related achievements suddenly gain an extra meaning while in other modes (namely, they refer to SDPMs and FPMs in SDF or ξ scoring respectively)
    ";

	ctx.reply(CONTENT).await?;

	Ok(())
}
// }}}
// {{{ Scoring gen-z help
/// Explains the different scoring systems using gen-z slang
#[poise::command(prefix_command, slash_command)]
async fn scoringz(ctx: Context<'_>) -> Result<(), Error> {
	static CONTENT: &'static str = "
## 1. Standard scoring (`standard`):
Alright, fam, this is the OG Arcaea scoring setup that everyone vibes with! You hit notes, you get points — easy clap. The ratio is straight up `2:1` pure:far. The score then gets a glow-up, maxing out at `10 milly`. And hold up, you even get bonus points for those max pures at the end. No cap, this is the classic way to flex your skills.

## 2. ξ scoring (`ex`):
Now, this one’s for the real Gs. ξ scoring is inspired by EX-scoring, for the SDVX-pilled of y'all, so you know it’s serious business. It’s like standard, but with more drip — a `5:4:2` max-pure:pure:far ratio. That means shinies are worth fat stacks — 1.25x more than Ohio pures. No bonus points here, so it’s all about that shiny flex. 

If you’re all about shinymaxxing, this is your go-to. Oh, and ξ-PMs? They line up with standard FPMs - if you can hit those, you're truly the CEO of rhythm.

## 3. Skibidi-digit-forgiveness scoring (`sdf`):
For those who wanna chill a bit, while still on the acc grindset, we got SDF scoring. It’s like ξ scoring but with a bit of slack — up to 9 Ohio pures get a pass. SDF-PMs line up with standard SDPMs, so you’re still big-braining it. 


Real ones can skip the yap and use this already, fr. But for the sussy NPCs among y'all who wanna like, see the best 30 Ws with ξ-scoring — just hit `stats b30 ex` and you’re golden. This makes the whole system hella versatile — like, standard PMs highkey get a whole new ass meaning depending on the achievement mode you’re mewing in. 
    ";

	ctx.reply(CONTENT).await?;

	Ok(())
}
// }}}
