#!/usr/bin/env python3
"""The window itself, under Xvfb, typed into with xdotool.

    xvfb-run -a -s '-screen 0 1280x900x24' python3 -I tests/gui_test.py [bash|zsh]

Needs xdotool. Reads the state The Textsmith writes to TEXTSMITH_TEST_DUMP.
"""
import json, os, re, shutil, subprocess, sys, tempfile, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, 'target', 'release', 'textsmith')
SHELL = sys.argv[1] if len(sys.argv) > 1 else 'bash'
failures = 0


def check(name, got, want):
    global failures
    ok = want(got) if callable(want) else got == want
    print(('ok    ' if ok else 'FAIL  ') + name + ('' if ok else f'\n      got:  {got!r}\n      want: {want!r}'))
    if not ok:
        failures += 1


def xdo(*a):
    return subprocess.run(['xdotool', *a], capture_output=True, text=True).stdout.strip()


home = tempfile.mkdtemp(prefix='ts-gui-')
dump = os.path.join(home, 'state.json')
os.makedirs(os.path.join(home, '.textsmith'))
open(os.path.join(home, '.textsmith', 'bookmarks'), 'w').write(
    '# my bookmarks\n#|count: wc -l\n#hello: echo hello <who:world>\n#burn: echo dd if=<iso> of=<disk:/dev/sdb>\n#install: echo apt install -y <..>\n#update: [:forge "echo updating forge", :* "echo updating here"]\n#both: [:forge "echo one -y && echo two -y"]\n')
# A pretend ssh: a plain shell without The Textsmith's integration, as on a server.
os.makedirs(os.path.join(home, 'bin'))
# exec -a: the process shows as `ssh ... forge`, as a real ssh would.
open(os.path.join(home, 'bin', 'ssh'), 'w').write("#!/bin/bash\nPS1='remote$ ' exec -a ssh bash --norc --noprofile -i -s \"$@\"\n")
os.chmod(os.path.join(home, 'bin', 'ssh'), 0o755)
json.dump({'title': {'format': '{prompt} — {shell} — {app}', 'running': '{command} — {shell} — {app}'}, 'display': {'font': 'Monospace 11'}, 'review': {'speakJumps': True, 'readUpTo': 10}, 'keys': {'prevFailed': 'Alt+Shift+Up', 'nextFailed': 'Ctrl+Alt+J'}, 'meta': {'version': 1}},
          open(os.path.join(home, '.textsmith', 'gtk-prefs.json'), 'w'))
open(os.path.join(home, '.bashrc'), 'w').write("PS1='\\u@\\h:\\w\\$ '\nalias vps='ssh forge'\n")
open(os.path.join(home, '.zshrc'), 'w').write("PS1='%n@%m:%~%# '\nalias vps='ssh forge'\n")
env = dict(os.environ, PATH=os.path.join(home, 'bin') + ':' + os.environ['PATH'], HOME=home, TEXTSMITH_CONFIG=os.path.join(home, '.textsmith'), TEXTSMITH_TEST_DUMP=dump,
           TEXTSMITH_SHELL='/usr/bin/' + SHELL, LANG='C.UTF-8', NO_AT_BRIDGE='1')
env.pop('XDG_RUNTIME_DIR', None)
proc = subprocess.Popen([BIN, '--foreground', '--working-directory', home], env=env)


def state():
    try:
        return json.load(open(dump))
    except Exception:
        return {}


def wait_for(what, cond, timeout=8):
    end = time.time() + timeout
    while time.time() < end:
        s = state()
        try:
            if cond(s):
                return s
        except Exception:
            pass
        time.sleep(0.1)
    print(f'      (timed out waiting for {what}; state: {json.dumps(state())[:900]})')
    return state()


def typ(text):
    xdo('type', '--delay', '15', text)


def key(*k):
    for one in k:
        xdo('key', '--delay', '40', one)
        time.sleep(0.08)


def run(line, keyname='Return'):
    n = len(state().get('blocks', []))
    typ(line)
    time.sleep(0.15)
    key(keyname)
    s = wait_for(line, lambda s: len(s['blocks']) > n and s['blocks'][-1]['status'] is not None and s['state'] == 'Prompt')
    return s['blocks'][-1] if s.get('blocks') else {}


