use anyhow::anyhow;
use image::RgbaImage;

use crate::{
	assets::get_data_dir,
	context::{Error, UserContext},
	user::User,
};

use super::{
	chart::{Difficulty, Level},
	play::get_best_plays,
	score::{Grade, ScoringSystem},
};

// {{{ Goal
#[derive(Debug, Clone, Copy)]
pub enum Goal {
	/// PM X FTR<=charts
	PMCount(usize),
	/// PM a given number of packs
	PMPacks(usize),
	/// PM at least one song of each level up to a given one
	PMRelay(Level),
	/// Reach a given b30 ptt
	PTT(u32),
	/// Get a given grade or better on everything you own of a given level,
	/// with a minum of X owned charts.
	#[allow(dead_code)]
	GradeEntireLevel(Grade, Level, usize),
	/// Submit at least a given number of plays
	SubmitPlays(usize),
	/// PM the same song on all difficulties up to a given one
	MultiDifficultyPM(Difficulty),
}

impl Goal {
	// {{{ Texture names
	#[inline]
	pub fn texture_name(&self) -> String {
		match self {
			Self::PMCount(count) => format!("pm_count_{count:0>3}"),
			Self::PMPacks(count) => format!("pm_packs_{count:0>3}"),
			Self::PMRelay(level) => format!("pm_relay_{level}"),
			Self::PTT(min) => format!("ptt_min_{min:0>4}"),
			Self::GradeEntireLevel(grade, level, _) => format!("grade_{grade}_level_{level}"),
			Self::SubmitPlays(count) => format!("play_count_{count:0>6}"),
			Self::MultiDifficultyPM(difficulty) => format!("multi_pm_{difficulty}"),
		}
	}
	// }}}
	// {{{ Difficulty
	#[inline]
	pub fn difficulty(&self) -> Difficulty {
		use Difficulty::*;
		use Grade::*;
		use Level::*;

		match *self {
			Self::PMCount(count) if count < 25 => PST,
			Self::PMCount(count) if count < 100 => PRS,
			Self::PMCount(count) if count < 200 => FTR,
			Self::PMCount(count) if count < 350 => ETR,
			Self::PMCount(_) => BYD,
			Self::PMPacks(count) if count < 5 => PRS,
			Self::PMPacks(count) if count < 10 => FTR,
			Self::PMPacks(count) if count < 25 => ETR,
			Self::PMPacks(_) => BYD,
			Self::PMRelay(level) if level < Nine => PST,
			Self::PMRelay(level) if level < Ten => PRS,
			Self::PMRelay(level) if level < Eleven => FTR,
			Self::PMRelay(level) if level < Twelve => ETR,
			Self::PMRelay(_) => BYD,
			Self::PTT(amount) if amount < 1100 => PST,
			Self::PTT(amount) if amount < 1200 => PRS,
			Self::PTT(amount) if amount < 1250 => FTR,
			Self::PTT(amount) if amount < 1300 => ETR,
			Self::PTT(_) => BYD,
			Self::GradeEntireLevel(EXP, level, _) if level < Eight => PST,
			Self::GradeEntireLevel(EX, level, _) if level < Nine => PST,
			Self::GradeEntireLevel(EXP, level, _) if level < Nine => PRS,
			Self::GradeEntireLevel(EX, level, _) if level < Ten => PRS,
			Self::GradeEntireLevel(EXP, level, _) if level < Ten => FTR,
			Self::GradeEntireLevel(EX, level, _) if level < Eleven => FTR,
			Self::GradeEntireLevel(EXP, level, _) if level < Eleven => ETR,
			Self::GradeEntireLevel(EX, level, _) if level < Twelve => ETR,
			Self::GradeEntireLevel(EXP, _, _) => BYD,
			Self::GradeEntireLevel(EX, _, _) => BYD,
			Self::GradeEntireLevel(_, _, _) => PST,
			Self::SubmitPlays(count) if count < 500 => PST,
			Self::SubmitPlays(count) if count < 2500 => PRS,
			Self::SubmitPlays(count) if count < 5000 => FTR,
			Self::SubmitPlays(count) if count < 10000 => ETR,
			Self::SubmitPlays(_) => BYD,
			Self::MultiDifficultyPM(ETR) => FTR,
			Self::MultiDifficultyPM(BYD) => FTR,
			Self::MultiDifficultyPM(FTR) => PRS,
			Self::MultiDifficultyPM(_) => PST,
		}
	}
	// }}}
}
// }}}
// {{{ GoalStats
/// Stats collected in order to efficiently compute whether
/// a set of achievements were completed.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GoalStats {
	pm_count: usize,
	pmed_packs: usize,
	peak_pm_relay: Option<Level>,
	peak_ptt: u32,
	per_level_lowest_grades: [(Grade, usize); Level::LEVELS.len()],
	play_count: usize,
	multi_difficulty_pm_table: [bool; Difficulty::DIFFICULTIES.len()],
}

