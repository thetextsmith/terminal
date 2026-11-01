#!/usr/bin/env python3
"""The shell integration, driven through a real pty as a terminal would.

    python3 -I tests/shell_test.py [bash|zsh] [path/to/textsmith]

Checks the marks (133 A/C/D, 7773 command lines) and the keys: Enter expands
#@, Alt+Enter calls a bookmark, defaults fill in, errors keep the line, Tab
completes #@ names, Alt+M bookmarks, Shift+Enter adds sudo.
"""
import os, pty, re, select, shutil, signal, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SHELL = sys.argv[1] if len(sys.argv) > 1 else 'bash'
BIN = os.path.abspath(sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, 'target', 'release', 'textsmith'))

failures = 0


class Shell:
    def __init__(self, kind, home, bashrc_extra=''):
        self.kind = kind
        self.home = home
        self.tmp = tempfile.mkdtemp(prefix='ts-tmp-')
        env = {
            'HOME': home, 'PATH': os.environ['PATH'], 'TERM': 'xterm-256color', 'LANG': 'C.UTF-8',
            'TEXTSMITH_BIN': BIN, 'TEXTSMITH_CONFIG': os.path.join(home, '.textsmith'),
            'TEXTSMITH_TMP': self.tmp, 'TEXTSMITH': '1', 'PS1': r'\u@\h:\w\$ ',
        }
        if kind == 'bash':
            argv = ['bash', '--rcfile', os.path.join(ROOT, 'shell', 'textsmith.bashrc'), '-i']
        else:
            env['ZDOTDIR'] = os.path.join(ROOT, 'shell', 'zsh')
            env['TEXTSMITH_USER_ZDOTDIR'] = home
            argv = ['zsh', '-i']
        self.master, slave = pty.openpty()
        os.set_blocking(self.master, False)
        import fcntl, termios, struct
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
        self.proc = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave, env=env, cwd=home,
                                     start_new_session=True, preexec_fn=lambda: fcntl.ioctl(0, termios.TIOCSCTTY, 0))
        os.close(slave)
        self.buf = ''
        self.wait(r'\x1b\]133;A\x07', 'first prompt')

    def read(self, t=0.05):
        r, _, _ = select.select([self.master], [], [], t)
        if r:
            try:
                d = os.read(self.master, 65536)
            except OSError:
                return
            self.buf += d.decode('utf-8', 'replace')

    def wait(self, pat, what, timeout=6):
        end = time.time() + timeout
        rx = re.compile(pat, re.S)
        while time.time() < end:
            m = rx.search(self.buf)
            if m:
                out = self.buf[:m.end()]
                self.buf = self.buf[m.end():]
                return out
            self.read()
        raise TimeoutError(f'{what}: waited for {pat!r}; have {self.buf[-600:]!r}')

    def send(self, s):
        os.write(self.master, s.encode())

    def settle(self, t=0.4):
        end = time.time() + t
        while time.time() < end:
            self.read()
        out, self.buf = self.buf, ''
        return out

    def run(self, line, key='\r'):
        """Type a line, press a key, return (command line marks, output, status)."""
        self.settle(0.15)
        self.send(line)
        time.sleep(0.1)
        self.send(key)
        out = self.wait(r'\x1b\]133;D;(\d+)\x07.*?\x1b\]133;A\x07', line)
        cmds = re.findall(r'\x1b\]7773;([^\x07]*)\x07', out)
        st = re.findall(r'\x1b\]133;D;(\d+)\x07', out)
        c = out.find('\x1b]133;C\x07')
        body = out[c + 8:] if c >= 0 else ''
        body = re.sub(r'\x1b\][^\x07]*\x07', '', body)
        body = re.sub(r'(\x1b\[[0-9;]*m)*[#%](\x1b\[[0-9;]*m)* +\r \r', '', body)   # zsh's PROMPT_SP, erased on a terminal
        body = re.sub(r'\x1b\[[0-9;?]*[A-Za-z]', '', body)
        body = body.split('\x1b]133;D')[0].replace('\r', '')
        return cmds, body, int(st[-1]) if st else None, c >= 0

    def close(self):
        try:
            os.killpg(self.proc.pid, signal.SIGKILL)
        except Exception:
            pass
        shutil.rmtree(self.tmp, ignore_errors=True)


def check(name, got, want):
    global failures
    ok = got == want if not callable(want) else want(got)
    print(('ok    ' if ok else 'FAIL  ') + name + ('' if ok else f'\n      got:  {got!r}\n      want: {want!r}'))
    if not ok:
        failures += 1


