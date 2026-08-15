//! Rename subtitle files to match the video files beside them.
//!
//! The crate is layered so that the two front-ends can never disagree:
//!
//! - [`planning`] decides what should be renamed, touching nothing;
//! - [`applying`] carries a plan out, and owns every safety rule;
//! - [`presentation`] holds the words and the match levels both front-ends use;
//! - [`cli`] and [`tui`] are just two ways of driving the three above.

pub mod applying;
pub mod cli;
pub mod names;
pub mod paths;
pub mod planning;
pub mod presentation;
pub mod similarity;
pub mod tui;