impl GoalStats {
	pub async fn make(
		ctx: &UserContext,
		user: &User,
		scoring_system: ScoringSystem,
	) -> Result<Self, Error> {
		let plays = get_best_plays(ctx, user.id, scoring_system, 0, usize::MAX, None)?
			.map_err(|s| anyhow!("{s}"))?;
		let conn = ctx.db.get()?;

		// {{{ PM count
		let pm_count = plays
			.iter()
			.filter(|(play, _, chart)| {
				play.score(scoring_system).0 >= 10_000_000 && chart.difficulty >= Difficulty::FTR
			})
			.count();
		// }}}
		// {{{ Play count
		let play_count = conn
			.prepare_cached("SELECT count() as count FROM plays WHERE user_id=?")?
			.query_row([user.id], |row| row.get(0))?;
		// }}}
		// {{{ Peak ptt
		let peak_ptt = conn
			.prepare_cached(
				"
        SELECT s.creation_ptt
        FROM plays p
        JOIN scores s ON s.play_id = p.id
        WHERE user_id = ?
        AND scoring_system = ?
        ORDER BY s.creation_ptt DESC
        LIMIT 1
      ",
			)?
			.query_row(
				(
					user.id,
					ScoringSystem::SCORING_SYSTEM_DB_STRINGS[scoring_system.to_index()],
				),
				|row| row.get(0),
			)
			.map_err(|_| anyhow!("No ptt history data found"))?;
		// }}}
		// {{{ Peak PM relay
		let peak_pm_relay = {
			let mut pm_checklist = [false; Level::LEVELS.len()];
			for (play, _, chart) in &plays {
				if play.score(scoring_system).is_pm() {
					pm_checklist[chart.level.to_index()] = true;
				}
			}

			pm_checklist
				.into_iter()
				.enumerate()
				.find(|(_, has_pm)| !*has_pm)
				.map_or(Some(Level::Twelve), |(i, _)| {
					Level::LEVELS.get(i.checked_sub(1)?).copied()
				})
		};
		// }}}
		// {{{ Per level lowest grades
		let mut per_level_lowest_grades = [(Grade::EXP, 0); Level::LEVELS.len()];
		for (play, _, chart) in plays {
			let element = &mut per_level_lowest_grades[chart.level.to_index()];
			*element = (
				element.0.min(play.score(scoring_system).grade()),
				element.1 + 1,
			);
		}
		// }}}

		Ok(GoalStats {
			pm_count,
			play_count,
			peak_ptt,
			peak_pm_relay,
			per_level_lowest_grades,
			pmed_packs: 0,
			multi_difficulty_pm_table: [false; Difficulty::DIFFICULTIES.len()],
		})
	}
}
// }}}
// {{{ Achievement
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Achievement {
	pub goal: Goal,
	pub texture: &'static RgbaImage,
}

impl Achievement {
	pub fn new(goal: Goal) -> Self {
		let texture_name = goal.texture_name();
		Self {
			goal,
			texture: Box::leak(Box::new(
				image::open(
					get_data_dir()
						.join("achievements")
						.join(format!("{texture_name}.png")),
				)
				.unwrap_or_else(|_| {
					panic!("Cannot read texture `{texture_name}` for achievement {goal:?}")
				})
				.into_rgba8(),
			)),
		}
	}
}

// }}}
// {{{ Achievement towers
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AchievementTower {
	pub achievements: Vec<Achievement>,
}

