#!/bin/bash
# F27a1: usage bash research/f27a1/pcm.sh DIR NAME OUT_PREFIX: DIR/NAME.mp4 to mono and stereo s16 at 22050 Hz
ffmpeg -v error -y -i "$1/$2.mp4" -vn -ac 1 -ar 22050 -f s16le "$3.pcm"
ffmpeg -v error -y -i "$1/$2.mp4" -vn -ac 2 -ar 22050 -f s16le "$3_st.pcm"
