#!/usr/bin/env python3
"""Exported functions do what The Textsmith does with the same call.

    python3 -I tests/export_test.py

For each call, the command The Textsmith fills in (`bm call`) is run, and so
is the exported function, in bash and in zsh; their output (and errors) must
match. printf '[%s]' shows exactly which words a program receives.
"""
import os, shutil, subprocess, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, 'target', 'release', 'textsmith')
P = "printf '[%s]'"
MARKS = f"""# notes
#site: {P} -rlt <source> forge:/var/www/<destination>
#notes: {P} '*<>*'
#say: {P} "hello <who> there"
#burn: {P} if=<iso> of=<disk:/dev/sdb>
#serve: {P} <port:8000> --bind <addr:127.0.0.1>
#g: {P} <pat:'TODO|FIXME'> <dir:.>
#install: {P} install -y <..>
#cpto: {P} <..> to <dst> and <dst>
#lint: {P} clippy <opts..:--all-targets>
#copy(2..): {P}
#l(..1): {P} ls
#up: {P} -c 3 <host:forge> <:-q>
#|count: wc -l
#pk: {P} <pkgs..> again <pkgs..>
#x: {P} <a> <..>
#mix: {P} "in dq <a>" 'in sq <b>' <c>
#quiet: 2>/dev/null
#ls: {P} ls-is-a-program
#send: {P} sync <from> drive:Library/<to:<from>> --progress
#bk: {P} <f> <to:<f>.bak>
#dd2: {P} <a:x> <b:<a>-y>
#rs: {P} <src> <..:<src>>
#update: [:anvil "{P} apt update", :brainpower :hammer '{P} pacman -Syu']
#get: [:anvil "{P} apt install <..>", :* "{P} 'no apt: <..>'"]
#mk(1..2): [:anvil "{P} dd if=<iso> of=<disk:/dev/sdb>", :brainpower "{P} \\"dd\\" if=<iso>"]
"""
CALLS = [
    'site a.txt html/', 'site "my file.txt" "sub dir/"', 'notes artificial', "notes 'two words'", 'say Ishe',
    'burn arch.iso', 'burn arch.iso /dev/sdc', 'serve', 'serve 9000', 'serve 9000 0.0.0.0', 'g', 'g fixme src',
    'install', 'install vim', 'install vim git "a b"', 'cpto x/', 'cpto a b c x/', 'lint', 'lint -q --tests',
    'copy a b', 'copy a b c', 'l', 'l /tmp', 'up', 'up vps', 'up vps -v', 'pk a b', 'x 1', 'x 1 2 3', 'mix 1 2 3',
    # mistakes: the same sentence from both
    'site a', 'site a b c', 'burn', 'copy a', 'l a b', 'x',
    'send Music', 'send Music Songs', 'send "My Books"', 'bk a.txt', 'bk a.txt b', 'dd2', 'dd2 q', 'dd2 q r', 'rs a', 'rs a b c', 'send',
]
# Where the function does better: a quoted argument with spaces put inside a
# quoted place. Pasted in as text, its quotes end the place's quotes and it
# falls apart into two words; the function keeps it one.
BETTER = {"notes 'two words'": '[*two words*]'}
MACHINE_CALLS = ['update', 'update x', 'get vim git', 'get', 'mk a.iso', 'mk a.iso /dev/sdc', 'mk']
failures = 0


def run(shell, script):
    r = subprocess.run([shell, '-c', script], capture_output=True, text=True, timeout=20)
    return r.stdout, r.stderr.strip(), r.returncode


def main():
    global failures
    home = tempfile.mkdtemp()
    cfg = os.path.join(home, '.textsmith')
    os.makedirs(cfg)
    open(os.path.join(cfg, 'bookmarks'), 'w').write(MARKS)
    env = dict(os.environ, HOME=home, TEXTSMITH_CONFIG=cfg)
    out = os.path.join(home, 'bookmarks.sh')
    r = subprocess.run([BIN, 'bm', 'export', '-o', out], env=env, capture_output=True, text=True)
    print(r.stderr.strip().replace('\n', '\n   ') and '   ' + r.stderr.strip().replace('\n', '\n   '))
    warn = r.stderr
    ok = 'quiet: left out' in warn and 'ls: left out; there is already a program' in warn and 'Wrote' in warn
    print(('ok    ' if ok else 'FAIL  ') + 'additions are left out, and so are names that would hide a program')
    failures += not ok
    # The tests below need every function: exported with --keep-clashes.
    subprocess.run([BIN, 'bm', 'export', '--keep-clashes', '-o', out], env=env, capture_output=True)
    r = subprocess.run([BIN, 'bm', 'export', '--prefix', 'bm_'], env=env, capture_output=True, text=True)
    ok = 'bm_install() {' in r.stdout and 'bm_ls() {' in r.stdout and 'left out; there is already' not in r.stderr
    print(('ok    ' if ok else 'FAIL  ') + '--prefix gives every function a name of its own')
    failures += not ok
    for shell in ('bash', 'zsh'):
        bad = []
        for call in CALLS:
            name = call.split()[0]
            r = subprocess.run([BIN, 'bm', 'call', call], env=env, capture_output=True, text=True)
            if r.returncode == 0:
                want = run(shell, r.stdout.rstrip('\n'))
                got = run(shell, f'. {out}; {call}')
                if call in BETTER:
                    if got[0] != BETTER[call]:
                        bad.append(f'{call!r}: function {got[0]!r}, want {BETTER[call]!r}')
                elif (got[0], got[2]) != (want[0], want[2]):
                    bad.append(f'{call!r}: function {got[0]!r} ({got[2]}), The Textsmith {want[0]!r} ({want[2]})')
            else:
                got = run(shell, f'. {out}; {call}')
                if got[1] != r.stdout.strip() or got[2] != 2:
                    bad.append(f'{call!r}: function said {got[1]!r} ({got[2]}), The Textsmith {r.stdout.strip()!r}')
        # A command per machine: the same choice, and the same sentence when there is none.
        for m in ('anvil', 'brainpower', 'hammer', 'elsewhere'):
            menv = dict(env, TEXTSMITH_MACHINE=m)
            for call in MACHINE_CALLS:
                r = subprocess.run([BIN, 'bm', 'call', call], env=menv, capture_output=True, text=True)
                g = subprocess.run([shell, '-c', f'. {out}; {call}'], env=menv, capture_output=True, text=True)
                if r.returncode == 0:
                    w = subprocess.run([shell, '-c', r.stdout.rstrip()], env=menv, capture_output=True, text=True)
                    if (g.stdout, g.returncode) != (w.stdout, w.returncode):
                        bad.append(f'{m}: {call!r}: function {g.stdout!r} ({g.returncode}), The Textsmith {w.stdout!r}')
                elif g.stderr.strip() != r.stdout.strip() or g.returncode != 2:
                    bad.append(f'{m}: {call!r}: function said {g.stderr.strip()!r}, The Textsmith {r.stdout.strip()!r}')
        piped = run(shell, f". {out}; printf 'a\\nb\\n' | count")[0].strip()
        if piped != '2':
            bad.append(f'piped count gave {piped!r}')
        print(('ok    ' if not bad else 'FAIL  ') + f'{shell}: {len(CALLS) + 1 + 4 * len(MACHINE_CALLS)} calls behave the same, on four machines' + ''.join('\n      ' + b for b in bad))
        failures += bool(bad)
    shutil.rmtree(home, ignore_errors=True)
    print('\n' + ('all passed' if not failures else f'{failures} failed'))
    sys.exit(1 if failures else 0)


main()
