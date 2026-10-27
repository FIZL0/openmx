#!/bin/bash
# One still from a C12 film: research/c12/still.sh NAME SECONDS OUT.jpg (context/c12/NAME.mp4).
ffmpeg -v error -y -ss "$2" -i "context/c12/$1.mp4" -frames:v 1 "$3"
