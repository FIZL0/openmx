# P262 — native player build

2026-11-02. Task: `./kit plan P262`.

The user adopted the native-vs-original comparison's recommendation: player builds ride F15's native sim, the gate
stays exact. Every build players download now turns on openmx's `native-sim` by default — tools/build.sh (Linux, the
Steam Deck's, Windows; `--exact-sim` for the reference), tools/web.sh (the page; `--check`, the tests' page, stays
exact), tools/ios-ipa.sh (the app), and pages.yml's beta Linux job through `tools/build.sh features` — while
`cargo run -p openmx`, ci.yml and the gate stay exact. Tapes now name their sim (`sim native`) and the shell warns
when it plays one of the other sim; the release and beta notes say which sim the downloads run.
crates/openmx/tests/player_build.rs holds it. FINAL.
