# MD156 — mxvatv car update replay

2026-11-16. Task: `./kit plan MD156`.

MD107's five car drives now step the car's once-a-frame update (`atv_car::class_update`, with
`OwnUpdate` inside) beside the interpreter, as MD135 does the world-step frame. Every update of all
five drives (730 each) matches the original word for word, with the same calls in the same order.
The buggy class's own step (`atv_car::class_step`, MD151) was already ported. Its part words are
compared on the buggy, baja and sandrail drives. The golf cart and the truck have their own class
steps, which are not ported yet; they were split off as MD156a and MD156b.
