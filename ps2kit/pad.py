"""A virtual controller, so a script can play the game.

Why: half of what you want to measure only happens after someone presses a button — walk to the menu, start
a level, swing at the ball. PCSX2 has no "press X" IPC command, so the kit creates a *real* gamepad at the OS
level and PCSX2 binds it like any other pad. That also means a kit-managed copy can be paused and
screenshotted with no window to click on (its TogglePause and Screenshot hotkeys are bound to pad buttons —
see `emu.copy_config`).

    ./kit pad serve &                          # creates the device; keep it running
    ./kit pad send "press cross 120"           # hold Cross for 120 ms
    ./kit pad send "stick l 0 -1" "sleep 300" "release"
    ./kit pad send "dpad right 100; press cross 100"   # ';' separates commands on one line
    ./kit pad script moves.txt                 # one command per line, '#' comments

Commands: press <btn> <ms> · down <btn> · up <btn> · dpad <dir> <ms> · stick <l|r> <x> <y> (-1..1,
          y down = +1) · sleep <ms> (alias wait) · release (everything neutral).
Durations are **milliseconds**; a `120f` suffix means 120 frames instead, since every other kit tool counts
in frames. A press shorter than one frame is pointless — PCSX2 reads the pad once per frame — so the script
says so instead of pressing nothing.
Buttons, by PS2 name: cross circle square triangle start l1 r1 l2 r2 l3 r3 up down left right guide.

`select` is refused on purpose: PCSX2's own hotkeys are Select + shoulder combinations, so a script that
"just presses Select" silently saves a state, toggles turbo or loads over your game. The reference project
learned that the hard way.

**Timing is wall-clock, not frame-exact.** A `press cross 120` lands wherever the 120 ms happen to fall;
re-running a script does not reproduce the same frames. For anything that must be frame-exact (a replay you
intend to compare against your port) record PCSX2's own input recording instead and play that back.
"""
from __future__ import annotations

import os
import sys
import time
from pathlib import Path

from ps2kit import platform_ as plat

# PS2 button names → the Xbox-layout meaning SDL will see. Everything speaks PS2 names because the game's
# own documentation and the decompilation do.
FACE = ('cross', 'circle', 'square', 'triangle')
DPAD = ('up', 'down', 'left', 'right')
TRIGGERS = ('l2', 'r2')
BUTTONS = FACE + ('start', 'l1', 'r1', 'l3', 'r3', 'guide') + DPAD + TRIGGERS


def _fps() -> float:
    """Frames per second for duration suffixes. PAL discs run at 50, everything else at 60.

    Only used to convert `120f` into wall-clock; nothing here is frame-exact either way.
    """
    try:
        from ps2kit.config import cfg
        r = (cfg(required=False).region or '').upper()
    except Exception:                      # no kit.toml yet: a script should still run
        r = ''
    return 50.0 if r in ('EU', 'PAL', 'E') else 60.0


def fifo() -> Path:
    """Where `serve` listens. One per PCSX2 copy, so parallel agents never press each other's buttons."""
    return plat.rt(f'pad{plat.instance()}.fifo')


# --- backends ----------------------------------------------------------------

class PadBackend:
    """What a backend must do. `serve()` drives it; `Script` turns text into these calls.

    Kept this thin on purpose: the Windows edition adds a ViGEm class with the same five methods and
    changes nothing else in the kit.
    """

    name = 'none'

    def open(self) -> str:
        raise NotImplementedError

    def button(self, name: str, down: bool) -> None:
        raise NotImplementedError

    def axis(self, side: str, x: float, y: float) -> None:
        raise NotImplementedError

    def close(self) -> None:
        pass


