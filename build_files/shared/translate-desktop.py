#!/usr/bin/env python3
"""Put the apps' translations into their launchers: Name[fr]=, Comment[fr]= and the rest.

    translate-desktop.py [root]

Each app's launcher, /usr/share/applications/<app>.desktop, says in English what the app is called, what
it does and what to find it by, in its main entry and in its actions. The app's catalogs, locale/<language>.json
beside it, hold those same words translated (check-translations.js lists them there for the translators),
and this writes each translation in below its English, the way the desktop entry specification has a
launcher carry its other languages. Run by 13-apps.sh, once the apps and their catalogs are in place.
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/")
APPS = ["amethystora-manual", "amethystora-security", "amethystora-update", "amethystora-logs", "amethystora-notes",
        "amethystora-control", "amethystora-backups", "amethystora-phone", "amethystora-shelf",
        "amethystora-trim"]
# Launchers of no app of their own, with the app whose catalogs hold their words
OTHERS = {"amethystora-private-folder": "amethystora-security", "amethystora-phone-send": "amethystora-phone",
          "zz-amethystora-open-anything": "amethystora-control"}
KEYS = ("Name", "GenericName", "Comment", "Keywords")
LOCALIZED = re.compile(r"^(Name|GenericName|Comment|Keywords)\[")

for app, owner in [(app, app) for app in APPS] + list(OTHERS.items()):
    desktop = ROOT / "usr/share/applications" / f"{app}.desktop"
    catalogs = {
        catalog.stem: json.loads(catalog.read_text(encoding="utf-8"))
        for catalog in sorted((ROOT / "usr/lib" / owner / "resources/app/locale").glob("*.json"))
        if catalog.stem != "en"
    }
    lines = []
    for line in desktop.read_text(encoding="utf-8").splitlines():
        # Written again below from the catalogs, so that running this twice changes nothing
        if LOCALIZED.match(line):
            continue
        lines.append(line)
        key, _, value = line.partition("=")
        if key in KEYS and value:
            for language, strings in catalogs.items():
                text = strings.get(value)
                if isinstance(text, str) and text:
                    # The specification's locale names are POSIX ones: pt_BR, not pt-BR
                    lines.append(f"{key}[{language.replace('-', '_')}]={text}")
    desktop.write_text("\n".join(lines) + "\n", encoding="utf-8")
