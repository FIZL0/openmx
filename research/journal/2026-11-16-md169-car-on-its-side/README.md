# MD169 — car on its side

2026-11-16. Task: `./kit plan MD169`.

The task guessed that the car's tires on MXU should read ground along the car's down when it lies on
its side. They should not: the original's tire probe also reads the heightfield straight down. Run
beside SLUS-21104's own frame from images poked onto their side, the port matches it word for word
for all five cars. On its side the original's buggy stands back up and the other four wreck tipped.
No port change. Two new tests: the side run beside the interpreter, and a drop on free09 where each
car wrecks tipped and is put back, or stands up, then rides on.
