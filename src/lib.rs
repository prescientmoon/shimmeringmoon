#![allow(async_fn_in_trait)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::redundant_closure)]
// This sometimes triggers for rationals, where it doesn't make sense
#![allow(clippy::int_plus_one)]

pub mod arcaea;
pub mod assets;
pub mod bitmap;
pub mod commands;
pub mod context;
pub mod levenshtein;
pub mod logs;
pub mod recognition;
pub mod time;
pub mod transform;
pub mod user;
pub mod utils;
