"""Press a PCSX2 menu item over AT-SPI. usage: press.py PID 'Tools/Input Recording/Play'"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import atspi_lib as A
from gi.repository import Atspi

app = A.app_of(int(sys.argv[1]))
item = A.menu_path(app, sys.argv[2].split('/'))
try:
    print('checked before', item.get_state_set().contains(Atspi.StateType.CHECKED))
except Exception:
    pass
print(A.press(item))
