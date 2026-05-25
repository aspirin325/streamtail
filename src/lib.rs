#![forbid(unsafe_code)]

pub mod app;
pub mod cli;
pub mod color;
pub mod diff;
pub mod fetcher;
pub mod renderer;
pub mod retry;

pub use app::{run, run_with, AppError};
