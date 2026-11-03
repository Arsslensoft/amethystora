#!/usr/bin/python3
"""vaultsock.py [--socket PATH] [--time] [--hold SECONDS] LINE...: each LINE (a request of the Vault's wire format, one line of
JSON) on a connection of its own to the service's socket, as a program of the account that runs this would send it. Prints
the answer's line, with --time first the milliseconds from connecting to the answer. Exits 2 and prints `error TEXT` when
the socket cannot be reached or hangs up (what an account with no session, or a Flatpak, gets). --hold keeps the first
connection open and idle that long before asking, to see the service drop it.

    vaultsock.py '{"v":1,"op":"status"}'
"""
import socket
import sys
import time


def ask(path, line, hold=0.0):
    began = time.monotonic()
    with socket.socket(socket.AF_UNIX) as sock:
        sock.settimeout(60)
        sock.connect(path)
        if hold:
            time.sleep(hold)
        sock.sendall(line.encode() + b"\n")
        answer = b""
        while not answer.endswith(b"\n"):
            chunk = sock.recv(65536)
            if not chunk:
                break
            answer += chunk
    return answer.decode().strip(), (time.monotonic() - began) * 1000


def main(argv):
    path, show_time, hold, lines = "/run/amethystora/vault.sock", False, 0.0, []
    while argv:
        word = argv.pop(0)
        if word == "--socket":
            path = argv.pop(0)
        elif word == "--time":
            show_time = True
        elif word == "--hold":
            hold = float(argv.pop(0))
        else:
            lines.append(word)
    status = 0
    for line in lines:
        try:
            answer, ms = ask(path, line, hold)
        except OSError as error:
            print(f"error {error}")
            status = 2
            continue
        if not answer:
            print("error hung up")
            status = 2
            continue
        print(f"{ms:.0f}\t{answer}" if show_time else answer)
        hold = 0.0
    return status


sys.exit(main(sys.argv[1:]))
