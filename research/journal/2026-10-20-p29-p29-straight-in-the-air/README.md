# P29 — p29 straight in the air

2026-10-20. Task: `./kit plan P29`.

The bike went crooked in the air: off a jump with the stick alone it left the lip square and came
down twisted. The solver was not doing it. Hands off, race125p flies a jump straight and lands rolled
0.00° with its heading on its velocity, at 30 Hz and 60 Hz, in the test harness and in the shell's new
`--trace-air`. What twisted was the drawing. The shell stepped the chassis' animation machine with both
request axes at zero, and in the air the phase axis is the `bike_air_*` crossup sweep, so every flight
drew the bike crossed all the way up (61.8° on the `Bike` node). `Chassis::riding` hands it the rider's
own crossup and fore-and-aft, the axes the original's bike request block carries, and the drawn twist
is 1.6°. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
