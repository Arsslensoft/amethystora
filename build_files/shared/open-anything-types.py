#!/usr/bin/python3 -I
"""Writes the kinds of file nothing in the image opens into zz-amethystora-open-anything.desktop's MimeType, once every package
and launcher is in place, and has the launchers' index made again, which is what GIO reads (R7-40).

Left to what they have today:
- anything an image's launcher claims, or /etc/xdg/mimeapps.list names, whose app keeps it;
- text, and anything under text/plain: Text Editor opens it;
- folders, the media types of volumes, messages and programs, which Files handles itself;
- a kind with a parent other than a plain container: GIO asks for an exact kind first, so a claim here would come before an
  app that opens the parent (Showtime's video/x-matroska and its 3D child). A ZIP or OLE file is no such parent: an office
  document is a ZIP that nothing here opens as what it is.

An app installed later comes first all the same: Flatpak's launchers are ahead of /usr/share in XDG_DATA_DIRS, and in
/usr/share (a package layered, or an image built on this one) update-desktop-database sorts each kind's list by the launcher's
name, which is why this one's starts with zz-.
"""

import configparser
import glob
import subprocess
import sys
import xml.etree.ElementTree as ET

APPLICATIONS = "/usr/share/applications"
LAUNCHER = f"{APPLICATIONS}/zz-amethystora-open-anything.desktop"
MIME = "{http://www.freedesktop.org/standards/shared-mime-info}"
# A parent that says only how a file is packed
CONTAINERS = {"application/zip", "application/x-ole-storage", "application/octet-stream"}
LEFT_MEDIA = ("text/", "inode/", "x-content/", "message/", "multipart/", "x-scheme-handler/", "all/")
LEFT_KINDS = {
    "application/x-zerosize", "application/x-executable", "application/x-pie-executable", "application/x-sharedlib",
    "application/x-core", "application/x-object", "application/x-desktop",
}


def kinds_known():
    parents, aliases = {}, {}
    for package in glob.glob("/usr/share/mime/packages/*.xml"):
        for kind in ET.parse(package).getroot().iter(f"{MIME}mime-type"):
            name = kind.get("type")
            parents.setdefault(name, set()).update(p.get("type") for p in kind.iterfind(f"{MIME}sub-class-of"))
            for alias in kind.iterfind(f"{MIME}alias"):
                aliases[alias.get("type")] = name
    return parents, aliases


def ancestors(kind, parents, aliases):
    seen, waiting = set(), [aliases.get(p, p) for p in parents.get(kind, ())]
    while waiting:
        parent = waiting.pop()
        if parent not in seen:
            seen.add(parent)
            waiting += [aliases.get(p, p) for p in parents.get(parent, ())]
    return seen


def kinds_handled():
    handled = set()
    for path in glob.glob(f"{APPLICATIONS}/*.desktop"):
        if path == LAUNCHER:
            continue
        keys = configparser.RawConfigParser(strict=False, interpolation=None)
        try:
            keys.read(path, encoding="utf-8")
            handled.update(k for k in keys.get("Desktop Entry", "MimeType", fallback="").split(";") if k)
        except (configparser.Error, UnicodeDecodeError):
            continue
    with open("/etc/xdg/mimeapps.list", encoding="utf-8") as defaults:
        handled.update(line.split("=")[0].strip() for line in defaults if "=" in line and not line.startswith("#"))
    return handled


def main():
    parents, aliases = kinds_known()
    # A launcher's kind, and what it is an alias of: a name can be both a type of one package and an alias in another
    # (gcr-viewer's application/x-pkcs7-certificates), and claiming it would take it from that app
    named = kinds_handled()
    handled = named | {aliases.get(kind, kind) for kind in named}
    claimed = sorted(
        kind for kind in parents
        if kind not in handled and kind not in LEFT_KINDS and not kind.startswith(LEFT_MEDIA)
        and ancestors(kind, parents, aliases) <= CONTAINERS)
    if len(claimed) < 100:
        sys.exit(f"open-anything-types: only {len(claimed)} kinds, which is not what shared-mime-info has")
    with open(LAUNCHER, encoding="utf-8") as launcher:
        lines = launcher.read().splitlines()
    written = ["MimeType=" + ";".join(claimed) + ";" if line.startswith("MimeType=") else line for line in lines]
    with open(LAUNCHER, "w", encoding="utf-8") as launcher:
        launcher.write("\n".join(written) + "\n")
    subprocess.run(["update-desktop-database", APPLICATIONS], check=True)
    print(f"open-anything-types: {len(claimed)} kinds claimed")


if __name__ == "__main__":
    main()
