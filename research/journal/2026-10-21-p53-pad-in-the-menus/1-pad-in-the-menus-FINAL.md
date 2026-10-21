# 1 — pad in the menus (FINAL)

Task P53. 2026-10-21.

## What I did

The user reported the controller couldn't pause the ride or work the pause menu, and on the splash
Cross/Start sometimes did nothing. Their pad is a Nintendo Switch Pro Controller on another machine. I
found four separate causes and fixed each:

1. **Presses between frames were lost.** The menus read each pad's *held* state once per frame, so a tap
   that went down and up between two frames (easy at 5 fps under 4K) never happened. `Pads::event`
   (pad.rs) now queues every button and hat down-edge as it reads the js stream (up to 4 per control).
   Each frame hands one queued press per control to the menus as that frame's `taps`, and also ORs it
   into the report, so a tap is one frame of "down". `Seats::devices` returns `(device, report, taps)`,
   `Seats::taps(seat)` gives a seated pad's taps during a ride, and pads held in no seat have their queue
   cleared so old presses don't fire in the next menu. In menu.rs, `Reader::read_taps` /
   `Readers::read_taps` take a press as `now & !last | taps`. The keyboard got the same treatment:
   `tapped` flags in window.rs and gpuwin.rs, read through `Screen::pressed`, so Enter/Escape tapped
   inside one slow frame count too.
2. **The pause read the report after the ride's scheme had remapped it.** Under the default new scheme,
   `Scheme::New.ride` moves R2 onto Cross and takes Cross off. The pause read that report, so Cross (OK)
   did nothing and R2 acted as OK. main.rs now gives the pause each player's report as the pad sent it,
   plus that seat's taps.
3. **Pads were read by xpad's js numbers whatever they were.** joydev numbers buttons by which key codes
   the device has, so a Pro Controller under hid-nintendo came out as A=Circle, B=Cross, ZL=Start, +=R3,
   −=L3. Its right stick read as L2 and its d-pad hat as right-stick Y and R2. A DualSense came out with
   Options=L3 and R2=Start. `JsMap::of` builds the map from sysfs `capabilities/key` and `/abs`,
   numbered the way joydev numbers them (`js_order`). It reads face buttons by code, positionally for
   Sony and Nintendo and by xpad's X/Y convention otherwise. Digital ZL/ZR become full-pressure L2/R2
   when there's no trigger axis, and d-pad keys (`BTN_DPAD_*`) are read too. A node with no sysfs entry
   keeps the old xpad numbering.
4. **Nintendo labels.** On a Nintendo pad (vendor 057e or a name starting "Nintendo ") A confirms (Cross)
   and B goes back (Circle), as the labels say. `nintendo = positions` in the controls file swaps them to
   positions (B as Cross). + is Start and − is Select.

Also:
- hid-nintendo's IMU device (`INPUT_PROP_ACCELEROMETER`, or a name ending " (IMU)") is never read as a pad.
- The joycond "Nintendo Switch Combined Joy-Cons" virtual device is allowed through the virtual-pad filter.
- The splash lists the pads it hears ("Controllers: …") and the devices it skipped ("Not read: …").

Ruled out: `pause_for(seat)` mismatching seats with devices. The presses are indexed by world seat and
`Menu::step_seats` reads them that way. The "idle virtual pad takes 1P" suspect was ruled out too: P46's
claim-on-Cross already holds, and the new splash test proves it with an idle drifting pad plugged in
first.

**Taken from opentennis:** `crates/hst-play/src/play/legend.rs` (B362) picks a pad's style by vendor id
(054c Sony, 057e Nintendo). I took the same vendor ids for the positional/Nintendo decision. opentennis
reads pads through gilrs, which already maps by evdev code, so there was no js-numbering code to copy.
Its seats have no between-frames queue either; the queue is new here.

## What the original does

Nothing to port. These are the shell's menus and pad layer, which are ours to design (AGENT.md).

## How it was proved

Unit tests over the real read path: fake js fifos plus a fake sysfs tree that carries `capabilities` and `id`.
- `pad.rs` `a_switch_pro_controller_reads_as_its_labels_say`: hid-nintendo's key and abs codes, read
  out of this machine's `hid-nintendo.ko` (7.2.8, `procon_button_mappings`), each js button and
  axis to its control. The IMU device is skipped and listed. `nintendo = positions` swaps A and B.
- `an_xbox_pad_and_a_dualsense_read_by_their_keys`.
- `a_tap_between_two_frames_is_still_a_press` (held, re-pressed in one frame, two hat taps in one frame, cleared for a seatless pad).
- `the_map_follows_the_codes`.
- tests/menu.rs, the brief's three:
  - `a_press_and_release_between_two_frames_still_opens_the_pause_and_moves_its_cursor`
  - `a_seated_pad_s_start_opens_the_pause_and_its_d_pad_and_cross_work_it`: a seated Pro Controller's + / hat / A, which also shows the scheme's report losing Cross.
  - `on_the_splash_a_pad_s_cross_makes_it_1p_even_while_an_idle_virtual_pad_is_plugged_in_at_any_frame_rate`: 3-tick press, menu frames every 1, 2, 3, 4, 6 and 12 ticks, every phase.

`./kit check`: TESTS PASSED.

## Not verified / not 1:1

- **No real pad tried.** There is no Pro Controller, DualSense, 8BitDo or Xbox pad on this machine. The
  maps come from kernel sources/modules, not a live device. The Pro Controller codes were read from the
  installed hid-nintendo.ko. The DualSense codes (hid-playstation, positional, digital L2/R2 beside
  their axes) are from memory of the driver, not checked against this kernel's module.
- **No live 4K/GPU reproduction.** The kit's uinput pads are virtual, and the shell skips virtual
  devices, so a uinput Pro Controller would not be read. The tests drive the same fifo+sysfs read path
  the shell uses instead. The user should check on their machine.
- **8BitDo pads** were not modelled separately. In Switch mode they wear Nintendo's ids and are mapped as
  a Pro Controller. In X-input mode they're xpad. Their D-input mode (hid-generic, its own key codes) is
  unchecked.
- **hid-nintendo's stick Y sign** is assumed to match xpad's (up negative). It wasn't read from the module.
- **Pro Controller without joycond, over Bluetooth**, possibly appearing twice or coming and going: not
  reproduced. Each node is read on its own and the hot-plug code handles come and go, but a duplicate
  node would show as two pads.
- **Xbox pads over Bluetooth (hid-generic / xpadneo)** may number their keys differently. Unchecked.
- **Stick flicks inside one frame are not queued**, only buttons and the hat. The menu's held-direction
  repeat still counts frames, so at 5 fps it repeats slower in seconds.
- **At most one press per control per frame** reaches the menu (up to 4 queued). Faster mashing at 5 fps
  is spread over the next frames.
- **Keyboard taps** (gpuwin/window `tapped`) are covered only through `Screen::pressed`. No test drives a
  window.

## Next

Nothing left for this task. If the user's Pro Controller still misbehaves, the splash's
"Controllers:" and "Not read:" lines say what the shell saw. Start from those.
