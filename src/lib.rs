#![feature(lazy_cell)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]
pub mod backend;
pub mod config;
pub mod interval;
pub mod msg;
pub mod neighbourhood;
pub mod notename;
pub mod pattern;
pub mod process;
pub mod tui;
pub mod util;