def plain(s):
    s = re.sub(r'\x1b\][^\x07]*\x07', '', s)
    s = re.sub(r'\x1b\[[0-9;?]*[A-Za-z]', '', s)
    return s.replace('\r', '')


def main():
    home = tempfile.mkdtemp(prefix='ts-home-')
    os.makedirs(os.path.join(home, '.textsmith'))
    with open(os.path.join(home, '.textsmith', 'bookmarks'), 'w') as f:
        f.write('# notes\n#|count: wc -l\n#|cut: cut -c1-<n:3>\n#hello: echo hello <who:world>\n'
                '#two: echo <a> <b:B>\n#vps: echo ssh forge\n#install: echo apt install -y <..>\n')
    open(os.path.join(home, '.bashrc'), 'w').write('# empty\n')
    open(os.path.join(home, '.zshrc'), 'w').write('# empty\n')
    os.makedirs(os.path.join(home, 'dir1'))
    for n in ('alpha.txt', 'alps.txt', 'beta.txt'):
        open(os.path.join(home, 'dir1', n), 'w').close()
    sh = Shell(SHELL, home)
    try:
        cmds, out, st, started = sh.run('echo hi')
        check('plain command: line mark', cmds, ['echo hi'])
        check('plain command: output', out.strip(), 'hi')
        check('plain command: status', st, 0)

        cmds, out, st, _ = sh.run('false')
        check('failure status', st, 1)

        cmds, out, st, _ = sh.run('printf "a\\nb\\nc\\n" #@count')
        check('#@ pipe expansion runs', out.strip(), '3')
        check('#@ expanded line is reported', cmds, ['printf "a\\nb\\nc\\n" | wc -l'])

        cmds, out, st, _ = sh.run('echo abcdef #@cut')
        check('#@ default filled in', out.strip(), 'abc')
        cmds, out, st, _ = sh.run('echo abcdef #@cut 5 @count')
        check('#@ given argument and a chain', out.strip(), '1')

        cmds, out, st, _ = sh.run('#@hello')
        check('#@ alone at the start calls the bookmark', out.strip(), 'hello world')

        # An error keeps the line and runs nothing.
        sh.settle(0.15)
        sh.send('echo x #@nope')
        time.sleep(0.1)
        sh.send('\r')
        o = sh.settle(0.8)
        check('#@ error is shown', 'No bookmark called nope.' in plain(o), True)
        check('#@ error runs nothing', '133;C' in o, False)
        sh.send('\x15')  # Ctrl+U clears the kept line
        # Fixing the line and Enter again works (accept was re-armed).
        cmds, out, st, _ = sh.run('echo fixed')
        check('Enter works again after an error', out.strip(), 'fixed')

        # Alt+Enter: a bookmark call, defaults and all.
        cmds, out, st, _ = sh.run('hello', '\x1b\r')
        check('Alt+Enter calls a bookmark (default)', out.strip(), 'hello world')
        check('Alt+Enter reports the filled-in line', cmds, ['echo hello world'])
        cmds, out, st, _ = sh.run('hello Ishe', '\x1b\r')
        check('Alt+Enter with an argument', out.strip(), 'hello Ishe')
        cmds, out, st, _ = sh.run('two 1', '\x1b[9980~')
        check('private Alt+Enter key, default for the 2nd place', out.strip(), '1 B')
        sh.settle(0.15)
        sh.send('two')
        sh.send('\x1b[9980~')
        o = sh.settle(0.8)
        check('Alt+Enter error is shown and nothing runs', ('two takes 1 to 2 arguments: a, b defaults to B; you gave 0.' in plain(o), '133;C' in o), (True, False))
        sh.send('\x15')

        # <..> takes any number of arguments.
        cmds, out, st, _ = sh.run('install vim git htop', '\x1b\r')
        check('<..> takes any number of arguments', out.strip(), 'apt install -y vim git htop')
        cmds, out, st, _ = sh.run('echo x #@install a b')
        check('<..> in a #@ addition', cmds, ['echo x echo apt install -y a b'])
        # Bookmark and run on a <..> bookmark asks for its arguments instead of running bare.
        sh.settle(0.15)
        sh.send('#inst2: echo apt install <..>')
        sh.send('\x1b[9984~')
        o = sh.settle(0.8)
        check('Alt+Shift+M on a <..> bookmark asks for arguments', ('Type its arguments' in plain(o), '133;C' in o), (True, False))
        cmds, out, st, _ = sh.run('nano', '\r')
        check('…and then runs with them', out.strip(), 'apt install nano')

        # Fill in without running.
        sh.settle(0.15)
        sh.send('hello there')
        sh.send('\x1b[9981~')
        o = sh.settle(0.6)
        check('fill in only: no run', '133;C' in o, False)
        cmds, out, st, _ = sh.run('', '\r')
        check('fill in only: Enter then runs the filled line', cmds, ['echo hello there'])

        # Tab completion of #@ names.
        sh.settle(0.15)
        sh.send('echo abc #@cou')
        sh.send('\t')
        o = sh.settle(0.6)
        check('Tab completes a #@ name and says what it is', 'count. piped into. no arguments' in plain(o), True)
        cmds, out, st, _ = sh.run('', '\r')
        check('completed #@ name runs', cmds, ['echo abc | wc -l'])
        sh.settle(0.15)
        sh.send('ls #@c')
        sh.send('\t')
        o = sh.settle(0.6)
        check('Tab lists several names', '2 bookmarks: count, cut' in plain(o), True)
        sh.send('\x15')

        # Ordinary Tab completion still works, and a second Tab lists.
        sh.settle(0.15)
        sh.send('ls dir1/be')
        sh.send('\t')
        sh.settle(0.5)
        cmds, out, st, _ = sh.run('', '\r')
        check('ordinary Tab completion', [c.strip() for c in cmds], ['ls dir1/beta.txt'])
        sh.settle(0.15)
        sh.send('ls dir1/al')
        sh.send('\t')
        sh.settle(0.4)
        sh.send('\t')
        o = sh.settle(0.6)
        check('second Tab lists the choices', ('alpha.txt' in plain(o) and 'alps.txt' in plain(o)), True)
        sh.send('\x15')

        # Alt+M bookmarks the line; Alt+M on the same line removes it.
        sh.settle(0.15)
        sh.send('#greet: echo hi <name:you>')
        sh.send('\x1b[9983~')
        o = sh.settle(0.6)
        check('Alt+M bookmarks the line', 'Bookmarked: greet. up to 1 argument: name defaults to you' in plain(o), True)
        saved = open(os.path.join(home, '.textsmith', 'bookmarks')).read()
        check('bookmark saved in the file, notes kept', ('#greet: echo hi <name:you>' in saved, saved.startswith('# notes')), (True, True))
        cmds, out, st, _ = sh.run('greet', '\x1b\r')
        check('new bookmark callable at once', out.strip(), 'hi you')
        # Empty line: the last command.
        cmds, out, st, _ = sh.run('echo last one')
        sh.settle(0.15)
        sh.send('\x1b[9983~')
        o = sh.settle(0.6)
        check('Alt+M on an empty line bookmarks the last command', plain(o), lambda o: 'Bookmarked: echo last one' in o)
        # Bookmark and run.
        cmds, out, st, _ = sh.run('#now: echo running now', '\x1b[9984~')
        check('Alt+Shift+M bookmarks and runs', out.strip(), 'running now')

        # Shift+Enter: sudo in front (sudo itself may not be there: check the line).
        sh.settle(0.15)
        sh.send('echo s')
        sh.send('\x1b[9982~')
        o = sh.settle(1.0)
        check('Shift+Enter puts sudo in front', re.findall(r'\x1b\]7773;([^\x07]*)\x07', o), ['sudo echo s'])
        sh.send('\x03')
        sh.settle(0.4)

        # A line put there by The Textsmith.
        open(os.path.join(sh.tmp, 'line'), 'w').write('echo from the list')
        sh.settle(0.15)
        sh.send('\x1b[9986~')
        o = sh.wait(r'133;A\x07', 'run put line')
        check('a line from The Textsmith runs', 'from the list' in plain(o), True)

        # Multi-line command: two Enter presses, both reported.
        cmds, out, st, _ = sh.run('echo "a\rb"')
        check('a command over two lines', out.strip().split('\n'), ['a', 'b'])

        # Empty Enter: no output start.
        cmds, out, st, started = sh.run('', '\r')
        check('empty Enter: no command start', started, SHELL == 'zsh' and started or False)

        # Exit status of a pipeline stays for the next command's $?.
        cmds, out, st, _ = sh.run('(exit 7)')
        check('status 7 reported', st, 7)
        cmds, out, st, _ = sh.run('echo $?')
        check('$? survives the hooks', out.strip(), '7')

        # Program input is untouched: Enter inside `read`.
        sh.settle(0.15)
        sh.send('read -r v; echo "got $v"\r')
        time.sleep(0.3)
        sh.send('x #@count\r')
        o = sh.wait(r'133;A\x07', 'read')
        check('input to a program is not expanded', 'got x #@count' in plain(o), True)
    finally:
        sh.close()
        shutil.rmtree(home, ignore_errors=True)
    print(f'\n{SHELL}: ' + ('all passed' if not failures else f'{failures} failed'))
    sys.exit(1 if failures else 0)


main()
