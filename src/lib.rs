#![allow(async_fn_in_trait)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::redundant_closure)]
#![feature(iter_map_windows)]
#![feature(let_chains)]
#![feature(array_try_map)]
#![feature(async_closure)]
#![feature(try_blocks)]
#![feature(thread_local)]
#![feature(generic_arg_infer)]
#![feature(iter_collect_into)]
#![feature(stmt_expr_attributes)]

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
