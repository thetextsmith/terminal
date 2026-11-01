#!/usr/bin/python3.12
"""What Orca would hear: AT-SPI events from the window.

    dbus-run-session -- xvfb-run -a tests/a11y_run.sh

Run by tests/a11y_run.sh, which starts the accessibility bus first. Listens
for announcements, focus and caret events while xdotool presses keys.
"""
import json, os, shutil, subprocess, sys, tempfile, threading, time
import pyatspi
from gi.repository import GLib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, 'target', 'release', 'textsmith')
events = []
failures = 0


def check(name, ok, detail=''):
    global failures
    print(('ok    ' if ok else 'FAIL  ') + name + ('' if ok else f'\n      {detail}'))
    if not ok:
        failures += 1


def on_event(e):
    try:
        name = e.source.name if e.source else ''
        role = e.source.getRoleName() if e.source else ''
    except Exception:
        name, role = '', ''
    events.append((time.time(), e.type, role, name, e.any_data if isinstance(e.any_data, (str, int)) else str(e.any_data), e.detail1))


for t in ['object:announcement', 'object:state-changed:focused', 'object:text-caret-moved', 'window:activate']:
    pyatspi.Registry.registerEventListener(on_event, t)

home = tempfile.mkdtemp(prefix='ts-a11y-')
os.makedirs(os.path.join(home, '.textsmith'))
open(os.path.join(home, '.textsmith', 'bookmarks'), 'w').write('#|count: wc -l\n')
open(os.path.join(home, '.bashrc'), 'w').write("PS1='\\u@\\h:\\w\\$ '\n")
dump = os.path.join(home, 'state.json')
env = dict(os.environ, HOME=home, TEXTSMITH_CONFIG=os.path.join(home, '.textsmith'), TEXTSMITH_SHELL='/bin/bash',
           TEXTSMITH_TEST_DUMP=dump)
env.pop('NO_AT_BRIDGE', None)
proc = subprocess.Popen([BIN, '--foreground'], env=env)


def xdo(*a):
    return subprocess.run(['xdotool', *a], capture_output=True, text=True).stdout.strip()


def script():
    try:
        time.sleep(3)
        w = xdo('search', '--name', 'The Textsmith').split('\n')[0]
        xdo('windowfocus', '--sync', w)
        time.sleep(0.5)
        for line in ['echo one', 'ls /nope', 'echo three']:
            xdo('type', '--delay', '15', line)
            xdo('key', 'Return')
            time.sleep(0.6)
        mark = time.time()
        events.append((mark, 'MARK', 'F4', '', '', 0))
        xdo('key', 'F4')
        time.sleep(0.8)
        events.append((time.time(), 'MARK', 'alt+Up', '', '', 0))
        xdo('key', 'alt+Up')
        time.sleep(0.8)
        events.append((time.time(), 'MARK', 'alt+Prior', '', '', 0))
        xdo('key', 'alt+Prior')
        time.sleep(0.8)
        events.append((time.time(), 'MARK', 'Down', '', '', 0))
        xdo('key', 'Down')
        time.sleep(0.8)
        events.append((time.time(), 'MARK', 'Escape', '', '', 0))
        xdo('key', 'Escape')
        time.sleep(0.8)
        events.append((time.time(), 'MARK', 'ctrl+shift+t', '', '', 0))
        xdo('key', 'ctrl+shift+t')
        time.sleep(1.5)
        events.append((time.time(), 'MARK', 'ctrl+Prior', '', '', 0))
        xdo('key', 'ctrl+Prior')
        time.sleep(1.0)
    finally:
        GLib.idle_add(pyatspi.Registry.stop)


threading.Thread(target=script, daemon=True).start()
pyatspi.Registry.start()

# The tree: the terminal and the review text.
app = None
for a in pyatspi.Registry.getDesktop(0):
    if a and a.name in ('textsmith', 'The Textsmith'):
        app = a
roles = []
def walk(o, d=0):
    try:
        roles.append((o.getRoleName(), o.name))
        for c in o:
            walk(c, d + 1)
    except Exception:
        pass
if app:
    walk(app)
proc.terminate()
proc.wait(5)
shutil.rmtree(home, ignore_errors=True)

for e in events:
    print('   ', f'{e[1]:<36} {e[2]:<14} {e[3][:30]!r:<32} {str(e[4])[:70]!r}')
print()
check('the application is on the accessibility bus', app is not None)
check('the terminal is exposed with the terminal role', ('terminal', '') in roles or any(r == 'terminal' for r, _ in roles), roles[:20])


def after(label):
    i = next(k for k, e in enumerate(events) if e[1] == 'MARK' and e[2] == label)
    out = []
    for e in events[i + 1:]:
        if e[1] == 'MARK':
            break
        out.append(e)
    return out

ann = lambda lab: [e[4] for e in after(lab) if e[1] == 'object:announcement']
check('F4: focus moves to the text area named Review', any(e[1].startswith('object:state-changed:focused') and e[3] == 'Review' and e[5] == 1 for e in after('F4')), after('F4'))
check('F4: no announcement on top of the focus change', ann('F4') == [], ann('F4'))
check('Alt+Up: the caret moves (Orca reads the line), nothing announced', (any(e[1] == 'object:text-caret-moved' for e in after('alt+Up')), ann('alt+Up')), (True, []))
check('Alt+PageUp: no earlier failure is announced', any('No earlier command failed' in a for a in ann('alt+Prior')), ann('alt+Prior'))
check('Down in the review moves the caret (Orca reads the line)', any(e[1] == 'object:text-caret-moved' for e in after('Down')), after('Down'))
check('Escape: focus back on the terminal', any(e[1].startswith('object:state-changed:focused') and e[2] == 'terminal' and e[5] == 1 for e in after('Escape')), after('Escape'))
check('Ctrl+PageUp: focus moves to the other terminal, nothing announced', (any(e[1].startswith('object:state-changed:focused') for e in after('ctrl+Prior')), ann('ctrl+Prior')), (True, []))
print('\n' + ('all passed' if not failures else f'{failures} failed'))
sys.exit(1 if failures else 0)
