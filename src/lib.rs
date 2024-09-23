#![allow(async_fn_in_trait)]
#![feature(iter_map_windows)]
#![feature(let_chains)]
#![feature(array_try_map)]
#![feature(async_closure)]
#![feature(try_blocks)]
#![feature(thread_local)]
#![feature(generic_arg_infer)]
#![feature(iter_collect_into)]

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
