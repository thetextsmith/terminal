#!/usr/bin/python3.12
"""Enter must not make the terminal report the prompt line as new text:
Orca would read it before the output.

    dbus-run-session -- xvfb-run -a sh tests/a11y_run.sh tests/echo_test.py [bash|zsh]
"""
import os, subprocess, sys, tempfile, threading, time
import pyatspi
from gi.repository import GLib
BIN = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'target', 'release', 'textsmith')
SHELL = '/usr/bin/zsh' if (sys.argv[1:] or ['bash'])[0] == 'zsh' else '/bin/bash'
events = []
def on(e):
    try:
        if e.source and e.source.getRoleName() == 'terminal':
            events.append((e.type, e.detail1, e.detail2, str(e.any_data)))
    except Exception: pass
pyatspi.Registry.registerEventListener(on, 'object:text-changed')
pyatspi.Registry.registerEventListener(lambda e: events.append(('object:announcement', 0, 0, str(e.any_data))), 'object:announcement')
home = tempfile.mkdtemp(); os.makedirs(home + '/.textsmith'); os.makedirs(home + '/Projects/alpha'); os.makedirs(home + '/Projects/beta'); [open(home + '/Projects/' + z, 'w').close() for z in ('textsmith-gtk-0.5.1.zip', 'textsmith-gtk-0.6.0.zip', 'textsmith-electron-0.8.0.zip')]
open(home + '/.bashrc', 'w').write("PS1='\\[\\e]0;\\u@\\h: \\w\\a\\]\\u@brainpower:\\w\\$ '\nalias l='ls -CF'\n")
open(home + '/.zshrc', 'w').write("PS1='%n@brainpower:%~%# '\nalias l='ls -CF'\n")
env = dict(os.environ, HOME=home, TEXTSMITH_CONFIG=home + '/.textsmith', TEXTSMITH_SHELL=SHELL)
env.pop('NO_AT_BRIDGE', None)
p = subprocess.Popen([BIN, '--foreground', '--working-directory', home + '/Projects'], env=env)
def xdo(*a): return subprocess.run(['xdotool', *a], capture_output=True, text=True).stdout.strip()
def script():
    time.sleep(3)
    w = xdo('search', '--name', 'Projects').split('\n')[0]; xdo('windowfocus', '--sync', w); time.sleep(0.5)
    xdo('type', '--delay', '30', 'l'); time.sleep(0.5)
    events.append(('MARK', 'Enter', 0, ''))
    xdo('key', 'Return'); time.sleep(1.5)
    xdo('type', '--delay', '30', 'unzip tex'); time.sleep(0.6)
    if SHELL.endswith('bash'):   # Tab cycles through the choices (editing.tabCompletion)
        for k in ['Tab', 'Tab', 'Tab', 'Tab', 'shift+Tab']:
            events.append(('MARK', 'c-' + k, 0, '')); xdo('key', k); time.sleep(0.7)
        events.append(('MARK', 'msg', 0, '')); xdo('key', 'ctrl+u'); xdo('key', 'ctrl+shift+c'); time.sleep(1.0)
        events.append(('MARK', 'end', 0, ''))
        GLib.idle_add(pyatspi.Registry.stop); return
    events.append(('MARK', 'Tab', 0, ''))
    xdo('key', 'Tab'); time.sleep(1.0)
    events.append(('MARK', 'Tab2', 0, ''))
    xdo('key', 'Tab'); time.sleep(0.6)
    if SHELL.endswith('bash'):   # bash lists on the Tab after one that added nothing
        xdo('key', 'Tab'); time.sleep(0.8)
    xdo('type', '--delay', '30', 'g'); time.sleep(0.4)
    events.append(('MARK', 'Tab3', 0, ''))
    xdo('key', 'Tab'); time.sleep(1.0)
    events.append(('MARK', 'end', 0, ''))
    GLib.idle_add(pyatspi.Registry.stop)
threading.Thread(target=script, daemon=True).start()
pyatspi.Registry.start()
p.terminate()
def between(a, b):
    i = next(k for k, e in enumerate(events) if e[0] == 'MARK' and e[1] == a)
    j = next(k for k, e in enumerate(events) if e[0] == 'MARK' and e[1] == b)
    return ''.join(e[3] for e in events[i:j] if e[0] == 'object:text-changed:insert')
bad = 0
def marks_seq():
    return [e[1] for e in events if e[0] == 'MARK']
if SHELL.endswith('bash'):
    m = marks_seq()
    def seg(k):
        a = [i for i, e in enumerate(events) if e[0] == 'MARK'][k]
        b = [i for i, e in enumerate(events) if e[0] == 'MARK'][k + 1]
        return events[a + 1:b]
    first = [i for i, x in enumerate(m) if x.startswith('c-')][0]
    said = [[e[3] for e in seg(first + n) if e[0] == 'object:announcement'] for n in range(5)]
    for what, ok in [
        ('cycling: the first Tab puts in the first choice, Orca reads it as typed', said[0] == []),
        ('cycling: each next Tab says the whole choice', said[1:3] == [['textsmith-gtk-0.5.1.zip'], ['textsmith-gtk-0.6.0.zip']]),
        ('cycling: Shift+Tab goes back and says it', said[4] == ['textsmith-gtk-0.6.0.zip']),
        ('a message on the status line does not make the terminal report its text again',
         not any('brainpower' in e[3] for e in seg(m.index('msg')) if e[0].startswith('object:text-changed'))),
    ]:
        bad += not ok
        print(('ok    ' if ok else 'FAIL  ') + f'bash: {what}' + ('' if ok else f'\n      {said}'))
    checks = [('after Enter only the output is new text', 'Enter', 'c-Tab', lambda t: t.startswith('alpha/'))]
else:
    checks = [('after Enter only the output is new text', 'Enter', 'Tab', lambda t: t.startswith('textsmith') or t.startswith('alpha/')),
                         ('Tab completes the common part only', 'Tab', 'Tab2', lambda t: t == 'tsmith-'),
                         ('a second Tab lists the choices', 'Tab2', 'Tab3', lambda t: 'textsmith-electron-0.8.0.zip' in t),
                         ('Tab after one more letter adds just the new part', 'Tab3', 'end', lambda t: t.strip() == 'tk-0.')]
for what, a, b, want in checks:
    got = between(a, b)
    first = got.split('\n')[0]
    ok = want(got) and (a == 'Tab2' or 'brainpower' not in first)
    bad += not ok
    print(('ok    ' if ok else 'FAIL  ') + f'{os.path.basename(SHELL)}: {what}' + ('' if ok else f'\n      got: {got!r}'))
if os.environ.get('SHOW'):
    for e in events: print('   ', e)
sys.exit(1 if bad else 0)
