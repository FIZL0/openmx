//! The menu's own disc browser (P220): a desktop start with no disc, no dialog program and no
//! terminal — a launcher's start on a Linux box without zenity or kdialog — opens a page that walks
//! the folders for the `.iso` with the pad, the keyboard or the mouse, as `ios.rs`'s no-disc page
//! waits for one. [`crate::player::ask`] falls back to it; what it picks is checked, remembered and
//! started from as a picked disc is.
//!
//! [`Browser`] is the page's state and [`Browser::step`] its moves, plain functions of the folders on
//! disk; [`page`] draws it; [`browse`] is the window's loop.

use std::path::{Path, PathBuf};

/// One line of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The folder above.
    Up,
    /// A folder in this one, by name.
    Folder(String),
    /// A disc image in this one, by name.
    Disc(String),
}

impl Row {
    /// The line as the page shows it.
    pub fn label(&self) -> String {
        match self {
            Row::Up => "..  (up a folder)".into(),
            Row::Folder(n) => format!("{n}/"),
            Row::Disc(n) => n.clone(),
        }
    }
}

/// What the player did, read off a pad, the keyboard or the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    /// A page's worth of rows: Left and Right, L1 and R1, Page Up and Page Down.
    PageUp,
    PageDown,
    /// Cross, Enter or Space: open the folder, or take the disc.
    Open,
    /// Circle or Backspace: the folder above.
    Back,
    /// Straight to a row, the mouse's click.
    To(usize),
}

/// What a [`Move`] came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Stay,
    /// A disc image was opened: its path.
    Disc(PathBuf),
}

/// The rows a page holds below its title, at most.
pub const PAGE: usize = 10;

/// The page's state: the folder shown, its rows, the one lit, and a line to say (a folder that
/// would not open, a disc that is not the game's).
pub struct Browser {
    pub dir: PathBuf,
    pub rows: Vec<Row>,
    pub at: usize,
    pub said: Option<String>,
}

/// Whether `name` is a disc image's: `.iso`, any case.
fn is_disc(name: &str) -> bool {
    Path::new(name).extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("iso"))
}

/// `dir`'s rows: the folder above (unless `dir` is a root), then its folders, then its disc images,
/// each by name without regard to case. Hidden entries (a leading dot) are left out.
pub fn rows_of(dir: &Path) -> std::io::Result<Vec<Row>> {
    let mut folders = Vec::new();
    let mut discs = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let Ok(name) = entry.file_name().into_string() else { continue };
        if name.starts_with('.') {
            continue;
        }
        // a link is followed, so a linked folder opens and a linked image is offered
        let path = entry.path();
        if path.is_dir() {
            folders.push(name);
        } else if is_disc(&name) && path.is_file() {
            discs.push(name);
        }
    }
    let by_name = |a: &String, b: &String| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b));
    folders.sort_by(by_name);
    discs.sort_by(by_name);
    let up = dir.parent().is_some().then_some(Row::Up);
    Ok(up.into_iter().chain(folders.into_iter().map(Row::Folder)).chain(discs.into_iter().map(Row::Disc)).collect())
}

impl Browser {
    /// The page on `dir`, its first disc image lit when it has one, else its first row; a folder
    /// that will not read is an empty page that says so.
    pub fn open(dir: PathBuf) -> Browser {
        let mut b = Browser { dir: PathBuf::new(), rows: Vec::new(), at: 0, said: None };
        if let Err(e) = b.enter(dir.clone()) {
            b.dir = dir;
            b.said = Some(e);
        }
        b
    }

