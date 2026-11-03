#!/usr/bin/env python3
"""Archives `amethystora-plugin-root prepare` must refuse (tests/vm/suites/72-plugins.sh): one file for each thing a plugin never holds.

Usage: archives.py OUTPUT-FOLDER MANIFEST.json
"""
import io
import os
import sys
import tarfile

out, manifest = sys.argv[1], open(sys.argv[2], "rb").read()
os.makedirs(out, exist_ok=True)


def make(name, extra):
    with tarfile.open(os.path.join(out, name + ".tar"), "w", format=tarfile.USTAR_FORMAT) as tar:
        info = tarfile.TarInfo("manifest.json")
        info.size, info.mode = len(manifest), 0o644
        tar.addfile(info, io.BytesIO(manifest))
        extra(tar)


def member(name, kind, mode=0o644, data=b"", linkname=""):
    info = tarfile.TarInfo(name)
    info.type, info.mode, info.linkname = kind, mode, linkname
    info.size = len(data) if kind == tarfile.REGTYPE else 0
    return info, io.BytesIO(data)


make("symlink", lambda t: t.addfile(*member("run.sh", tarfile.SYMTYPE, linkname="/etc/passwd")))
make("hardlink", lambda t: t.addfile(*member("run.sh", tarfile.LNKTYPE, linkname="manifest.json")))
make("device", lambda t: t.addfile(*member("null", tarfile.CHRTYPE)))
make("fifo", lambda t: t.addfile(*member("pipe", tarfile.FIFOTYPE)))
make("traversal", lambda t: t.addfile(*member("../escape", tarfile.REGTYPE, data=b"x")))
make("traversal-inside", lambda t: t.addfile(*member("a/../../escape", tarfile.REGTYPE, data=b"x")))
make("absolute", lambda t: t.addfile(*member("/etc/evil", tarfile.REGTYPE, data=b"x")))
make("setuid", lambda t: t.addfile(*member("run.sh", tarfile.REGTYPE, mode=0o4755, data=b"#!/bin/sh\n")))
make("setgid", lambda t: t.addfile(*member("run.sh", tarfile.REGTYPE, mode=0o2755, data=b"#!/bin/sh\n")))
make("twice", lambda t: [t.addfile(*member("run.sh", tarfile.REGTYPE, data=b"x")) for _ in range(2)])
make("huge", lambda t: t.addfile(*member("big", tarfile.REGTYPE, data=b"\0" * (9 << 20))))
make("many", lambda t: [t.addfile(*member(f"f{i}", tarfile.REGTYPE, data=b"x")) for i in range(600)])
make("newline-in-name", lambda t: t.addfile(*member("a\nb", tarfile.REGTYPE, data=b"x")))
with open(os.path.join(out, "not-a-tar.tar"), "wb") as f:
    f.write(b"#!/bin/sh\necho not a tar\n" * 100)
