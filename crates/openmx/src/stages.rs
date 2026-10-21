//! Where a frame's time goes (P47): the loop's stages, each charged the wall time since the stage
//! before it ended, and `--timings`' line a second of what each cost a frame on average.
//!
//! It is the shell's own stopwatch rather than a profiler's because the two questions it answers
//! are the shell's: which stage of *this* loop a slow frame spent its time in, on the machine the
//! complaint came from, with nothing installed; and whether a stage that should cost nothing after
//! the first frame — a file read, a texture decode — has started costing something again. Off by
//! default, and off it is one `Instant::now` a stage, which is nothing beside a frame.

use std::time::{Duration, Instant};

/// The loop's stages, in the order a frame runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The window's events and every seat's pad (P35's device scan among them).
    Input,
    /// The front end's pause, and the settings it saves.
    Menu,
    /// The fixed update: every seat's simulation (or online play's session).
    Sim,
    /// The body's and the chassis' animation, and the chassis' skin.
    Animation,
    /// The follow cameras.
    Camera,
    /// Clearing the frame and drawing the sky.
    Sky,
    /// The ground.
    Ground,
    /// The scene's props, the machines and the riders, the riders' skin included.
    Scene,
    /// The overlay and the front end's art, and a split view's copy into the frame.
    Hud,
    /// The frame onto the window.
    Present,
    /// `--shot`'s file and the line a second.
    Log,
    /// Sleeping out the rest of the 60 Hz frame.
    Pace,
}

/// Every stage, for naming them in order.
pub const STAGES: [Stage; 12] = [
    Stage::Input,
    Stage::Menu,
    Stage::Sim,
    Stage::Animation,
    Stage::Camera,
    Stage::Sky,
    Stage::Ground,
    Stage::Scene,
    Stage::Hud,
    Stage::Present,
    Stage::Log,
    Stage::Pace,
];

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Input => "input",
            Stage::Menu => "menu",
            Stage::Sim => "sim",
            Stage::Animation => "anim",
            Stage::Camera => "camera",
            Stage::Sky => "sky",
            Stage::Ground => "ground",
            Stage::Scene => "scene",
            Stage::Hud => "hud",
            Stage::Present => "present",
            Stage::Log => "log",
            Stage::Pace => "pace",
        }
    }
}

/// The stopwatch: what each stage has cost since the last report, and how many frames that was.
pub struct Stages {
    spent: [Duration; STAGES.len()],
    /// Over the whole session, for the line it ends on.
    total: [Duration; STAGES.len()],
    frames: u32,
    total_frames: u32,
    last: Instant,
}

impl Default for Stages {
    fn default() -> Self {
        Stages::new()
    }
}

impl Stages {
    pub fn new() -> Stages {
        Stages {
            spent: [Duration::ZERO; STAGES.len()],
            total: [Duration::ZERO; STAGES.len()],
            frames: 0,
            total_frames: 0,
            last: Instant::now(),
        }
    }

    /// Charge the time since the last mark to `stage`.
    pub fn mark(&mut self, stage: Stage) {
        let now = Instant::now();
        let i = STAGES.iter().position(|&s| s == stage).expect("every stage is listed");
        let d = now - self.last;
        self.spent[i] += d;
        self.total[i] += d;
        self.last = now;
    }

    /// One frame has ended.
    pub fn frame(&mut self) {
        self.frames += 1;
        self.total_frames += 1;
    }

    /// The milliseconds a frame each stage has cost since the last call, as one line, and start
    /// again.
    pub fn line(&mut self) -> String {
        let line = Stages::format(&self.spent, self.frames);
        self.spent = [Duration::ZERO; STAGES.len()];
        self.frames = 0;
        line
    }

    /// The same line over the whole session.
    pub fn session(&self) -> String {
        Stages::format(&self.total, self.total_frames)
    }

    fn format(spent: &[Duration; STAGES.len()], frames: u32) -> String {
        let frames = frames.max(1) as f64;
        let mut line = String::from("ms a frame:");
        let mut sum = 0.0;
        for (stage, d) in STAGES.iter().zip(spent) {
            let ms = d.as_secs_f64() * 1000.0 / frames;
            sum += ms;
            line.push_str(&format!(" {} {ms:.2}", stage.name()));
        }
        line.push_str(&format!(", all {sum:.2}"));
        line
    }
}

/// Every byte this process has read so far, from any file, device or socket: `rchar` in
/// `/proc/self/io`. `None` where there is no such file.
pub fn bytes_read() -> Option<u64> {
    let io = std::fs::read_to_string("/proc/self/io").ok()?;
    io.lines().find_map(|l| l.strip_prefix("rchar:")).and_then(|n| n.trim().parse().ok())
}

/// [`bytes_read`] as a mark to count from: less the bytes that the next look at `/proc/self/io`
/// will itself have read, so a process that reads nothing in between is shown reading nothing.
pub fn bytes_read_mark() -> Option<u64> {
    let (a, b) = (bytes_read()?, bytes_read()?);
    Some(b + (b - a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_names_every_stage_in_order_and_adds_them_up() {
        let mut s = Stages::new();
        s.spent[2] = Duration::from_millis(4);
        s.spent[6] = Duration::from_millis(6);
        s.frames = 2;
        let line = s.line();
        assert!(line.starts_with("ms a frame: input 0.00 menu 0.00 sim 2.00"), "{line}");
        assert!(line.contains(" ground 3.00 "), "{line}");
        assert!(line.ends_with(", all 5.00"), "{line}");
        // And it starts again.
        assert!(s.line().ends_with(", all 0.00"));
    }
}
