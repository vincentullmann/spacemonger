//! The camera: zoom, pan, animated moves, fitting a folder to the window, and holding the view
//! steady while the window is resized.
//!
//! The camera is the scan root's box in view coordinates. It always has the view's shape and
//! is only ever scaled evenly; a folder that should exactly fill the window is stretched with a
//! [`Reshape`](crate::core::layout::Reshape) instead (see [`Fit`]).

mod anchor;
mod animation;
mod fit;
mod framing;
mod state;

pub use anchor::Anchor;
pub use animation::Anim;
pub use fit::Fit;
pub use state::Camera;

#[cfg(test)]
mod tests;