impl AchievementTower {
	pub fn new(achievements: Vec<Achievement>) -> Self {
		Self { achievements }
	}
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AchievementTowers {
	pub towers: Vec<AchievementTower>,
}

impl Default for AchievementTowers {
	// {{{ Construct towers
	fn default() -> Self {
		use Difficulty::*;
		use Goal::*;
		use Grade::*;
		use Level::*;

		// {{{ PM count tower
		let pm_count_tower = AchievementTower::new(vec![
			Achievement::new(PMCount(1)),
			Achievement::new(PMCount(5)),
			Achievement::new(PMCount(10)),
			Achievement::new(PMCount(20)),
			Achievement::new(PMCount(30)),
			Achievement::new(PMCount(40)),
			Achievement::new(PMCount(50)),
			Achievement::new(PMCount(75)),
			Achievement::new(PMCount(100)),
			Achievement::new(PMCount(125)),
			Achievement::new(PMCount(150)),
			Achievement::new(PMCount(175)),
			Achievement::new(PMCount(200)),
			Achievement::new(PMCount(250)),
			Achievement::new(PMCount(300)),
			Achievement::new(PMCount(350)),
			Achievement::new(PMCount(400)),
		]);
		// }}}
		// {{{ PM pack tower
		let pm_pack_tower = AchievementTower::new(vec![
			Achievement::new(PMPacks(1)),
			Achievement::new(PMPacks(3)),
			Achievement::new(PMPacks(5)),
			Achievement::new(PMPacks(7)),
			Achievement::new(PMPacks(10)),
			Achievement::new(PMPacks(15)),
			Achievement::new(PMPacks(20)),
			Achievement::new(PMPacks(25)),
			Achievement::new(PMPacks(30)),
			Achievement::new(PMPacks(35)),
			Achievement::new(PMPacks(40)),
			Achievement::new(PMPacks(45)),
			Achievement::new(PMPacks(50)),
		]);
		// }}}
		// {{{ PM relay tower
		let pm_relay_tower = AchievementTower::new(vec![
			Achievement::new(PMRelay(Seven)),
			Achievement::new(PMRelay(SevenP)),
			Achievement::new(PMRelay(Eight)),
			Achievement::new(PMRelay(EightP)),
			Achievement::new(PMRelay(Nine)),
			Achievement::new(PMRelay(NineP)),
			Achievement::new(PMRelay(Ten)),
			Achievement::new(PMRelay(TenP)),
			Achievement::new(PMRelay(Eleven)),
			Achievement::new(PMRelay(Twelve)),
		]);
		// }}}
		// {{{ PTT tower
		let ptt_tower = AchievementTower::new(vec![
			Achievement::new(PTT(0800)),
			Achievement::new(PTT(0900)),
			Achievement::new(PTT(1000)),
			Achievement::new(PTT(1050)),
			Achievement::new(PTT(1100)),
			Achievement::new(PTT(1125)),
			Achievement::new(PTT(1150)),
			Achievement::new(PTT(1200)),
			Achievement::new(PTT(1210)),
			Achievement::new(PTT(1220)),
			Achievement::new(PTT(1230)),
			Achievement::new(PTT(1240)),
			Achievement::new(PTT(1250)),
			Achievement::new(PTT(1260)),
			Achievement::new(PTT(1270)),
			Achievement::new(PTT(1280)),
			Achievement::new(PTT(1290)),
			Achievement::new(PTT(1300)),
		]);
		// }}}
		// {{{ EX(+) level tower
		let ex_level_tower = AchievementTower::new(vec![
			Achievement::new(GradeEntireLevel(EX, Seven, 5)),
			Achievement::new(GradeEntireLevel(EX, SevenP, 5)),
			Achievement::new(GradeEntireLevel(EX, Eight, 10)),
			Achievement::new(GradeEntireLevel(EX, EightP, 5)),
			Achievement::new(GradeEntireLevel(EX, Nine, 20)),
			Achievement::new(GradeEntireLevel(EX, NineP, 15)),
			Achievement::new(GradeEntireLevel(EX, Ten, 15)),
			Achievement::new(GradeEntireLevel(EX, TenP, 10)),
			Achievement::new(GradeEntireLevel(EX, Eleven, 5)),
			Achievement::new(GradeEntireLevel(EX, Twelve, 1)),
		]);

		let exp_level_tower = AchievementTower::new(vec![
			Achievement::new(GradeEntireLevel(EXP, Seven, 5)),
			Achievement::new(GradeEntireLevel(EXP, SevenP, 5)),
			Achievement::new(GradeEntireLevel(EXP, Eight, 10)),
			Achievement::new(GradeEntireLevel(EXP, EightP, 5)),
			Achievement::new(GradeEntireLevel(EXP, Nine, 20)),
			Achievement::new(GradeEntireLevel(EXP, NineP, 15)),
			Achievement::new(GradeEntireLevel(EXP, Ten, 15)),
			Achievement::new(GradeEntireLevel(EXP, TenP, 10)),
			Achievement::new(GradeEntireLevel(EXP, Eleven, 5)),
			Achievement::new(GradeEntireLevel(EXP, Twelve, 1)),
		]);
		// }}}
		// {{{ Submit plays
		let submit_plays_tower = AchievementTower::new(vec![
			Achievement::new(SubmitPlays(100)),
			Achievement::new(SubmitPlays(250)),
			Achievement::new(SubmitPlays(500)),
			Achievement::new(SubmitPlays(1000)),
			Achievement::new(SubmitPlays(2000)),
			Achievement::new(SubmitPlays(3000)),
			Achievement::new(SubmitPlays(4000)),
			Achievement::new(SubmitPlays(5000)),
			Achievement::new(SubmitPlays(7500)),
			Achievement::new(SubmitPlays(10000)),
		]);
		// }}}
		// {{{ Multi-difficulty PM
		let multi_difficulty_tower = AchievementTower::new(vec![
			Achievement::new(MultiDifficultyPM(PST)),
			Achievement::new(MultiDifficultyPM(PRS)),
			Achievement::new(MultiDifficultyPM(FTR)),
			Achievement::new(MultiDifficultyPM(ETR)),
			Achievement::new(MultiDifficultyPM(BYD)),
		]);
		// }}}

		let towers = vec![
			pm_count_tower,
			pm_pack_tower,
			pm_relay_tower,
			ptt_tower,
			ex_level_tower,
			exp_level_tower,
			submit_plays_tower,
			multi_difficulty_tower,
		];

		Self { towers }
	}
	// }}}
}
// }}}
