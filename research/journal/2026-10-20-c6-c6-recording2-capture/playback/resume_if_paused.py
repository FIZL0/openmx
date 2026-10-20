"""Press System -> Pause only if it is checked (the VM is paused). No PINE."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import atspi_lib as A
from gi.repository import Atspi

app = A.app_of(int(sys.argv[1]))
p = A.menu_path(app, ['System', 'Pause'])
checked = p.get_state_set().contains(Atspi.StateType.CHECKED)
print('paused' if checked else 'running', '->', A.press(p) if checked else 'left alone', flush=True)
