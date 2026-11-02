# P245 — firefox gamepad

2026-11-02. Task: `./kit plan P245`.

A pad test that goes through a browser's own Gamepad API, beside P236's script stand-in in chromium.
`/dev/uinput` is still root's (0600, no ACL), so it is Firefox's gamepad test service: headless Firefox 159
(`tools/firefox.sh`, pinned and checked against its SHA-256), driven over WebDriver BiDi by
`crates/openmx/tests/web/firefox_gamepad.mjs`, run as `tests/web.rs::a_firefox_pad_rides`. The pad is plugged in
on the splash, is not listed until its Cross is pressed, that press makes it 1P, Cross reaches a ride, L2 holds
the speedometer at 0 and R2 moves it. Doing it found two Firefox-only page bugs, filed as B13 (WebGL2 path never
rides) and B14 (the persistent-storage prompt holds the start).
