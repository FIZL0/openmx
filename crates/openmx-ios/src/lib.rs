//! The iPhone's side of the shell (P200), as opentennis's `hst-base/src/ios.rs` (M91, M105, M144, B694):
//! the app's Documents folder and the log in it, the crash reports a phone with no Mac attached has
//! no other way to show, the memory iOS counts, the safe area, the sound out through an AudioQueue,
//! the GameController framework's pads and the rotation lock's landscape hold.
//!
//! Every Objective-C and C call is here, behind safe functions, so `crates/openmx` goes on forbidding
//! unsafe code. Only [`Hold`]'s decision is built everywhere: it is plain logic, and the desktop's
//! tests prove it.

mod hold;
pub use hold::{Hold, Holder};

#[cfg(target_os = "ios")]
mod sys;
#[cfg(target_os = "ios")]
pub use sys::*;
