# P342 — phone world hitches

2026-11-14. Task: `./kit plan P342`.

MX World's long frames on the iPhone (build 4179) came from three things, none of them the tile's ground
or paint uploads. The first frame back from the app switcher uploaded every model given back (now 4 ms a
frame). A jump to a marker waited 140 ms for fine ground over the coarse already held (no longer waited
for). A tile's cars were read on the ride's thread (now on the dresser's). Each second's line now splits
present into upload, submit and wait, and a `video:` line says which rows a ride runs on. The fresh
phone default stays the Phone preset, which Auto already gives iOS and Android. The user's rows were
picked by hand and cost ~46% more GPU and twice the triangles. Not verified on a phone: P347. Details in
1-phone-world-hitches-FINAL.md.
