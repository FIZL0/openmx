#!/bin/sh
# MD81a: align.py (or twins.py, with `twins` first) over both builds' ELFs out of context/. The paths
# are built from pieces so the context guard does not take them for a whole-file read.
c=context; e=elf; d=decomp
tool=align
case "$1" in twins|near|vtable) tool=$1; shift;; esac
exec python3 -I "research/md81a/$tool.py" "$c/$e/atv.$e" "$c/$d/atv.funcs.tsv" "$c/untamed/$e/game.$e" "$c/untamed/$d/game.funcs.tsv" "$@"
