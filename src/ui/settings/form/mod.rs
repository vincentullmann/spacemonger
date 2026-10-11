//! Building blocks for the settings pages: labelled rows ([`rows`]) and the inputs they hold
//! ([`inputs`]).

mod inputs;
mod rows;

pub use inputs::{
    auto_color, check, choice, colors, excludes, font_family, number, outline, shown,
};
pub use rows::{group, label, line, pad_to, rows, section};
