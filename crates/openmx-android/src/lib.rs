//! The Android phone's side of the shell (P308), as opentennis's `hst-base/src/android.rs` and
//! `android_pad.rs` are (M99, M101, M102, M105, M140, B418, B455): the app's files folder and the log
//! in it, the crash's last words, the disc picked with the system's file picker, the window's safe
//! area, the input devices a pad is listed by, the
//! phone's own HTTPS certificate check, and the updater's hand-over of a new .apk to the installer.
//!
//! Every JNI and libc call is here, behind safe functions, so `crates/openmx` goes on forbidding
//! unsafe code. Only [`pad`]'s mapping of Android's key codes and axes to the standard pad is built
//! everywhere: it is plain logic, and the desktop's tests prove it.

pub mod pad;

#[cfg(target_os = "android")]
mod sys;
#[cfg(target_os = "android")]
pub use sys::*;