    /// Show `dir`, lighting its first disc image, else the folder the player came up out of
    /// (`from`), else its first row. Unread, the page stays as it was.
    fn enter_from(&mut self, dir: PathBuf, from: Option<&str>) -> Result<(), String> {
        let rows = rows_of(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let came = |r: &Row| matches!((r, from), (Row::Folder(n), Some(f)) if n == f);
        self.at = rows
            .iter()
            .position(|r| matches!(r, Row::Disc(_)))
            .or_else(|| rows.iter().position(came))
            .or_else(|| rows.iter().position(|r| *r != Row::Up))
            .unwrap_or(0);
        self.rows = rows;
        self.dir = dir;
        self.said = None;
        Ok(())
    }

    fn enter(&mut self, dir: PathBuf) -> Result<(), String> {
        self.enter_from(dir, None)
    }

    /// The folder above, lighting the one left.
    fn up(&mut self) {
        let Some(parent) = self.dir.parent().map(Path::to_path_buf) else { return };
        let left = self.dir.file_name().and_then(|n| n.to_str()).map(str::to_string);
        if let Err(e) = self.enter_from(parent, left.as_deref()) {
            self.said = Some(e);
        }
    }

    /// The first row a page shows, so that the lit one is on it.
    pub fn top(&self) -> usize {
        (self.at.saturating_sub(PAGE / 2)).min(self.rows.len().saturating_sub(PAGE))
    }

    /// Do `m`.
    pub fn step(&mut self, m: Move) -> Step {
        let last = self.rows.len().saturating_sub(1);
        match m {
            Move::Up => self.at = if self.at == 0 { last } else { self.at - 1 },
            Move::Down => self.at = if self.at >= last { 0 } else { self.at + 1 },
            Move::PageUp => self.at = self.at.saturating_sub(PAGE),
            Move::PageDown => self.at = (self.at + PAGE).min(last),
            Move::Back => self.up(),
            Move::To(i) if i < self.rows.len() => {
                self.at = i;
                return self.step(Move::Open);
            }
            Move::To(_) => {}
            Move::Open => match self.rows.get(self.at).cloned() {
                Some(Row::Up) => self.up(),
                Some(Row::Folder(n)) => {
                    let dir = self.dir.join(&n);
                    if let Err(e) = self.enter(dir) {
                        self.said = Some(e);
                    }
                }
                Some(Row::Disc(n)) => return Step::Disc(self.dir.join(n)),
                None => {}
            },
        }
        Step::Stay
    }
}

/// Where the page starts: the home folder (`HOME`, `USERPROFILE`), else the working directory.
pub fn start_dir_in(env: impl Fn(&str) -> Option<String>) -> PathBuf {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(|k| env(k).filter(|v| !v.is_empty()).map(PathBuf::from))
        .find(|d| d.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// The page's lines, top to bottom, at a text size of `size`: (top of the line's box, its height).
/// [`page`] draws on them and a click is read against them.
struct Layout {
    size: f32,
    /// The baseline of the title, the folder, the first row; the rows' pitch; the help's baseline.
    title: f32,
    folder: f32,
    first: f32,
    pitch: f32,
    help: f32,
}

fn layout(w: usize, h: usize) -> Layout {
    let size = (w.min(h * 2) as f32 / 40.0).max(11.0);
    let pitch = size * 1.45;
    let block = size * 1.8 * 1.5 + size * 1.5 + pitch * (PAGE as f32 + 0.5) + size * 2.5;
    let top = ((h as f32 - block) / 2.0).max(0.0);
    let title = top + size * 1.8 * 1.3;
    let folder = title + size * 1.6;
    let first = folder + pitch * 1.5;
    let help = first + pitch * PAGE as f32 + size * 2.0;
    Layout { size, title, folder, first, pitch, help }
}

/// The row a click at window height `y` lands on, if any.
pub fn row_at(b: &Browser, w: usize, h: usize, y: f32) -> Option<usize> {
    let l = layout(w, h);
    let line = ((y - (l.first - l.pitch * 0.8)) / l.pitch).floor();
    (line >= 0.0 && (line as usize) < PAGE).then(|| b.top() + line as usize).filter(|&i| i < b.rows.len())
}

/// `text` cut from the front to `room` pixels at `size`, an ellipsis where it was cut.
fn fit(ink: &crate::menu::ink::Type, weight: crate::menu::ink::Weight, text: &str, size: f32, room: f32) -> String {
    if ink.measure(weight, text, size) <= room {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    for cut in 1..chars.len() {
        let s = format!("…{}", chars[cut..].iter().collect::<String>());
        if ink.measure(weight, &s, size) <= room {
            return s;
        }
    }
    "…".into()
}

/// The page, drawn by the CPU into a frame of `w` by `h`.
pub fn page(b: &Browser, w: usize, h: usize) -> game_render::raster::Frame {
    use crate::menu::ink as art;
    let mut frame = game_render::raster::Frame::new(w, h, [0, 0, 0]);
    let ink = art::ink();
    let l = layout(w, h);
    let margin = l.size * 2.0;
    let room = w as f32 - margin * 2.0;
    let white = [255, 255, 255, 255];
    let grey = [150, 150, 150, 255];
    let lit = [255, 200, 40, 255];
    let mut out = Vec::new();
    let line = |out: &mut Vec<_>, weight: art::Weight, text: &str, x: f32, y: f32, s: f32, c: [u8; 4]| {
        let text = fit(ink, weight, text, s, room - (x - margin));
        ink.text(out, 0, weight, &text, x, y, s, 0.0, c);
    };
    // the title shrinks to fit a narrow window rather than lose its start
    let title = "Pick your MX Unleashed disc (SLUS-20767)";
    let fits = room / ink.measure(art::Weight::Heavy, title, 1.0);
    line(&mut out, art::Weight::Heavy, title, margin, l.title, (l.size * 1.8).min(fits), white);
    let folder = b.said.clone().unwrap_or_else(|| b.dir.display().to_string());
    line(&mut out, art::Weight::Bold, &folder, margin, l.folder, l.size, if b.said.is_some() { lit } else { grey });
    if b.rows.is_empty() {
        line(&mut out, art::Weight::Bold, "(no folders or .iso files here)", margin, l.first, l.size, grey);
    }
    let top = b.top();
    for (i, row) in b.rows.iter().enumerate().skip(top).take(PAGE) {
        let y = l.first + l.pitch * (i - top) as f32;
        let (weight, colour) = match (i == b.at, row) {
            (true, _) => (art::Weight::Heavy, lit),
            (false, Row::Disc(_)) => (art::Weight::Bold, white),
            (false, _) => (art::Weight::Bold, grey),
        };
        if i == b.at {
            line(&mut out, weight, ">", margin, y, l.size, colour);
        }
        line(&mut out, weight, &row.label(), margin + l.size * 1.2, y, l.size, colour);
    }
    if top + PAGE < b.rows.len() {
        let y = l.first + l.pitch * (PAGE as f32 - 0.2);
        line(&mut out, art::Weight::Bold, &format!("… {} more", b.rows.len() - top - PAGE), margin + l.size * 1.2, y, l.size * 0.8, grey);
    }
    let help = "Cross / Enter: open   Circle / Backspace: up a folder   Esc: quit";
    line(&mut out, art::Weight::Bold, help, margin, l.help, l.size * 0.85, grey);
    art::raster(&mut frame, &out, &[&ink.atlas]);
    frame
}

/// The keyboard's keys the page reads beyond [`crate::window::key`]'s, as X keycodes (the evdev
/// scancode and 8), which is what the window's keys are numbered by.
mod more_keys {
    pub const BACKSPACE: u8 = 22;
    pub const PAGE_UP: u8 = 112;
    pub const PAGE_DOWN: u8 = 117;
}

/// The moves a pad's presses (`taps`, the game's button bitmask) make.
pub fn pad_moves(taps: u16) -> Vec<Move> {
    use game_sim::input::Control;
    [
        (Control::Up, Move::Up),
        (Control::Down, Move::Down),
        (Control::Left, Move::PageUp),
        (Control::L1, Move::PageUp),
        (Control::Right, Move::PageDown),
        (Control::R1, Move::PageDown),
        (Control::Cross, Move::Open),
        (Control::Circle, Move::Back),
    ]
    .into_iter()
    .filter(|(c, _)| taps & c.mask() != 0)
    .map(|(_, m)| m)
    .collect()
}

/// The browser in a window of its own until a disc that opens is picked (its path) or the player
/// quits or closes it (`None`, as is a desktop with no window to open). `opens` says whether a
/// picked image is the game's disc; one that is not is said on the page and the walk goes on.
pub fn browse(opens: impl Fn(&Path) -> Result<(), String>) -> Option<PathBuf> {
    use crate::window::key;
    let mut window = match crate::gpuwin::GpuWindow::open(None, 1, "openMX: pick your disc", true) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("no window for the disc browser: {e}");
            return None;
        }
    };
    let mut seats = crate::pad::Seats::open(None, None, crate::pad::Wiring::default(), Default::default());
    let mut b = Browser::open(start_dir_in(|k| std::env::var(k).ok()));
    println!("disc: none, browsing for it from {}", b.dir.display());
    loop {
        window.poll();
        if window.closed() || window.tapped(key::ESCAPE) {
            return None;
        }
        let (w, h) = (window.width.max(1), window.height.max(1));
        let mut moves = Vec::new();
        for (code, m) in [
            (key::UP, Move::Up),
            (key::DOWN, Move::Down),
            (key::LEFT, Move::PageUp),
            (more_keys::PAGE_UP, Move::PageUp),
            (key::RIGHT, Move::PageDown),
            (more_keys::PAGE_DOWN, Move::PageDown),
            (key::RETURN, Move::Open),
            (key::SPACE, Move::Open),
            (more_keys::BACKSPACE, Move::Back),
        ] {
            if window.tapped(code) {
                moves.push(m);
            }
        }
        // the pads, each in a seat or loose: the keyboard is read above, so the seats read no keys
        let _ = seats.frame(&crate::pad::no_keys);
        let mut taps = (0..crate::menu::SEATS_MAX).fold(0, |t, s| t | seats.taps(s));
        taps = seats.loose().iter().fold(taps, |t, l| t | l.2);
        moves.extend(pad_moves(taps));
        moves.extend(window.clicks().iter().filter_map(|&(_, y)| row_at(&b, w, h, y as f32)).map(Move::To));
        for m in moves {
            if let Step::Disc(path) = b.step(m) {
                match opens(&path) {
                    Ok(()) => return Some(path),
                    Err(e) => {
                        println!("disc: {} won't open: {e}", path.display());
                        b.said = Some(format!("not the game's disc: {e}"));
                    }
                }
            }
        }
        let frame = page(&b, w, h);
        if window.present(&frame.color, w, h).is_err() {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder tree of its own under the temp folder.
    fn tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openmx-p220-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["Games/PS2", "Music", ".hidden", "b"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        for f in ["Games/PS2/MX Unleashed (USA).iso", "Games/PS2/other.ISO", "Games/PS2/notes.txt", "Games/readme.txt", "Z.iso"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        dir
    }

    #[test]
    fn rows_are_up_folders_then_discs_without_hidden_or_other_files() {
        let dir = tree("rows");
        assert_eq!(
            rows_of(&dir).unwrap(),
            vec![Row::Up, Row::Folder("b".into()), Row::Folder("Games".into()), Row::Folder("Music".into()), Row::Disc("Z.iso".into())]
        );
        assert_eq!(
            rows_of(&dir.join("Games/PS2")).unwrap(),
            vec![Row::Up, Row::Disc("MX Unleashed (USA).iso".into()), Row::Disc("other.ISO".into())]
        );
        assert_eq!(rows_of(Path::new("/")).unwrap().first().is_some_and(|r| *r == Row::Up), false);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn walks_down_to_the_disc_and_back_up() {
        let dir = tree("walk");
        let mut b = Browser::open(dir.clone());
        // a folder with a disc lights the disc
        assert_eq!(b.rows[b.at], Row::Disc("Z.iso".into()));
        b.at = b.rows.iter().position(|r| *r == Row::Folder("Games".into())).unwrap();
        assert_eq!(b.step(Move::Open), Step::Stay);
        assert_eq!(b.dir, dir.join("Games"));
        // no disc here: the first folder is lit, not the way up
        assert_eq!(b.rows[b.at], Row::Folder("PS2".into()));
        assert_eq!(b.step(Move::Open), Step::Stay);
        assert_eq!(b.step(Move::Open), Step::Disc(dir.join("Games/PS2/MX Unleashed (USA).iso")));
        assert_eq!(b.step(Move::Down), Step::Stay);
        assert_eq!(b.step(Move::Open), Step::Disc(dir.join("Games/PS2/other.ISO")));
        // Down from the last row wraps to the first, Up from the first to the last
        assert_eq!(b.step(Move::Down), Step::Stay);
        assert_eq!(b.at, 0);
        b.step(Move::Up);
        assert_eq!(b.at, 2);
        // Back, or Open on "..", goes up and lights the folder left
        b.step(Move::Back);
        assert_eq!((b.dir.clone(), b.rows[b.at].clone()), (dir.join("Games"), Row::Folder("PS2".into())));
        b.at = 0;
        b.step(Move::Open);
        assert_eq!(b.dir, dir);
        assert_eq!(b.rows[b.at], Row::Disc("Z.iso".into()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreadable_folder_is_said_and_the_page_stays() {
        let dir = tree("unread");
        let mut b = Browser::open(dir.clone());
        let before = b.rows.clone();
        b.rows.push(Row::Folder("gone".into()));
        b.at = b.rows.len() - 1;
        assert_eq!(b.step(Move::Open), Step::Stay);
        assert_eq!(b.dir, dir);
        assert!(b.said.as_deref().is_some_and(|s| s.contains("gone")), "{:?}", b.said);
        assert_eq!(&b.rows[..before.len()], &before[..]);
        let missing = Browser::open(dir.join("nowhere"));
        assert!(missing.rows.is_empty() && missing.said.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pages_and_clicks() {
        let mut b = Browser { dir: PathBuf::from("/x"), rows: (0..25).map(|i| Row::Disc(format!("{i:02}.iso"))).collect(), at: 0, said: None };
        b.step(Move::PageDown);
        assert_eq!((b.at, b.top()), (PAGE, PAGE / 2));
        b.step(Move::PageDown);
        b.step(Move::PageDown);
        assert_eq!((b.at, b.top()), (24, 25 - PAGE));
        b.step(Move::PageUp);
        assert_eq!(b.at, 24 - PAGE);
        // a click on a page's first row opens what is shown there
        let (w, h) = (1280, 720);
        let l = layout(w, h);
        let top = b.top();
        assert_eq!(row_at(&b, w, h, l.first - l.size * 0.3), Some(top));
        assert_eq!(row_at(&b, w, h, l.first + l.pitch - l.size * 0.3), Some(top + 1));
        assert_eq!(row_at(&b, w, h, 0.0), None);
        assert_eq!(b.step(Move::To(top + 1)), Step::Disc(PathBuf::from(format!("/x/{:02}.iso", top + 1))));
        assert_eq!(b.step(Move::To(99)), Step::Stay);
    }

    #[test]
    fn pad_presses_are_moves() {
        use game_sim::input::Control;
        assert_eq!(pad_moves(Control::Cross.mask() | Control::Down.mask()), vec![Move::Down, Move::Open]);
        assert_eq!(pad_moves(Control::Circle.mask()), vec![Move::Back]);
        assert_eq!(pad_moves(Control::R1.mask()), vec![Move::PageDown]);
        assert!(pad_moves(Control::Start.mask()).is_empty());
    }

    #[test]
    fn starts_home_else_here() {
        let dir = tree("home");
        let d = dir.display().to_string();
        assert_eq!(start_dir_in(|k| (k == "HOME").then(|| d.clone())), dir);
        assert_eq!(start_dir_in(|k| (k == "USERPROFILE").then(|| d.clone())), dir);
        assert_eq!(start_dir_in(|_| None), std::env::current_dir().unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_page_draws_with_a_long_folder_and_a_message() {
        let mut b = Browser::open(tree("draw"));
        b.dir = PathBuf::from(format!("/{}", "very long folder name/".repeat(20)));
        let f = page(&b, 640, 360);
        assert_eq!(f.color.len(), 640 * 360 * 3);
        assert!(f.color.iter().any(|&c| c != 0), "nothing drawn");
        b.said = Some("not the game's disc".into());
        page(&b, 200, 100);
    }
}
