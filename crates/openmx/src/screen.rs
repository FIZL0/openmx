//! The shell's window, whichever renderer is behind it (P44): the GPU's winit window by default, or
//! the X11 window the software rasteriser presents into (`--renderer software`).
//!
//! Both are the same thing to the loop — a size, a scale, the screen's size, keys by X keycode, a
//! close and a resize — so the loop holds a [`Screen`] and asks [`Screen::gpu`] only where it draws.

use crate::gpuwin::GpuWindow;
use crate::window::Window;

/// Which renderer draws the picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// wgpu: everything uploaded once, skinned and batched on the GPU (`game_render::gpu`).
    #[default]
    Gpu,
    /// The rasteriser on one CPU thread, presented through X11: the fallback.
    Software,
}

impl Backend {
    pub fn parse(s: &str) -> Result<Backend, String> {
        match s {
            "gpu" => Ok(Backend::Gpu),
            "software" | "cpu" => Ok(Backend::Software),
            _ => Err(format!("--renderer: {s}? gpu or software")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Backend::Gpu => "gpu",
            Backend::Software => "software",
        }
    }
}

enum Inner {
    X(Window),
    Gpu(Box<GpuWindow>),
}

/// A window on the screen.
pub struct Screen {
    /// The window's size in pixels, as of the last [`Screen::poll`].
    pub width: usize,
    pub height: usize,
    /// How many window pixels one frame pixel becomes.
    pub scale: usize,
    pub screen: (usize, usize),
    inner: Inner,
}

impl Screen {
    /// Open a window for `backend`. A GPU that cannot be had falls back to the software window, and
    /// says so.
    pub fn open(
        backend: Backend,
        display: &str,
        size: Option<(usize, usize)>,
        scale: usize,
        title: &str,
    ) -> Result<Screen, String> {
        let inner = match backend {
            Backend::Gpu => match GpuWindow::open(size, scale, title) {
                Ok(w) => Inner::Gpu(Box::new(w)),
                Err(e) => {
                    println!("renderer: no GPU window ({e}); drawing in software");
                    Inner::X(Window::open(display, size, scale, title)?)
                }
            },
            Backend::Software => Inner::X(Window::open(display, size, scale, title)?),
        };
        let mut s = Screen {
            width: 0,
            height: 0,
            scale,
            screen: (0, 0),
            inner,
        };
        s.sync();
        Ok(s)
    }

    fn sync(&mut self) {
        (self.width, self.height, self.scale, self.screen) = match &self.inner {
            Inner::X(w) => (w.width, w.height, w.scale, w.screen),
            Inner::Gpu(w) => (w.width, w.height, w.scale, w.screen),
        };
    }

    pub fn poll(&mut self) {
        match &mut self.inner {
            Inner::X(w) => w.poll(),
            Inner::Gpu(w) => w.poll(),
        }
        self.sync();
    }

    pub fn closed(&self) -> bool {
        match &self.inner {
            Inner::X(w) => w.closed(),
            Inner::Gpu(w) => w.closed(),
        }
    }

    pub fn held(&self, code: u8) -> bool {
        match &self.inner {
            Inner::X(w) => w.held(code),
            Inner::Gpu(w) => w.held(code),
        }
    }

    /// Down now, or pressed since the last poll though let go again (P53): a key's state for a
    /// frame, so a tap inside one slow frame is still a press.
    pub fn pressed(&self, code: u8) -> bool {
        self.held(code)
            || match &self.inner {
                Inner::X(w) => w.tapped(code),
                Inner::Gpu(w) => w.tapped(code),
            }
    }

    pub fn resized(&mut self) -> bool {
        let r = match &mut self.inner {
            Inner::X(w) => w.resized(),
            Inner::Gpu(w) => w.resized(),
        };
        self.sync();
        r
    }

    /// Show a frame the CPU drew.
    pub fn present(&mut self, rgb: &[u8], width: usize, height: usize) -> Result<(), String> {
        let out = match &mut self.inner {
            Inner::X(w) => w.present(rgb, width, height),
            Inner::Gpu(w) => w.present(rgb, width, height),
        };
        self.sync();
        out
    }

    /// The GPU window, where the GPU draws.
    pub fn gpu(&mut self) -> Option<&mut GpuWindow> {
        match &mut self.inner {
            Inner::X(_) => None,
            Inner::Gpu(w) => Some(w),
        }
    }
}
