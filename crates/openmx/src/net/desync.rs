//! A desync: two machines' hashes of one confirmed tick differ. Every time it is logged; the first
//! time, every seat's pads of every tick so far are written out (opentennis's `save_desync`) and the
//! host is asked to send its state ([`super::resync`]). The ride goes on either way.
//!
//! The report is text, one tick a line, so it reads in an editor and in [`Report::read`]:
//!
//! ```text
//! openmx desync
//! why Out of sync at tick 330 (hash ..., a peer's ...)
//! me g
//! seats host,g
//! tick 0 <seat 0's 20 pad bytes in hex> <seat 1's> ...
//! ```
//!
//! A tape of the original's pad bytes, every seat, from tick 0: [`Report::replay`] rides it again
//! offline on any [`Game`], so the machine that went wrong can be found without the network.

use std::path::PathBuf;

use super::{Game, Lineup, Netplay, Pad, WIRE, unwire};

/// The start of the line every desync logs.
pub const DESYNC: &str = "Out of sync";

/// The states differ at tick `t`: logged every time; the first time the report, and the host's state
/// asked for. Ticks a resync is already mending are not reported again.
pub(super) fn seen<S: Clone + 'static>(np: &mut Netplay<S>, (t, ours, theirs): (u32, u64, u64)) {
    let mended = np.resync.stale(t);
    np.desyncs.push((t, ours, theirs));
    if mended && np.report.is_some() {
        return;
    }
    let why = format!("{DESYNC} at tick {t} (hash {ours:016x}, a peer's {theirs:016x})");
    np.say(format!("online: {why}"));
    if !mended {
        super::resync::ask(np);
        if np.desynced {
            return;
        }
    }
    let at = save(np, &why);
    np.say(format!("online: inputs saved to {at}"));
    np.desynced = !mended;
    np.notice = format!("{why}: riding on.\nInputs saved to {at}");
}

/// Every seat's pads, every tick, and the line-up, once a session (later calls say where).
fn save<S: Clone + 'static>(np: &mut Netplay<S>, why: &str) -> String {
    if let Some(at) = &np.report {
        return at.display().to_string();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    // std's clock and process id panic in a browser (N4): the page's clock, and no id
    #[cfg(target_arch = "wasm32")]
    let unix = (js_sys::Date::now() / 1000.0) as u64;
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let me: String = np.me.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
    // two sessions in one process (the tests) or one second must not share a file
    #[cfg(not(target_arch = "wasm32"))]
    let pid = std::process::id();
    #[cfg(target_arch = "wasm32")]
    let pid = 0;
    let path = np.dumps.join(format!("desync-{unix}-{pid}-{n}-{me}.txt"));
    let report = Report { why: why.to_string(), me: np.me.clone(), lineup: np.lineup.clone(), pads: np.log.clone() };
    let saved = std::fs::create_dir_all(&np.dumps).and_then(|()| std::fs::write(&path, report.text()));
    match saved {
        Ok(()) => {
            np.report = Some(path.clone());
            path.display().to_string()
        }
        Err(e) => format!("(not saved: {e})"),
    }
}

/// A desync report, read back.
#[derive(Debug, PartialEq)]
pub struct Report {
    pub why: String,
    pub me: String,
    pub lineup: Lineup,
    pub pads: Vec<Vec<Pad>>,
}

impl Report {
    pub fn text(&self) -> String {
        let mut s = format!("openmx desync\nwhy {}\nme {}\nseats {}\n", self.why, self.me, self.lineup.text());
        for (t, pads) in self.pads.iter().enumerate() {
            s.push_str(&format!("tick {t}"));
            for p in pads {
                s.push(' ');
                for b in p.0 {
                    s.push_str(&format!("{b:02x}"));
                }
            }
            s.push('\n');
        }
        s
    }

    pub fn read(path: &std::path::Path) -> Result<Report, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut lines = text.lines();
        if lines.next() != Some("openmx desync") {
            return Err(format!("{}: not a desync report", path.display()));
        }
        let mut field = |name: &str| {
            lines.next().and_then(|l| l.strip_prefix(name)).map(|v| v.trim_start().to_string()).ok_or(format!("no `{name}` line"))
        };
        let (why, me, seats) = (field("why")?, field("me")?, field("seats")?);
        let lineup = Lineup::parse(&seats)?;
        let mut pads = Vec::new();
        for (t, l) in lines.enumerate() {
            let mut words = l.split(' ');
            if words.next() != Some("tick") || words.next() != Some(t.to_string().as_str()) {
                return Err(format!("line for tick {t}: {l}"));
            }
            let tick: Result<Vec<Pad>, String> = words
                .map(|w| {
                    let mut p = [0u8; WIRE];
                    if w.len() != 2 * WIRE {
                        return Err(format!("tick {t}: {w}"));
                    }
                    for (i, b) in p.iter_mut().enumerate() {
                        *b = u8::from_str_radix(&w[2 * i..2 * i + 2], 16).map_err(|e| format!("tick {t}: {e}"))?;
                    }
                    Ok(Pad(p))
                })
                .collect();
            pads.push(tick?);
        }
        Ok(Report { why, me, lineup, pads })
    }

    /// The report's ticks ridden again on `game` from its start: the hash before every tick.
    pub fn replay<G: Game>(&self, game: &mut G) -> Vec<u64> {
        let mut hashes = Vec::with_capacity(self.pads.len());
        for pads in &self.pads {
            hashes.push(game.hash());
            let reports: Vec<_> = pads.iter().map(|&p| unwire(p)).collect();
            game.step(&reports);
        }
        hashes
    }
}

/// Where the shell writes reports: `context/net/` (git-ignored: a report holds pad bytes and a
/// state, which are the user's own).
pub fn dumps() -> PathBuf {
    PathBuf::from("context/net")
}
