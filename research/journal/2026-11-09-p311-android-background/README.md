# P311 — android background

2026-11-09. Task: `./kit plan P311`.

Leaving the Android app kept the songs playing like a music app. Now the activity's Pause pauses the ride,
saves the settings and pauses the AAudio stream (it plays again on Resume), and its Stop gives the ride's
models back, as iOS's P298 does. The back button is Escape (Circle/Back, the pause in a ride), and rumble
goes to the phone's or the controller's vibrators. Everything was seen on the Android 16 emulator except
rumble, which is untested.
