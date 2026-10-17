#!/usr/bin/env python3
"""Start a PCSX2 input-recording playback over the desktop's accessibility bus.

    ./research/play_recording.py --tree                 # dump this copy's accessibility tree
    ./research/play_recording.py <a copy of a .p2m2>     # Tools -> Input Recording -> Play, then Open

No keystroke and no pointer event reaches PCSX2's window on this desktop (`plan/HUMAN.md`), but a Qt
application publishes its whole menu tree on AT-SPI, and a menu item can be *pressed* there with no
focus at all. That is the whole trick, and it is how S8 took the acceptance capture. The emulator has
to have been started with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` and `QT_ACCESSIBILITY=1` in its
environment, which `./kit pcsx2 start` passes through from this shell's.

Play a **copy** of a recording, never the human's own: PCSX2 switches to record mode when a movie ends
and keeps writing, into the same file. S8 restored it once already.

The two presses run as two child processes of this one on purpose. Pressing the menu item opens a modal
dialog, which blocks Qt's event loop, so the press never gets its D-Bus reply and the connection that
made it is finished; a fresh one finds the dialog and carries on.
"""
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

MENU = ('Tools', 'Input Recording', 'Play')


def atspi():
    import gi
    gi.require_version('Atspi', '2.0')
    from gi.repository import Atspi
    Atspi.init()
    return Atspi


def emu_pid() -> int:
    from ps2kit import emu
    if pid := emu.running():
        return pid
    raise SystemExit('PCSX2 is not running here: ./kit pcsx2 start')


def kids(acc) -> list:
    return [acc.get_child_at_index(i) for i in range(acc.get_child_count())]


def label(acc) -> str:
    """A Qt menu name as it reads without its mnemonic: '&Tools' is 'Tools'."""
    return (acc.get_name() or '').replace('&', '').strip()


def app_of(A, pid: int):
    """The desktop child that belongs to this emulator copy. By pid, because the other slots' copies
    are on the same bus under the same name."""
    for app in kids(A.get_desktop(0)):
        try:
            if app.get_process_id() == pid:
                return app
        except Exception:
            continue
    raise SystemExit(f'pid {pid} is not on the accessibility bus. Was PCSX2 started with '
                     f'QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 and QT_ACCESSIBILITY=1?')


def find(acc, want: str = '', roles: tuple = (), depth: int = 8) -> list:
    """Every node at or under `acc` whose name and role match, breadth first (so the shallowest menu
    wins over a duplicate deeper in a dialog)."""
    out, level = [], [acc]
    for _ in range(depth):
        nxt = []
        for node in level:
            try:
                name, role = label(node), node.get_role_name()
            except Exception:
                continue
            if (not want or name.lower() == want.lower()) and (not roles or role in roles):
                out.append(node)
            try:
                nxt += kids(node)
            except Exception:
                pass
        level = nxt
        if not level:
            break
    return out


def press(A, acc) -> None:
    """`do_action` the node's Press, whichever index it is on."""
    n = A.Action.get_n_actions(acc)
    for i in range(n):
        if (A.Action.get_action_name(acc, i) or '').lower() in ('press', 'click', 'open'):
            A.Action.do_action(acc, i)
            return
    if n:
        A.Action.do_action(acc, 0)
        return
    raise SystemExit(f'{label(acc)!r} ({acc.get_role_name()}) has no action to press')


def dump(node, prefix: str = '', depth: int = 0, limit: int = 7) -> None:
    try:
        role, name = node.get_role_name(), node.get_name()
    except Exception as e:
        print(f'{prefix}<unreadable: {e}>')
        return
    acts = ''
    try:
        acts = ','.join(A.Action.get_action_name(node, i)
                        for i in range(A.Action.get_n_actions(node)))
    except Exception:
        pass
    print(f'{prefix}{role}: {name!r}{"  [" + acts + "]" if acts else ""}')
    if depth >= limit:
        return
    for k in kids(node):
        dump(k, prefix + '  ', depth + 1, limit)


# --- the two phases ---------------------------------------------------------

def phase_menu() -> None:
    """Walk the menu bar down MENU and press the last item. Only the last: Qt publishes the whole menu
    tree whether or not it has ever been opened, so the ones on the way are only there to walk, and
    "pressing" one of them would be asking for an open popup to block the bus for no reason."""
    A = atspi()
    app = app_of(A, emu_pid())
    node = app
    for step in MENU:
        hits = [h for h in find(node, step, depth=6)
                if h.get_role_name() in ('menu', 'menu item', 'check menu item')]
        if not hits:
            raise SystemExit(f'no {step!r} under {label(node) or "the application"!r}'
                             f' — run with --tree to see what is there')
        node = hits[0]
        print(f'  {node.get_role_name()}: {label(node)!r}')
    press(A, node)


def phase_dialog(path: Path) -> None:
    """The file dialog the press opened: type the path in and press Open."""
    A = atspi()
    app = app_of(A, emu_pid())
    for _ in range(40):
        dialogs = find(app, roles=('dialog', 'file chooser'), depth=3)
        if dialogs:
            break
        time.sleep(0.25)
    else:
        raise SystemExit('no file dialog appeared')
    dlg = dialogs[0]
    print(f'  dialog: {dlg.get_name()!r}')

    fields = [f for f in find(dlg, roles=('text', 'entry'), depth=6)]
    if not fields:
        raise SystemExit('the dialog has no text field')
    A.EditableText.set_text_contents(fields[0], str(path))
    print(f'  typed: {path}')
    time.sleep(0.3)

    opens = [b for b in find(dlg, 'Open', roles=('push button', 'button'), depth=6)]
    if not opens:
        opens = [b for b in find(dlg, roles=('push button', 'button'), depth=6) if label(b) in ('OK', 'Choose')]
    if not opens:
        raise SystemExit('the dialog has no Open button')
    press(A, opens[0])
    print('  pressed Open')


def main() -> None:
    args = sys.argv[1:]
    if args[:1] == ['--tree']:
        global A
        A = atspi()
        dump(app_of(A, emu_pid()))
        return
    if args[:1] == ['--phase']:
        return phase_menu() if args[1] == 'menu' else phase_dialog(Path(args[2]))
    if len(args) != 1:
        raise SystemExit(__doc__.strip().splitlines()[2].strip())

    movie = Path(args[0]).resolve()
    if not movie.exists():
        raise SystemExit(f'no such recording: {movie}')
    if not os.access(movie, os.W_OK):
        raise SystemExit(f'{movie} is read-only. PCSX2 writes over a movie it plays: play a copy.')

    me = [sys.executable, __file__]
    print('pressing Tools -> Input Recording -> Play')
    # The press's own connection dies on the modal dialog (see the docstring), so a non-zero exit from
    # this phase is expected and not an error; the next phase says whether the dialog is really there.
    subprocess.run(me + ['--phase', 'menu'], timeout=60)
    print('filling in the file dialog')
    r = subprocess.run(me + ['--phase', 'dialog', str(movie)], timeout=60)
    raise SystemExit(r.returncode)


if __name__ == '__main__':
    main()
