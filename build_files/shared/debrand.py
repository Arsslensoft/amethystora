#!/usr/bin/env python3
"""Rename what the upstream layers ship as Universal Blue (ublue) to Amethystora.

The ublue-os base image, ublue-os/brew and the ublue COPR packages install files, commands,
services and settings under their own names. This renames all of them to Amethystora and
rewrites every reference, so the image carries no upstream names.

    debrand.py          apply the changes (07-debrand.sh)
    debrand.py --check  report what is left and fail on anything renameable (20-tests.sh)

Left alone on purpose:
  - compiled files: the uupd binary, the kernel and akmods packages and their signing certificate
  - licence files, copyright lines and credit comments: upstream's attribution must stay (Apache-2.0)
  - the RPM database and kernel modules
  - files owned by other RPMs, whose words are not our branding

DEBRAND_ROOT points the script at a staged tree instead of / for testing.
"""

import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.environ.get("DEBRAND_ROOT", "/")
REPO = "arsslensoft/amethystora"
REPO_URL = f"https://github.com/{REPO}"
VENDOR = REPO.split("/")[0]

SCAN_DIRS = ("usr", "etc")
PRUNE = ("usr/lib/modules", "usr/src", "usr/lib/sysimage", "usr/share/licenses", "usr/lib/.build-id")
LEGAL = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE)", re.I)
MAX_TEXT_SIZE = 16 * 1024 * 1024

TRACE = re.compile(r"ublue|universal.?blue", re.I)
TRACE_BYTES = re.compile(rb"ublue|universal.?blue", re.I)


def path(rel):
    return os.path.join(ROOT, rel)


def rel(full):
    return os.path.relpath(full, ROOT).replace(os.sep, "/")


def log(message):
    print(f"debrand: {message}", flush=True)


# --- Renaming rules ---------------------------------------------------------------------------

def keep_case(word):
    def replace(match):
        found = match.group(0)
        if found.isupper():
            return word.upper()
        if found[0].isupper():
            return word.capitalize()
        return word
    return replace


# Names, applied to both file names and file contents
WORDS = [
    (re.compile(r"universal[\s_-]?blue", re.I), keep_case("amethystora")),
    (re.compile(r"ublue[-_]os", re.I), keep_case("amethystora")),
    (re.compile(r"ublue", re.I), keep_case("amethystora")),
    # Names that come out doubled ("amethystora-amethystora") collapse into one
    (re.compile(r"(amethystora)[-_ ]amethystora", re.I), r"\1"),
]

URL_TAIL = r"[^\s\"'<>)\]}`]*"


def github_url(match):
    url = match.group(0)
    if "/discussions" in url:
        return f"{REPO_URL}/discussions"
    if "/issues/new" in url:
        return f"{REPO_URL}/issues/new"
    if "/issues" in url:
        return f"{REPO_URL}/issues"
    return REPO_URL


# Links and repository names, applied to file contents before WORDS
LINKS = [
    (re.compile(rf"https?://(?:[\w-]+\.)*(?:ublue\.it|universal-blue\.org){URL_TAIL}"), REPO_URL),
    (re.compile(rf"https?://github\.com/ublue-os\b{URL_TAIL}"), github_url),
    (re.compile(rf"https?://raw\.githubusercontent\.com/ublue-os/{URL_TAIL}"), REPO_URL),
    (re.compile(r"ghcr\.io/ublue-os\b"), f"ghcr.io/{VENDOR}"),
    # Upstream issue numbers ("ublue-os/main#1328") mean nothing on Amethystora's tracker
    (re.compile(r"(?<![\w./-])ublue-os/[\w.-]+#\d+"), REPO),
    (re.compile(
        r"(?<![\w./-])ublue-os/(?:aurora|bazzite|common|dakota|main"
        r"|config|packages|staging|akmods|bling|brew|uupd|umotd|uwelcome|legacy-rechunk|image-template)\b"
    ), REPO),
    # Upstream's blueberry emoji
    (re.compile("\U0001FAD0"), "\U0001F49C"),
]


# Copyright and licence lines keep upstream's names: the attribution must stay intact. So do the
# comments that credit who wrote something and where, as the udev rules taken from Universal Blue do
# ("# Written by ...", "# First added on https://github.com/ublue-os/config/..."): renaming the place
# would credit their work to this repository.
LEGAL_LINE = re.compile(
    r"copyright|\(c\)\s|©|spdx-license|licen[cs]ed under|all rights reserved"
    r"|^\s*(?:#|//|;)\s*(?:this got added on|added (?:by|to config on)|first (?:added|appeared) on|created on"
    r"|modified by|written by|commit (?:made by|from)|credits?:|source:)\s",
    re.I,
)


def rename_text(text):
    lines = text.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if LEGAL_LINE.search(line):
            continue
        for pattern, replacement in LINKS + WORDS:
            line = pattern.sub(replacement, line)
        lines[i] = line
    return "".join(lines)


def rename_name(name):
    for pattern, replacement in WORDS:
        name = pattern.sub(replacement, name)
    return name


# --- Scope --------------------------------------------------------------------------------------

def pruned(relative):
    return any(relative == p or relative.startswith(p + "/") for p in PRUNE)


def walk():
    """Yield (directory, dirnames, filenames) under the scanned directories, minus PRUNE."""
    for top in SCAN_DIRS:
        for directory, dirnames, filenames in os.walk(path(top)):
            dirnames[:] = [d for d in dirnames if not pruned(rel(os.path.join(directory, d)))]
            yield directory, dirnames, filenames


