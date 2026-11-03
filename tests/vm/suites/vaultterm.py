#!/usr/bin/python3
"""vaultterm.py TIMEOUT [EXPECT SEND]... -- COMMAND [ARG...]

term.py, with one more thing a SEND may be: @recovery, which is replaced by the recovery key the Vault showed since the last
step (eleven groups of five), as a person types back what they wrote down. The Vault prints it once,
and the suite reads it from what this prints at the end.

Runs COMMAND on a terminal of its own, 120 columns by 40 lines, as a person at a terminal would, for what asks
a question (gum, a typed confirmation, a check of [[ -t 0 ]]). For each EXPECT and SEND, in turn, it waits until
the regular expression EXPECT shows in what COMMAND printed since the last match, colours and cursor movement
left out, then types SEND as it is. Then it waits for COMMAND to end. Each wait lasts TIMEOUT seconds at most.
An EXPECT that starts with ? is a question COMMAND may not ask: if it ends first, the steps after it are left.

It prints everything COMMAND showed, without colours, and exits with COMMAND's status, or 124 when something
expected did not show or COMMAND did not end in time (it is killed then). It answers none of the questions a
program asks the terminal (the background colour): gum takes such an answer as typed, and goes on without one.
"""
import os
import pty
import re
import select
import signal
import struct
import sys
import time

RECOVERY = re.compile(r"(?:[A-Z0-9]{5}-){10}[A-Z0-9]{5}")

CODES = re.compile(r'\x1b\[[0-9;?<=>]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][0-9A-Za-z]|\x1b[=>78NOM]|\r')


def main():
    timeout = float(sys.argv[1])
    split = sys.argv.index('--')
    steps = sys.argv[2:split]
    command = sys.argv[split + 1:]
    if len(steps) % 2 or not command:
        sys.exit(__doc__)
    pid, fd = pty.fork()
    if pid == 0:
        import fcntl
        import termios
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
        os.environ['TERM'] = 'xterm-256color'
        os.execvp(command[0], command)
    shown = b''
    ended = False

    # What COMMAND printed, until it ends or SECONDS pass
    def read(seconds):
        nonlocal shown, ended
        ready, _, _ = select.select([fd], [], [], seconds)
        if not ready:
            return
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            chunk = b''
        if not chunk:
            ended = True
            return
        shown += chunk

    def text():
        return CODES.sub('', shown.decode('utf-8', 'replace'))

    status = None
    searched = 0
    for expect, send in zip(steps[0::2], steps[1::2]):
        deadline = time.monotonic() + timeout
        optional = expect.startswith('?')
        pattern = re.compile(expect[1:] if optional else expect)
        while not (match := pattern.search(text(), searched)):
            if ended and optional:
                break
            if ended or time.monotonic() > deadline:
                print(f'term.py: {expect!r} did not show within {timeout:.0f} s', file=sys.stderr)
                status = 124
                break
            read(0.5)
        if status or match is None:
            break
        searched = match.end()
        if send.startswith("@recovery"):
            shown_keys = RECOVERY.findall(text())
            if not shown_keys:
                print("vaultterm.py: no recovery key was shown", file=sys.stderr)
                status = 124
                break
            send = shown_keys[-1] + send[len("@recovery"):]
        # The question drawn whole before the answer
        time.sleep(0.5)
        os.write(fd, send.encode())
    if status is None:
        deadline = time.monotonic() + timeout
        while not ended and time.monotonic() < deadline:
            read(0.5)
        if not ended:
            print(f'term.py: the command did not end within {timeout:.0f} s', file=sys.stderr)
            status = 124
    if status == 124:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    else:
        status = os.waitstatus_to_exitcode(os.waitpid(pid, 0)[1])
    print(text())
    sys.exit(status if status >= 0 else 128 - status)


main()
