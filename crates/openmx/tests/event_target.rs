//! The freestyle event's target on the disc (S92): each freeride track's `hit.dat` names it, read by
//! [`game_sim::event::target`].

use game_render::Disc;
use game_sim::event;

#[test]
fn every_freeride_track_has_its_target() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped, no disc: {e}");
            return;
        }
    };
    let mut got = Vec::new();
    for track in ["free06", "free07", "free09", "free10", "free11"] {
        let res = disc.res(&format!("Tracks\\{track}_2.res")).unwrap_or_else(|e| panic!("{track}: {e}"));
        let text = res.read_suffix(&format!("{track}\\hit.dat")).unwrap_or_else(|e| panic!("{track}: {e}"));
        got.push(event::target(&String::from_utf8_lossy(&text)));
    }
    let want = [400_000, 300_000, 100_000, 150_000, 200_000].map(Some);
    assert_eq!(got, want, "free06, free07, free09, free10, free11");
}
