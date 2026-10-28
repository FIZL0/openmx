//! The shell's threads, on both builds (P196): `std::thread` on the desktop; in the browser, whose
//! module has one thread and no way to start another, the same calls with each job run where it is
//! started, before the call returns.
//!
//! What that changes in the page: a load's build runs before its loading screen draws a frame, and a
//! world's first ground and dressing build inline. A job meant to run beside the loop for as long as
//! the ride (a world's workers waiting on a queue) is still `std::thread::spawn`, which the browser
//! refuses with a panic that says so. The desktop's threads are untouched.

#[cfg(not(target_arch = "wasm32"))]
pub use std::thread::{JoinHandle, available_parallelism, scope, spawn};

#[cfg(target_arch = "wasm32")]
pub use one::*;

#[cfg(target_arch = "wasm32")]
mod one {
    use std::any::Any;
    use std::marker::PhantomData;
    use std::num::NonZeroUsize;

    type Joined<T> = Result<T, Box<dyn Any + Send + 'static>>;

    /// One: the page's.
    pub fn available_parallelism() -> std::io::Result<NonZeroUsize> {
        Ok(NonZeroUsize::MIN)
    }

    /// A job that has run, and what it gave.
    pub struct JoinHandle<T>(T);

    impl<T> JoinHandle<T> {
        pub fn join(self) -> Joined<T> {
            Ok(self.0)
        }

        pub fn is_finished(&self) -> bool {
            true
        }
    }

    /// `f`, run now.
    pub fn spawn<F: FnOnce() -> T, T>(f: F) -> JoinHandle<T> {
        JoinHandle(f())
    }

    /// `std::thread::scope`, its jobs run as they are spawned.
    pub fn scope<'env, F, T>(f: F) -> T
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
    {
        f(&Scope(PhantomData))
    }

    pub struct Scope<'scope, 'env: 'scope>(PhantomData<(&'scope (), &'env ())>);

    impl<'scope> Scope<'scope, '_> {
        pub fn spawn<F: FnOnce() -> T + 'scope, T: 'scope>(&'scope self, f: F) -> JoinHandle<T> {
            JoinHandle(f())
        }
    }
}