try:
    s = wait_for('the window', lambda s: s.get('state') == 'Prompt')
    win = xdo('search', '--sync', '--name', 'The Textsmith')
    win = win.split('\n')[0]
    xdo('windowactivate', '--sync', win)
    xdo('windowfocus', '--sync', win)
    time.sleep(0.5)
    check('title is the directory, as in other terminals', s.get('title', ''), '~')

    b = run('echo hello')
    check('a command and its output', (b.get('command'), b.get('text'), b.get('status')), ('echo hello', 'hello', 0))
    b = run('ls /nonexistent-dir')
    check('a failing command: status and error text', (b.get('status'), 'nonexistent-dir' in b.get('text', '')), (2, True))
    b = run("printf 'progress 10%%\\rprogress 100%%\\n'")
    check('carriage-return rewrites settle into the final text', b.get('text'), 'progress 100%')
    b = run('seq 1 30')
    check('long output kept whole', b.get('text', '').split('\n'), [str(i) for i in range(1, 31)])
    b = run('stty size')
    rows, cols = (b.get('text') or '0 0').split()
    check('the pty has the terminal\'s size', int(cols) > 80 and int(rows) > 24, True)

    b = run('seq 5 #@count')
    check('Enter expands #@ (default bookmark file shared)', (b.get('command'), b.get('text')), ('seq 5 | wc -l', '5'))
    typ('seq 3 #@cou')
    key('Tab')
    time.sleep(0.6)
    b = run('', 'Return')
    check('Tab completes a #@ name in the window', (b.get('command'), b.get('text')), ('seq 3 | wc -l', '3'))
    b = run('hello', 'alt+Return')
    check('Alt+Enter calls a bookmark, default filled in', (b.get('command'), b.get('text')), ('echo hello world', 'hello world'))
    b = run('burn arch.iso', 'alt+Return')
    check('Alt+Enter: the dd example', b.get('text'), 'dd if=arch.iso of=/dev/sdb')
    b = run('burn arch.iso /dev/sdc', 'alt+Return')
    check('Alt+Enter: default overridden', b.get('text'), 'dd if=arch.iso of=/dev/sdc')

    # Alt+Shift+Enter fills in without running; Enter then runs it.
    n = len(state()['blocks'])
    typ('hello you')
    key('alt+shift+Return')
    time.sleep(0.6)
    check('Alt+Shift+Enter runs nothing', len(state()['blocks']), n)
    key('Return')
    s = wait_for('filled line run', lambda s: len(s['blocks']) > n and s['state'] == 'Prompt')
    check('…and Enter runs the filled-in line', s['blocks'][-1]['command'], 'echo hello you')

    # Shift+Enter: sudo in front.
    n = len(state()['blocks'])
    typ('true')
    key('shift+Return')
    s = wait_for('sudo line', lambda s: len(s['blocks']) > n)
    check('Shift+Enter puts sudo in front', s['blocks'][-1]['command'], 'sudo true')
    time.sleep(0.5)
    key('ctrl+c')
    wait_for('prompt after sudo', lambda s: s['state'] == 'Prompt')
    time.sleep(0.3)

    # Alt+M bookmarks the line.
    typ('#up: uptime')
    key('alt+m')
    time.sleep(0.8)
    check('Alt+M writes the bookmark to the shared file', '#up: uptime' in open(os.path.join(home, '.textsmith', 'bookmarks')).read(), True)

    # Typing at the prompt is not taken over: Alt+M while a program runs goes to the program.
    n = len(state()['blocks'])
    typ('cat -v')
    key('Return')
    wait_for('cat running', lambda s: s['state'] == 'Running')
    time.sleep(0.3)
    key('alt+m', 'Return', 'ctrl+d')
    s = wait_for('cat done', lambda s: s['state'] == 'Prompt' and len(s['blocks']) > n and s['blocks'][-1]['status'] is not None)
    check('keys go to a running program unchanged (Alt+M reaches cat)', s['blocks'][-1]['text'], lambda t: '^[m' in t)

    # F4: the review buffer.
    key('F4')
    s = wait_for('review', lambda s: s['review_open'])
    check('F4 opens the review, focused', (s['review_open'], s['focus']), (True, 'GtkTextView'))
    text = s['review_text']
    lines = text.split('\n')
    check('review holds commands and output', ('$ echo hello' in lines and 'hello' in lines), True)
    check('a failed command says so', any(l.startswith('$ ls /nonexistent-dir (failed, exit 2)') for l in lines), True)
    check('the cursor starts on the last command', lines[s['review_line']], lambda l: l.startswith('$ cat -v'))
    key('alt+Up')
    s = wait_for('jump back', lambda s: s['review_line'] != len(lines))
    check('Alt+Up: previous command', s['review_text'].split('\n')[s['review_line']], lambda l: l.startswith('$ '))
    key('alt+Prior')
    s = state()
    check('Alt+PageUp: previous failed command', s['review_text'].split('\n')[s['review_line']], lambda l: '(failed' in l or 'interrupted' in l)
    key('alt+Down')
    s = state()
    check('Alt+Down: next command', s['review_text'].split('\n')[s['review_line']], lambda l: l.startswith('$ '))
    key('Escape')
    s = wait_for('back to terminal', lambda s: not s['review_open'])
    check('Escape goes back to the terminal', (s['review_open'], s['focus']), (False, 'VteTerminal'))
    b = run('echo after review')
    check('typing works after the review', b.get('text'), 'after review')

    # Tabs.
    key('ctrl+shift+t')
    s = wait_for('new tab', lambda s: s['tabs'] == 2 and s['state'] == 'Prompt')
    check('Ctrl+Shift+T opens a tab, focused', (s['tabs'], s['current'], s['focus']), (2, 1, 'VteTerminal'))
    b = run('echo in tab two')
    check('the new tab has its own shell', (b.get('text'), len(state()['blocks'])), ('in tab two', 1))
    key('ctrl+Prior')
    s = wait_for('tab 1', lambda s: s['current'] == 0)
    check('Ctrl+PageUp goes back', s['current'], 0)
    key('ctrl+Next')
    wait_for('tab 2', lambda s: s['current'] == 1)
    typ('exit')
    key('Return')
    s = wait_for('tab closed', lambda s: s['tabs'] == 1)
    check('exit closes the tab', s['tabs'], 1)

    # About and the manual.
    key('ctrl+F1')
    time.sleep(0.8)
    check('Ctrl+F1 opens the About window', bool(xdo('search', '--name', '^About The Textsmith$')), True)
    key('Escape')
    time.sleep(0.5)
    check('Escape closes it', bool(xdo('search', '--name', '^About The Textsmith$')), False)
    key('F1')
    time.sleep(0.8)
    manual = os.path.join(os.environ.get('XDG_DATA_HOME') or os.path.join(home, '.local', 'share'), 'textsmith', 'manual.html')
    check('F1 writes the manual to open', os.path.exists(manual) and '<h2 id="bookmarks">Bookmarks</h2>' in open(manual).read(), True)

    # The bookmark list.
    key('alt+shift+b')
    time.sleep(0.8)
    dlg = xdo('search', '--name', '^Bookmarks$')
    check('Alt+Shift+B opens the bookmark list', bool(dlg), True)
    typ('hel')
    time.sleep(0.3)
    key('Return')
    s = wait_for('bookmark from list', lambda s: s['blocks'][-1]['command'] == 'echo hello world' and s['state'] == 'Prompt')
    check('typing filters the list; Enter runs the bookmark (defaults filled in)', s['blocks'][-1]['text'], 'hello world')
    # A <..> bookmark from the list waits for its arguments.
    key('alt+shift+b')
    time.sleep(0.8)
    typ('install')
    time.sleep(0.3)
    key('Return')
    time.sleep(0.6)
    b = run('vim git', 'Return')
    check('a <..> bookmark from the list takes the arguments typed after it', (b.get('command'), b.get('text')), ('echo apt install -y vim git', 'apt install -y vim git'))
    # A piped bookmark is added to the end of the line.
    typ('seq 4')
    key('alt+shift+b')
    time.sleep(0.8)
    typ('count')
    time.sleep(0.3)
    key('Return')
    time.sleep(0.5)
    b = run('', 'Return')
    check('a piped bookmark from the list is added to the line', (b.get('command'), b.get('text')), ('seq 4 | wc -l', '4'))

    # A command per machine: here, the fallback; inside ssh forge, forge's own,
    # typed in from the bookmark list without Enter.
    b = run('update', 'alt+Return')
    check('a bookmark with a command per machine runs this machine\'s', b.get('text'), 'updating here')
    n = len(state()['blocks'])
    typ('ssh forge')
    key('Return')
    wait_for('pretend ssh', lambda s: s['state'] == 'Running')
    time.sleep(0.8)
    key('alt+shift+b')
    time.sleep(0.8)
    typ('update')
    time.sleep(0.3)
    key('Return')
    s = wait_for('typed', lambda s: s['status'].startswith('Typed into ssh'))
    check('inside ssh the list types the command for that machine', s['status'], 'Typed into ssh: echo updating forge. Enter runs it.')
    time.sleep(0.3)
    key('Return')
    time.sleep(0.5)
    typ('exit')
    key('Return')
    s = wait_for('ssh done', lambda s: len(s['blocks']) > n and s['blocks'][-1]['status'] is not None and s['state'] == 'Prompt')
    check('…and Enter runs it there, once', s['blocks'][-1]['text'], lambda t: t.split('\n')[:2] == ['remote$ echo updating forge', 'updating forge'])
    # A command with && typed in whole, exactly once.
    typ('ssh forge')
    key('Return')
    wait_for('ssh again', lambda s: s['state'] == 'Running')
    time.sleep(0.8)
    key('alt+shift+b')
    time.sleep(0.8)
    typ('both')
    time.sleep(0.3)
    key('Return')
    time.sleep(0.6)
    key('Return')
    time.sleep(0.5)
    typ('exit')
    key('Return')
    s = wait_for('ssh done 2', lambda s: s['state'] == 'Prompt' and s['blocks'][-1]['status'] is not None)
    check('a command with && is typed once, as written', s['blocks'][-1]['text'].split('\n')[:3], ['remote$ echo one -y && echo two -y', 'one -y', 'two -y'])
    # Reached through an alias: the running program is still found to be ssh forge.
    typ('vps')
    key('Return')
    wait_for('alias ssh', lambda s: s['state'] == 'Running')
    time.sleep(0.8)
    key('alt+shift+b')
    time.sleep(0.8)
    typ('update')
    time.sleep(0.3)
    key('Return')
    s = wait_for('typed via alias', lambda s: s['status'].startswith('Typed into'))
    check('through an alias too, the command for forge comes first', s['status'], 'Typed into vps: echo updating forge. Enter runs it.')
    key('ctrl+u')
    # The other machine's row can be chosen: Down into the list, Down, Enter.
    key('alt+shift+b')
    time.sleep(0.8)
    typ('update')
    time.sleep(0.3)
    key('Down', 'Down', 'Return')   # into the list, then the next row
    s = wait_for('typed other', lambda s: 'updating here' in s['status'])
    check('the list has a row per machine to choose from', s['status'], 'Typed into vps: echo updating here. Enter runs it.')
    key('ctrl+u')
    typ('exit')
    key('Return')
    wait_for('alias ssh done', lambda s: s['state'] == 'Prompt')

    # A full-screen program comes and goes; its command still ends cleanly.
    n = len(state()['blocks'])
    typ("printf '\\e[?1049h\\e[2Jfull screen'; sleep 0.3; printf '\\e[?1049l'; echo back")
    key('Return')
    s = wait_for('alt screen', lambda s: len(s['blocks']) > n and s['blocks'][-1]['status'] is not None)
    check('output after a full-screen program', s['blocks'][-1]['text'], 'back')

    p = json.load(open(os.path.join(home, '.textsmith', 'gtk-prefs.json')))
    check('old settings move on: title to the directory, jumps not spoken; others kept', (p['title'], p['review'], p['display']['font'], p['keys']['prevFailed'], p['keys']['nextFailed'], p['meta']['version']), ({'format': '{dir}', 'running': '{dir}'}, {'command': '$ {command}{status}', 'speakJumps': False}, 'Monospace 11', 'Alt+PageUp', 'Ctrl+Alt+J', 4))
    subprocess.run(['import', '-window', 'root', os.path.join(ROOT, 'target', f'screenshot-{SHELL}.png')])
finally:
    proc.terminate()
    try:
        proc.wait(5)
    except Exception:
        proc.kill()
    shutil.rmtree(home, ignore_errors=True)
print(f'\n{SHELL}: ' + ('all passed' if not failures else f'{failures} failed'))
sys.exit(1 if failures else 0)