class UinputPad(PadBackend):
    """Linux: a kernel-level pad via /dev/uinput and python-evdev.

    It identifies as a Microsoft Xbox 360 pad so SDL (and therefore PCSX2) applies its standard mapping with
    no per-device configuration. A kit copy N instead uses the pid.codes test vendor with product 0x500N, so
    that `SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT=0x1209/0x500N` pins copy N to pad N and nothing else —
    without it, eight copies all grab pad 1 and every agent plays the same game.
    """

    name = 'uinput'

    def open(self) -> str:
        ok, why = plat.pad_available()
        if not ok:
            raise SystemExit(f'no virtual pad: {why}')
        from evdev import AbsInfo, UInput, ecodes as e
        self.e = e
        # evdev's BTN_NORTH/BTN_WEST are swapped relative to their names; use the BTN_A..BTN_Y aliases,
        # which are what SDL reads to guess "this is an Xbox layout".
        self.keys = {'cross': e.BTN_A, 'circle': e.BTN_B, 'square': e.BTN_X, 'triangle': e.BTN_Y,
                     'start': e.BTN_START, 'l1': e.BTN_TL, 'r1': e.BTN_TR,
                     'l3': e.BTN_THUMBL, 'r3': e.BTN_THUMBR, 'guide': e.BTN_MODE}
        self.hats = {'up': (e.ABS_HAT0Y, -1), 'down': (e.ABS_HAT0Y, 1),
                     'left': (e.ABS_HAT0X, -1), 'right': (e.ABS_HAT0X, 1)}
        self.trigs = {'l2': e.ABS_Z, 'r2': e.ABS_RZ}
        stick = AbsInfo(0, -32768, 32767, 16, 128, 0)
        trig = AbsInfo(0, 0, 255, 0, 0, 0)
        hat = AbsInfo(0, -1, 1, 0, 0, 0)
        caps = {e.EV_KEY: list(self.keys.values()),
                e.EV_ABS: [(e.ABS_X, stick), (e.ABS_Y, stick), (e.ABS_RX, stick), (e.ABS_RY, stick),
                           (e.ABS_Z, trig), (e.ABS_RZ, trig),
                           (e.ABS_HAT0X, hat), (e.ABS_HAT0Y, hat)]}
        n = plat.instance()
        self.ui = UInput(caps,
                         name=f'ps2kit pad {n}' if n else 'Microsoft X-Box 360 pad',
                         vendor=0x1209 if n else 0x045E,
                         product=(0x5000 + int(n)) if n else 0x028E,
                         version=0x110, bustype=e.BUS_USB)
        return self.ui.device.path

    def button(self, name: str, down: bool) -> None:
        if name in self.keys:
            self.ui.write(self.e.EV_KEY, self.keys[name], int(down))
        elif name in self.hats:
            axis, v = self.hats[name]
            self.ui.write(self.e.EV_ABS, axis, v if down else 0)
        elif name in self.trigs:
            self.ui.write(self.e.EV_ABS, self.trigs[name], 255 if down else 0)
        else:
            return
        self.ui.syn()

    def axis(self, side: str, x: float, y: float) -> None:
        e = self.e
        ax, ay = (e.ABS_X, e.ABS_Y) if side == 'l' else (e.ABS_RX, e.ABS_RY)
        self.ui.write(e.EV_ABS, ax, int(max(-1.0, min(1.0, x)) * 32767))
        self.ui.write(e.EV_ABS, ay, int(max(-1.0, min(1.0, y)) * 32767))
        self.ui.syn()

    def close(self) -> None:
        self.ui.close()


class VigemPad(PadBackend):  # pragma: no cover - the Windows edition fills this in
    """Windows: an Xbox 360 pad through ViGEmBus (`pip install vgamepad`).

    Deliberately a stub here: it is listed so the dispatch below has nothing Windows-specific in it, and so
    the Windows edition is one class plus `platform_.py`.
    """

    name = 'vigem'

    def open(self) -> str:
        raise SystemExit('the vigem pad backend is not implemented in this edition — '
                         'see docs/INTERNALS.md (pad.py owns the backends)')


BACKENDS = {'uinput': UinputPad, 'vigem': VigemPad}


def backend() -> PadBackend:
    b = plat.pad_backend()
    if b not in BACKENDS:
        ok, why = plat.pad_available()
        raise SystemExit(f'no virtual pad on this system: {why or b}')
    return BACKENDS[b]()


# --- the script language -----------------------------------------------------

class Script:
    """Runs pad command lines against a backend. Separate from `serve` so a test can drive it headless."""

    def __init__(self, dev: PadBackend):
        self.dev = dev

    def _ms(self, text: str) -> float:
        """A duration in milliseconds. `120` = 120 ms, `8f` = 8 frames, `80ms` = 80 ms."""
        t = str(text).strip().lower()
        if t.endswith('ms'):
            return float(t[:-2])
        if t.endswith('f'):
            return float(t[:-1]) * (1000.0 / _fps())
        v = float(t)
        if 0 < v < 1000.0 / _fps():
            print(f'pad: {text} ms is shorter than one frame at {_fps():g} fps, so the game may never '
                  f'see it — durations here are milliseconds; write {t}f to mean frames', flush=True)
        return v

    def line(self, text: str) -> None:
        text = text.split('#', 1)[0].strip()
        if not text:
            return
        if ';' in text:                     # "dpad right 60; press cross 100" as one line
            for part in text.split(';'):
                self.line(part)
            return
        p = text.split()
        c = p[0].lower()
        try:
            if c in ('press', 'dpad'):
                # `dpad right 60` is just a press; it reads better in a menu-walking script.
                # The duration is parsed *before* the button goes down: `press cross soon` used to press
                # it and then raise, leaving the pad holding Cross for the rest of the session.
                ms = self._ms(p[2]) if len(p) > 2 else 100.0
                self._key(p[1], True)
                time.sleep(ms / 1000)
                self._key(p[1], False)
            elif c in ('down', 'up'):
                self._key(p[1], c == 'down')
            elif c == 'stick':
                self.dev.axis('l' if p[1].lower().startswith('l') else 'r', float(p[2]), float(p[3]))
            elif c in ('sleep', 'wait'):
                time.sleep(self._ms(p[1]) / 1000)
            elif c == 'release':
                for b in BUTTONS:
                    self._key(b, False)
                self.dev.axis('l', 0, 0)
                self.dev.axis('r', 0, 0)
            elif c == 'select':
                self._key(c, True)      # say *why* it is refused, not "unknown command"
            elif c in BUTTONS:
                print(f'pad: {c!r} is a button, not a command — did you mean `press {c} 100`?', flush=True)
            else:
                print(f'pad: unknown command {text!r} '
                      '(press/down/up/dpad/stick/sleep/release)', flush=True)
        except (IndexError, ValueError):
            print(f'pad: bad arguments: {text!r}', flush=True)

    def _key(self, name: str, down: bool) -> None:
        name = name.lower()
        if name == 'select':
            # Not an oversight: PCSX2 binds save-state/load-state/turbo to Select + a shoulder button.
            if down:   # once per press, not again on the release
                print('pad: refusing `select` — PCSX2 hotkeys are Select + shoulder combinations, so '
                      'sending it can save or load a state behind your back', flush=True)
            return
        if name not in BUTTONS:
            print(f'pad: unknown button {name!r} (have: {" ".join(BUTTONS)})', flush=True)
            return
        self.dev.button(name, down)


