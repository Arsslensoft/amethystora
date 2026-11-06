#!/usr/bin/python3 -I
# A stand-in for the Vault's seal/open/forget socket (apps/vault-client/src/lib.rs), for the tests of the features that keep
# data under it: never the live Vault. It speaks the same lines of JSON and really encrypts (SHAKE-256 as a keystream, an
# HMAC over the purpose, the record and the ciphertext), so that a test looking for a canary in what was written means something.
#
#   test-vault-stand-in.py SOCKET MODE-FILE
#
# The mode is read from MODE-FILE at every request: `open` (everything works), `closed`, `not-set-up`, `refused`, `background`
# (closed, but the purpose `background` works, as the service serves it with the vault closed), or `slow` (never answers).

import base64
import hashlib
import hmac
import json
import os
import socket
import sys
import threading
import time

KEYS = {}


def keystream(key, nonce, length):
    return hashlib.shake_256(key + nonce).digest(length)


def seal(purpose, record, data):
    key = KEYS.setdefault(purpose, os.urandom(32))
    nonce = os.urandom(12)
    body = bytes(a ^ b for a, b in zip(data, keystream(key, nonce, len(data))))
    tag = hmac.new(key, purpose.encode() + b"\0" + record.encode() + b"\0" + nonce + body, hashlib.sha256).digest()[:16]
    return b"AVs1" + nonce + tag + body


def unseal(purpose, record, data):
    key = KEYS.get(purpose)
    if key is None or len(data) < 32 or data[:4] != b"AVs1":
        return None
    nonce, tag, body = data[4:16], data[16:32], data[32:]
    want = hmac.new(key, purpose.encode() + b"\0" + record.encode() + b"\0" + nonce + body, hashlib.sha256).digest()[:16]
    if not hmac.compare_digest(tag, want):
        return None
    return bytes(a ^ b for a, b in zip(body, keystream(key, nonce, len(body))))


def answer(mode, request):
    op = request.get("op")
    purpose = request.get("purpose", "")
    if mode == "slow":
        time.sleep(60)
    if mode == "not-set-up":
        return {"ok": False, "why": "not-set-up"}
    if mode == "refused":
        return {"ok": False, "why": "refused"}
    if mode == "closed" or (mode == "background" and purpose != "background"):
        return {"ok": False, "why": "closed"}
    if op in ("seal", "open"):
        try:
            data = base64.b64decode(request["data"], validate=True)
        except Exception:
            return {"ok": False, "why": "bad-request"}
        if op == "seal":
            return {"ok": True, "data": base64.b64encode(seal(purpose, request["record"], data)).decode()}
        plain = unseal(purpose, request["record"], data)
        if plain is None:
            return {"ok": False, "why": "refused"}
        return {"ok": True, "data": base64.b64encode(plain).decode()}
    if op == "forget":
        KEYS.pop(purpose, None)
        return {"ok": True}
    return {"ok": False, "why": "bad-request"}


def serve(connection, mode_file):
    with connection:
        connection.settimeout(5)
        try:
            line = connection.makefile("rb").readline()
            mode = open(mode_file).read().strip() or "open"
            request = json.loads(line)
            reply = answer(mode, request)
        except Exception:
            reply = {"ok": False, "why": "bad-request"}
        try:
            connection.sendall(json.dumps(reply).encode() + b"\n")
        except OSError:
            pass


def main():
    path, mode_file = sys.argv[1], sys.argv[2]
    if os.path.exists(path):
        os.unlink(path)
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.bind(path)
    os.chmod(path, 0o600)
    server.listen(8)
    while True:
        connection, _ = server.accept()
        threading.Thread(target=serve, args=(connection, mode_file), daemon=True).start()


if __name__ == "__main__":
    main()
