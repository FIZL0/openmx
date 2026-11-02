# MD63a — MX vs. ATV's input limiter and steer ramp

2026-11-02. Task: `./kit plan MD63a`.

Under `Handling::MxVsAtv` the rider's levers now go through MX vs. ATV's own input limiter (no stick
squaring, the two-frame gas cut after the throttle comes on, the gas gate on the balance stunt) and the
front wheel's steering command through its [Joystick] InputRampAmount/InputRampTime steer ramp. Both are
run against the original off `state04.ee.bin` in `tests/mxvatv_step.rs`, 10,000 states each, and both
match word for word. The Original handling is untouched. Details in the numbered file.
