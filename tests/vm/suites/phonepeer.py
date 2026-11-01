#!/usr/bin/python3
"""A stand-in phone for the suites: the phone link's protocol (plans/amethystora-connect/spec-protocol.md, protocol 8)
from the phone's side, as far as a test needs it, with nothing of any other program's code. It has a certificate of
its own, is paired with the program by its pin (the suite writes it into the account's list of paired phones before
the program starts), connects to the computer, and then does what the lines of a command file say, one at a time:

  phonepeer.py init DIR                  a key, a certificate and a device id in DIR; prints the device id
  phonepeer.py pin DIR                   the certificate as the list of paired phones keeps it (base64 of the DER)
  phonepeer.py run DIR HOST [PORT]       connects to HOST:PORT (1716) and serves DIR/commands, logging to DIR/log

Commands, one a line, appended to DIR/commands while it runs:

  packet TYPE JSON                       sends a packet of that type with that body
  share PATH                             shares a file, its payload served on a port of its own
  quit                                   closes the connection and ends
"""
import base64
import json
import os
import socket
import ssl
import subprocess
import sys
import threading
import time

PHONE_IN = ["kdeconnect.ping", "kdeconnect.battery.request", "kdeconnect.notification.request",
            "kdeconnect.share.request", "kdeconnect.clipboard"]
PHONE_OUT = ["kdeconnect.ping", "kdeconnect.battery", "kdeconnect.share.request", "kdeconnect.connectivity_report"]
# PEER_IN_MORE and PEER_OUT_MORE, comma lists, add packet types: the ring, the players and the presenter of suite 64
PHONE_IN += [t for t in os.environ.get("PEER_IN_MORE", "").split(",") if t]
PHONE_OUT += [t for t in os.environ.get("PEER_OUT_MORE", "").split(",") if t]


def log(directory, line):
    with open(os.path.join(directory, "log"), "a") as out:
        out.write(f"{time.strftime('%H:%M:%S')} {line}\n")


def packet(kind, body, **extra):
    return (json.dumps({"id": int(time.time() * 1000), "type": kind, "body": body, **extra}) + "\n").encode()


def device_id(directory):
    with open(os.path.join(directory, "id")) as source:
        return source.read().strip()


def identity(directory):
    # PEER_TYPE and PEER_NAME play another kind of peer: a laptop for the computers' suite (67)
    return packet("kdeconnect.identity", {
        "deviceId": device_id(directory), "deviceName": os.environ.get("PEER_NAME", "Lab phone"),
        "deviceType": os.environ.get("PEER_TYPE", "phone"), "protocolVersion": 8,
        "incomingCapabilities": PHONE_IN, "outgoingCapabilities": PHONE_OUT, "tcpPort": 1716})


def init(directory):
    os.makedirs(directory, exist_ok=True)
    ident = os.urandom(16).hex()
    with open(os.path.join(directory, "id"), "w") as out:
        out.write(ident + "\n")
    subprocess.run(["openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1", "-nodes",
                    "-keyout", os.path.join(directory, "key.pem"), "-out", os.path.join(directory, "cert.pem"),
                    "-days", "3650", "-subj", f"/O=Amethystora lab/OU=phone/CN={ident}"],
                   check=True, capture_output=True)
    print(ident)


def pin(directory):
    der = subprocess.run(["openssl", "x509", "-in", os.path.join(directory, "cert.pem"), "-outform", "DER"],
                         check=True, capture_output=True).stdout
    print(base64.b64encode(der).decode())


def server_context(directory):
    # The caller of a control connection is the TLS server, and so is the side that serves a payload; the computer's
    # own certificate is not asked for here, which the computer does not need from its server
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(os.path.join(directory, "cert.pem"), os.path.join(directory, "key.pem"))
    context.verify_mode = ssl.CERT_NONE
    return context


def serve_payload(directory, path, listener):
    connection, _ = listener.accept()
    listener.close()
    with server_context(directory).wrap_socket(connection, server_side=True) as tls, open(path, "rb") as source:
        while chunk := source.read(65536):
            tls.sendall(chunk)
    log(directory, f"served {path}")


def reader(directory, tls):
    buffer = b""
    while True:
        try:
            data = tls.recv(65536)
        except OSError:
            break
        if not data:
            break
        buffer += data
        while b"\n" in buffer:
            line, buffer = buffer.split(b"\n", 1)
            try:
                got = json.loads(line)
                kind = got.get("type")
                body = json.dumps(got.get("body", {}), separators=(",", ":"), sort_keys=True)
            except ValueError:
                kind, body = "?", ""
            # The body after the type, for the suites that look into it: "got TYPE" stays the start of the line
            log(directory, f"got {kind} {body}")
    log(directory, "closed by the computer")


def run(directory, host, port):
    raw = socket.create_connection((host, port), timeout=10)
    raw.sendall(identity(directory))
    tls = server_context(directory).wrap_socket(raw, server_side=True)
    # Protocol 8: both send the identity again inside TLS, then read the other's
    tls.sendall(identity(directory))
    theirs = b""
    while not theirs.endswith(b"\n"):
        data = tls.recv(1)
        if not data:
            log(directory, "closed during the identity")
            return 1
        theirs += data
    log(directory, f"connected to {json.loads(theirs)['body'].get('deviceName')}")
    # What the computer listed, for the suites that look at it (64: what its switches add)
    for side in ("incoming", "outgoing"):
        listed = json.loads(theirs)["body"].get(f"{side}Capabilities", [])
        log(directory, f"their {side} {','.join(listed)}")
    tls.settimeout(None)
    threading.Thread(target=reader, args=(directory, tls), daemon=True).start()
    commands = os.path.join(directory, "commands")
    open(commands, "a").close()
    done = 0
    while True:
        with open(commands) as source:
            lines = source.read().splitlines()
        for line in lines[done:]:
            done += 1
            word, _, rest = line.partition(" ")
            if word == "quit":
                log(directory, "quit")
                tls.close()
                return 0
            if word == "packet":
                kind, _, body = rest.partition(" ")
                tls.sendall(packet(kind, json.loads(body)))
                log(directory, f"sent {kind}")
            elif word == "share":
                listener = socket.socket()
                listener.bind(("", 0))
                listener.listen(1)
                size = os.path.getsize(rest)
                threading.Thread(target=serve_payload, args=(directory, rest, listener), daemon=True).start()
                tls.sendall(packet("kdeconnect.share.request", {"filename": os.path.basename(rest)},
                                   payloadSize=size, payloadTransferInfo={"port": listener.getsockname()[1]}))
                log(directory, f"shared {rest}")
        time.sleep(0.2)


if __name__ == "__main__":
    what, directory = sys.argv[1], sys.argv[2]
    if what == "init":
        init(directory)
    elif what == "pin":
        pin(directory)
    elif what == "run":
        sys.exit(run(directory, sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 1716))
    else:
        sys.exit(__doc__)
