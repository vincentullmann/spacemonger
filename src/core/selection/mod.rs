//! Selection state and the operations on it: clicks, arrow keys, rectangle select.

mod marquee;
mod navigation;
mod state;

pub use marquee::Marquee;
pub use navigation::Nav;
pub use state::Selection;
