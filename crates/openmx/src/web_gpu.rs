//! The browser's stand-in for the GPU window (P196): the page draws in software until P48 puts the
//! renderer on its canvas, so this never opens — [`GpuWindow::open`] says why — and the screen falls
//! back to the page's canvas ([`crate::web_host`]). The same calls as `gpuwin.rs`'s, so the shell's
//! loop compiles unchanged.

use std::convert::Infallible;
use std::time::Duration;

use game_render::gpu::{Renderer, Target};

pub struct GpuWindow {
    pub renderer: Renderer,
    pub width: usize,
    pub height: usize,
    pub scale: usize,
    pub screen: (usize, usize),
    pub waited: Duration,
    pub refresh: Option<f32>,
    never: Infallible,
}

impl GpuWindow {
    pub fn open(_: Option<(usize, usize)>, _: usize, _: &str, _: bool) -> Result<GpuWindow, String> {
        Err("the browser draws in software for now (P48)".into())
    }

    pub fn set_vsync(&mut self, _: bool) {
        match self.never {}
    }

    pub fn waits(&self) -> bool {
        match self.never {}
    }

    pub fn poll(&mut self) {
        match self.never {}
    }

    pub fn closed(&self) -> bool {
        match self.never {}
    }

    pub fn focused(&self) -> bool {
        match self.never {}
    }

    pub fn clicks(&self) -> &[(i32, i32)] {
        match self.never {}
    }

    pub fn show_cursor(&self, _: bool) {
        match self.never {}
    }

    pub fn held(&self, _: u8) -> bool {
        match self.never {}
    }

    pub fn key_moments(&self) -> &[crate::window::KeyMoment] {
        match self.never {}
    }

    pub fn tapped(&self, _: u8) -> bool {
        match self.never {}
    }

    pub fn resized(&mut self) -> bool {
        match self.never {}
    }

    pub fn draw<T>(&mut self, _: usize, _: usize, _: impl FnOnce(&mut Renderer, &Target) -> T) -> Result<Option<T>, String> {
        match self.never {}
    }

    pub fn present(&mut self, _: &[u8], _: usize, _: usize) -> Result<(), String> {
        match self.never {}
    }

    pub fn present_again(&mut self, _: usize, _: usize) -> Result<(), String> {
        match self.never {}
    }
}
