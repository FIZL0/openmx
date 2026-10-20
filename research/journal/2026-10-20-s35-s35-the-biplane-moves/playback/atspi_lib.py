"""Small helpers for driving PCSX2's Qt GUI over AT-SPI."""
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi


def app_of(pid):
    desk = Atspi.get_desktop(0)
    for i in range(desk.get_child_count()):
        app = desk.get_child_at_index(i)
        try:
            if app.get_process_id() == pid:
                return app
        except Exception:
            pass
    raise SystemExit(f'no accessible app for pid {pid}')


def find(node, pred, depth=12):
    """Depth-first search for the first node with pred(node)."""
    try:
        if pred(node):
            return node
    except Exception:
        pass
    if depth == 0:
        return None
    try:
        n = node.get_child_count()
    except Exception:
        return None
    for i in range(n):
        try:
            hit = find(node.get_child_at_index(i), pred, depth - 1)
        except Exception:
            hit = None
        if hit is not None:
            return hit
    return None


def menu_path(app, names):
    node = app
    for name in names:
        node = find(node, lambda x, name=name: x.get_role_name() == 'menu item' and x.get_name() == name)
        if node is None:
            raise SystemExit(f'no menu item {name!r} on the way to {names}')
    return node


def press(node):
    try:
        node.do_action(0)
        return 'pressed'
    except Exception as e:
        return f'pressed (reply: {e})'