# --- serve / send ------------------------------------------------------------

def serve() -> int | None:
    """Create the pad and run commands from the FIFO until killed. One server per PCSX2 copy."""
    lock = plat.Lock('pad')
    lock.path = Path(str(fifo()) + '.lock')      # the lock sits beside the fifo it guards
    if not lock.take(block=False):
        print(f'pad: pad {plat.instance() or 0} is already being served', file=sys.stderr)
        return 0
    dev = backend()
    path = dev.open()
    f = fifo()
    if not f.exists():
        # A FIFO is the whole transport: `send` is then a plain file write, so any language or shell script
        # can drive the pad. (A Windows edition needs a named pipe or a socket here instead.)
        os.mkfifo(f, 0o600)
    print(f'pad: {dev.name} device {path} ready; commands on {f}', flush=True)
    script = Script(dev)
    try:
        while True:
            with open(f) as fh:      # blocks until a writer opens it; one batch per open
                for line in fh:
                    script.line(line.rstrip('\n'))
    except KeyboardInterrupt:
        return 0
    finally:
        dev.close()
        lock.release()


def send(cmds: list[str], quiet: bool = False) -> bool:
    """Write command lines to the running server. False (or exit) when nothing is serving."""
    f = fifo()
    if not f.exists():
        msg = (f'pad: no server on {f} — start one:  ./kit pad serve &'
               + ('  (KIT_PCSX2=%s for this copy)' % plat.instance() if plat.instance() else ''))
        if quiet:
            print(msg, file=sys.stderr)
            return False
        raise SystemExit(msg)
    try:
        # O_NONBLOCK first: opening a FIFO for writing blocks forever if the server died without removing it.
        fd = os.open(f, os.O_WRONLY | os.O_NONBLOCK)
    except OSError:
        msg = f'pad: {f} exists but nobody is reading it (a dead server?) — restart:  ./kit pad serve &'
        if quiet:
            print(msg, file=sys.stderr)
            return False
        raise SystemExit(msg)
    with os.fdopen(fd, 'w') as fh:
        fh.write('\n'.join(cmds) + '\n')
    return True


# --- CLI ---------------------------------------------------------------------

DOC = """virtual controller: serve one, then send button and stick commands to it.

  ./kit pad serve &                      create the device (keep running; KIT_PCSX2=N for copy N)
  ./kit pad send "press start 100"
  ./kit pad send "stick l 0 -1" "sleep 400" "release"
  ./kit pad send < moves.txt             or:  ./kit pad script moves.txt
  ./kit pad check                        can this machine make a pad, and is one serving?

Commands: press <btn> <ms> | down <btn> | up <btn> | dpad <dir> <ms> | stick <l|r> <x> <y> |
          sleep <ms> | release | # comment        (`8f` = 8 frames instead of 8 ms)
Buttons:  cross circle square triangle start l1 r1 l2 r2 l3 r3 up down left right guide  (select is refused)
Timing is wall-clock, not frame-exact."""


def cmd_pad(argv: list[str]) -> int | None:
    from ps2kit.cli import parser
    ap = parser('pad', DOC)
    ap.add_argument('cmd', nargs='?', default='check', choices=['serve', 'send', 'script', 'check'])
    ap.add_argument('args', nargs='*', help='commands for `send`, a file for `script`')
    a = ap.parse_args(argv)

    if a.cmd == 'serve':
        return serve()
    if a.cmd == 'check':
        ok, why = plat.pad_available()
        print(f'backend  {plat.pad_backend()}  {"ok" if ok else "UNUSABLE: " + why}')
        f = fifo()
        print(f'server   {f}  {"serving" if f.exists() else "not running (./kit pad serve &)"}')
        return 0 if ok else 1
    if a.cmd == 'script':
        if not a.args:
            raise SystemExit('./kit pad script FILE  (one command per line)')
        cmds = Path(a.args[0]).read_text().splitlines()
    else:
        cmds = a.args or [l for l in sys.stdin.read().splitlines()]
    cmds = [c for c in cmds if c.strip()]
    if not cmds:
        raise SystemExit('./kit pad send "press cross 100" ...   (or feed commands on stdin)')
    send(cmds)
    return None
