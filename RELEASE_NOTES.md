<!-- What's new since the last release, for players, in plain English: one "- " line per change. tools/release.sh puts this in the release and empties it. -->
- Linux / Steam Deck: the sound is no longer late. It now plays straight to the sound card (about 25 ms) instead of through `pw-cat`, which held 100 ms or more and drifted later the longer you played.
- Windows: sound plays again (the sound device was closed right after it opened).
- Steam Deck: picking your disc image no longer leaves a black screen; the game restarts itself after the pick.