def rpm_owned():
    """Files that other RPMs own. Files of packages built by Universal Blue stay in scope."""
    if ROOT != "/" or not shutil.which("rpm"):
        return set()

    def rpm(*args):
        return subprocess.run(["rpm", *args], check=True, capture_output=True, text=True).stdout.splitlines()

    owned = set(rpm("-qal"))
    for line in rpm("-qa", "--qf", "%{NAME}\t%{VENDOR}\t%{PACKAGER}\t%{URL}\n"):
        if TRACE.search(line):
            owned.difference_update(rpm("-ql", line.split("\t")[0]))
    return owned


def files_in_scope(owned):
    for directory, _, filenames in walk():
        for name in filenames:
            full = os.path.join(directory, name)
            if os.path.islink(full) or LEGAL.match(name) or "/" + rel(full) in owned:
                continue
            yield full


# --- Removals ahead of the rename ------------------------------------------------------------------

def drop_upstream_trust():
    """Universal Blue's image signing keys, registries and package repositories."""
    policy = path("etc/containers/policy.json")
    if os.path.isfile(policy):
        with open(policy, encoding="utf-8") as f:
            data = json.load(f)
        docker = data.get("transports", {}).get("docker", {})
        for registry in [r for r in docker if TRACE.search(r)]:
            del docker[registry]
            log(f"removed {registry} from {rel(policy)}")
        with open(policy, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=4)
            f.write("\n")
    for directory in ("etc/containers/registries.d", "etc/pki/containers", "usr/lib/pki/containers",
                      "etc/yum.repos.d"):
        if not os.path.isdir(path(directory)):
            continue
        for name in os.listdir(path(directory)):
            if TRACE.search(name):
                os.remove(path(f"{directory}/{name}"))
                log(f"removed {directory}/{name}")


# --- Rename --------------------------------------------------------------------------------------

def merge(source, target):
    """Move source to target. Where both exist, what is already at target (Amethystora's file) wins."""
    if os.path.lexists(target) and not (os.path.isdir(source) and not os.path.islink(source)
                                         and os.path.isdir(target) and not os.path.islink(target)):
        log(f"kept {rel(target)} over {rel(source)}")
        if os.path.isdir(source) and not os.path.islink(source):
            shutil.rmtree(source)
        else:
            os.remove(source)
    elif os.path.lexists(target):
        for name in os.listdir(source):
            merge(os.path.join(source, name), os.path.join(target, name))
        os.rmdir(source)
    else:
        # Overlay refuses to rename a directory of a lower layer (EXDEV) unless redirect_dir is on, and
        # shutil.move copies it then
        shutil.move(source, target)


def rename_paths():
    found = []
    for directory, dirnames, filenames in walk():
        found += [os.path.join(directory, name) for name in dirnames + filenames if TRACE.search(name)]
    # Deepest first, so a directory's contents are renamed before the directory itself
    for full in sorted(found, key=lambda p: p.count(os.sep), reverse=True):
        directory, name = os.path.split(full)
        merge(full, os.path.join(directory, rename_name(name)))
    log(f"renamed {len(found)} files and directories")


def retarget_symlinks():
    for directory, dirnames, filenames in walk():
        for name in dirnames + filenames:
            full = os.path.join(directory, name)
            if os.path.islink(full) and TRACE.search(os.readlink(full)):
                target = rename_name(os.readlink(full))
                os.remove(full)
                os.symlink(target, full)


def rewrite_files(owned):
    count = 0
    for full in files_in_scope(owned):
        if os.path.getsize(full) > MAX_TEXT_SIZE:
            continue
        with open(full, "rb") as f:
            data = f.read()
        if not TRACE_BYTES.search(data) and "\U0001FAD0".encode() not in data:
            continue
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            continue
        if "\0" in text:
            continue
        new = rename_text(text)
        if new != text:
            with open(full, "w", encoding="utf-8", newline="") as f:
                f.write(new)
            count += 1
    log(f"rewrote {count} files")


# --- Check ---------------------------------------------------------------------------------------

def check(owned):
    """Fail on any upstream name left in a path or text file; list compiled files that keep one."""
    problems, compiled = [], []
    for directory, dirnames, filenames in walk():
        for name in dirnames + filenames:
            full = os.path.join(directory, name)
            if TRACE.search(name):
                problems.append(f"path: /{rel(full)}")
            elif os.path.islink(full) and TRACE.search(os.readlink(full)):
                problems.append(f"link: /{rel(full)} -> {os.readlink(full)}")
    for full in files_in_scope(owned):
        if os.path.getsize(full) > MAX_TEXT_SIZE:
            continue
        with open(full, "rb") as f:
            data = f.read()
        if not TRACE_BYTES.search(data):
            continue
        try:
            text = data.decode("utf-8")
            binary = "\0" in text
        except UnicodeDecodeError:
            binary = True
        if binary:
            compiled.append(f"/{rel(full)}")
            continue
        line = next((line for line in text.splitlines() if TRACE.search(line) and not LEGAL_LINE.search(line)), None)
        if line is not None:
            problems.append(f"text: /{rel(full)}: {line.strip()[:160]}")
    for item in compiled:
        log(f"compiled file keeps an upstream name (expected): {item}")
    for item in problems:
        print(f"::error::upstream name left in the image, {item}", flush=True)
    return 1 if problems else 0


def main():
    owned = rpm_owned()
    if sys.argv[1:] == ["--check"]:
        return check(owned)
    drop_upstream_trust()
    rename_paths()
    retarget_symlinks()
    rewrite_files(owned)
    return 0


if __name__ == "__main__":
    sys.exit(main())
