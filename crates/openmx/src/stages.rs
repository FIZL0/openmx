//! Where a frame's time goes (P47): the loop's stages, each charged the wall time since the stage
//! before it ended, and `--timings`' line a second of what each cost a frame on average.
//!
//! It is the shell's own stopwatch rather than a profiler's because the two questions it answers
//! are the shell's: which stage of *this* loop a slow frame spent its time in, on the machine the
//! complaint came from, with nothing installed; and whether a stage that should cost nothing after
//! the first frame — a file read, a texture decode — has started costing something again. Off by
//! default, and off it is one `Instant::now` a stage, which is nothing beside a frame.

use std::time::Duration;

use crate::clock::Instant;

/// The loop's stages, in the order a frame runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The window's events and every seat's pad (P35's device scan among them).
    Input,
    /// The front end's pause, and the settings it saves.
    Menu,
    /// The fixed update: every seat's simulation (or online play's session).
    Sim,
    /// A world's streaming on the ride's thread (MD28): its tiles for the simulation, and its pieces'
    /// ground and props for the renderer as the riders come and go.
    Stream,
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
    /// Sleeping out the rest of a picture under `--draw-fps` (a headless ride's 60 a second). The
    /// ticks a frame runs are in `Input` to `Camera`, however many the wall clock owed it (P52).
    Pace,
}

/// Every stage, for naming them in order.
pub const STAGES: [Stage; 13] = [
    Stage::Input,
    Stage::Menu,
    Stage::Sim,
    Stage::Stream,
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
            Stage::Stream => "stream",
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
    /// This frame's stages so far, and the worst frame since the last report and over the session
    /// (MD28): an average a second hides the one frame a join costs.
    this: [Duration; STAGES.len()],
    worst: Worst,
    total_worst: Worst,
    /// The last frame, as a line, when its work went past [`OVER`] (MD28): which stages one slow
    /// frame spent it on, to say when it happened.
    slow: Option<String>,
}

/// The slowest frame's work — every stage but [`Stage::Pace`] — and what it spent it on, and how many
/// frames' work went past a 60 Hz display's picture.
#[derive(Clone, Copy, Default)]
struct Worst {
    spent: [Duration; STAGES.len()],
    over: u32,
    /// Each stage's dearest frame, whichever frame that was.
    peak: [Duration; STAGES.len()],
}

impl Worst {
    fn work(spent: &[Duration; STAGES.len()]) -> Duration {
        STAGES.iter().zip(spent).filter(|(s, _)| **s != Stage::Pace).map(|(_, d)| *d).sum()
    }

    fn take(&mut self, frame: &[Duration; STAGES.len()]) {
        for (p, d) in self.peak.iter_mut().zip(frame) {
            *p = (*p).max(*d);
        }
        let work = Worst::work(frame);
        if work > OVER {
            self.over += 1;
        }
        if work > Worst::work(&self.spent) {
            self.spent = *frame;
        }
    }

    /// `worst 3.20 ms (ground 2.10, hud 0.80), 0 over 16.7`: the slowest frame, its two dearest stages.
    fn format(&self) -> String {
        let mut dear: Vec<(Stage, Duration)> = STAGES.iter().copied().zip(self.spent).filter(|(s, _)| *s != Stage::Pace).collect();
        dear.sort_by_key(|(_, d)| std::cmp::Reverse(*d));
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let parts: Vec<String> = dear.iter().take(2).map(|(s, d)| format!("{} {:.2}", s.name(), ms(*d))).collect();
        format!("worst {:.2} ms ({}), {} over {:.1}", ms(Worst::work(&self.spent)), parts.join(", "), self.over, ms(OVER))
    }

    /// `peaks: sim 2.10 ground 0.40 ...`: every stage's dearest frame but the pace's.
    fn peaks(&self) -> String {
        let parts: Vec<String> =
            STAGES.iter().zip(self.peak).filter(|(s, _)| **s != Stage::Pace).map(|(s, d)| format!("{} {:.2}", s.name(), d.as_secs_f64() * 1000.0)).collect();
        format!("peaks: {}", parts.join(" "))
    }
}

/// A 60 Hz display's picture: a frame whose work takes longer misses one.
pub const OVER: Duration = Duration::from_micros(16_667);

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
            this: [Duration::ZERO; STAGES.len()],
            worst: Worst::default(),
            total_worst: Worst::default(),
            slow: None,
        }
    }

    /// Charge the time since the last mark to `stage`.
    pub fn mark(&mut self, stage: Stage) {
        let now = Instant::now();
        let i = STAGES.iter().position(|&s| s == stage).expect("every stage is listed");
        let d = now - self.last;
        self.spent[i] += d;
        self.total[i] += d;
        self.this[i] += d;
        self.last = now;
    }

    /// Charge the time since the last mark to no stage (P150): work that is not the ride's frame.
    pub fn skip(&mut self) {
        self.last = Instant::now();
    }

    /// Charge `d` of what `from` was charged to `to` instead: a stage measured inside another.
    pub fn shift(&mut self, from: Stage, to: Stage, d: Duration) {
        let [f, t] = [from, to].map(|s| STAGES.iter().position(|&x| x == s).expect("every stage is listed"));
        let d = d.min(self.this[f]);
        for spent in [&mut self.spent, &mut self.total, &mut self.this] {
            spent[f] -= d;
            spent[t] += d;
        }
    }

    /// One frame has ended.
    pub fn frame(&mut self) {
        self.frames += 1;
        self.total_frames += 1;
        self.worst.take(&self.this);
        self.total_worst.take(&self.this);
        self.slow = (Worst::work(&self.this) > OVER).then(|| Stages::format(&self.this, 1).replacen("ms a frame:", "slow frame, ms:", 1));
        self.this = [Duration::ZERO; STAGES.len()];
    }

    /// The frame just ended, as a line, if its work went past [`OVER`].
    pub fn take_slow(&mut self) -> Option<String> {
        self.slow.take()
    }

    /// The slowest frame since the last [`Stages::worst_line`], and how many went over [`OVER`],
    /// and start again.
    pub fn worst_line(&mut self) -> String {
        std::mem::take(&mut self.worst).format()
    }

    /// The same over the whole session.
    pub fn session_worst(&self) -> String {
        format!("{}; {}", self.total_worst.format(), self.total_worst.peaks())
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
        s.spent[7] = Duration::from_millis(6);
        s.frames = 2;
        let line = s.line();
        assert!(line.starts_with("ms a frame: input 0.00 menu 0.00 sim 2.00"), "{line}");
        assert!(line.contains(" ground 3.00 "), "{line}");
        assert!(line.ends_with(", all 5.00"), "{line}");
        // And it starts again.
        assert!(s.line().ends_with(", all 0.00"));
    }
}
