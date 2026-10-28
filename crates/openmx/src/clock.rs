//! The shell's clock, on both builds (P196): `std::time`'s on the desktop; in the browser, whose
//! `std::time::Instant::now` has no clock to read and panics, the page's `performance.now()` and
//! `Date.now()` through `web-time`, which has the same types.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(target_arch = "wasm32")]
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
