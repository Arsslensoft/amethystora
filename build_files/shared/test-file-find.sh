#!/usr/bin/bash
# amethystora-file-find (Find it by what you remember, R7-60), against an index made here in memory with GNOME's own
# ontology, and real files in a stand-in home: every rule asks the index what it should, what was typed reaches it only
# as a bound value, what is gone, a link or in the Private folder is never answered, and a question it does not know
# stops it. Nothing is written beside the stand-in home.
set -euo pipefail

HELPER="${1:-/usr/libexec/amethystora-file-find}"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

# Questions that are not questions, before any index is touched
for bad in '[]' '"x"' '{"kinds": ["nothing"]}' '{"order": "name"}' '{"after": "yesterday"}' '{"min": "1"}' \
    '{"min": -1}' '{"words": "a"}' '{"words": ["a", "b", "c", "d", "e"]}' '{"unknown": 1}' 'not json' \
    '{"kinds": [[]]}' '{"kinds": [{}]}' '{"order": []}' '{"order": {}}' '{"after": "2026-10-22T10:00:00Z\n"}'; do
    if printf '%s' "${bad}" | HOME="${WORK}" "${HELPER}" >/dev/null 2>&1; then
        echo "amethystora-file-find took ${bad}"
        exit 1
    fi
done

# -B: loading the helper must leave no bytecode beside it, in the image this runs in
python3 -I -B - "${HELPER}" "${WORK}" <<'PY'
import importlib.machinery
import importlib.util
import json
import os
import sys

import gi
gi.require_version('Tsparql', '3.0')
from gi.repository import Gio, Tsparql

helper, work = sys.argv[1:3]
loader = importlib.machinery.SourceFileLoader('file_find', helper)
spec = importlib.util.spec_from_loader('file_find', loader)
find = importlib.util.module_from_spec(spec)
loader.exec_module(find)

home = os.path.join(work, 'home')
os.makedirs(os.path.join(home, 'Documents'))
os.makedirs(os.path.join(home, 'Private'))
outside = os.path.join(work, 'outside.pdf')

FILES = [
    # name, MIME type, modified, size
    ('Documents/Lease.pdf', 'application/pdf', '2026-10-22T10:00:00Z', 500),
    ('Documents/Invoice 118.pdf', 'application/pdf', '2026-10-20T10:00:00Z', 900),
    ('Documents/Résumé.pdf', 'application/pdf', '2026-09-01T10:00:00Z', 700),
    ('Documents/film.mkv', 'video/x-matroska', '2026-10-21T10:00:00Z', 5000000000),
    ('Documents/clip.mp4', 'video/mp4', '2026-10-21T11:00:00Z', 20000000),
    ('Documents/notes.txt', 'text/plain', '2026-10-27T10:00:00Z', 10),
    ('Private/secret.pdf', 'application/pdf', '2026-10-23T10:00:00Z', 10),
    ('Documents/gone.pdf', 'application/pdf', '2026-10-24T10:00:00Z', 10),
    ('Documents/link.pdf', 'application/pdf', '2026-10-25T10:00:00Z', 10),
]
# A name that is not UTF-8, and thirty files of the Private folder newer than all of these (not on disk: the index alone
# must not let them use up the rows asked for)
BAD = os.fsdecode(b'Documents/bad\xff.pdf')
FILES.append((BAD, 'application/pdf', '2026-10-28T10:00:00Z', 10))
FILES += [(f'Private/more{index}.pdf', 'application/pdf', f'2026-10-29T10:{index:02d}:00Z', 10) for index in range(30)]
for name, _, _, _ in FILES:
    if name.endswith(('gone.pdf', 'link.pdf')) or '/more' in name:
        continue
    with open(os.path.join(home, name), 'w', encoding='utf-8') as file:
        file.write('x')
with open(outside, 'w', encoding='utf-8') as file:
    file.write('x')
os.symlink(outside, os.path.join(home, 'Documents/link.pdf'))

connection = Tsparql.SparqlConnection.new(Tsparql.SparqlConnectionFlags.NONE, None,
                                          Tsparql.sparql_get_ontology_nepomuk(), None)
uri = lambda name: Gio.File.new_for_path(os.path.join(home, name)).get_uri()
rows = list(FILES) + [('../outside.pdf', 'application/pdf', '2026-10-26T10:00:00Z', 10)]
for index, (name, mime, modified, size) in enumerate(rows):
    url = Gio.File.new_for_path(os.path.normpath(os.path.join(home, name))).get_uri()
    base = 'bad.pdf' if name == BAD else os.path.basename(name).replace('"', '')
    connection.update(
        f'INSERT DATA {{ <{url}> a nfo:FileDataObject ; nie:url "{url}" ; nfo:fileName "{base}" ; '
        f'nfo:fileLastModified "{modified}"^^xsd:dateTime ; nfo:fileSize {size} . '
        f'<urn:content:{index}> a nfo:Document ; nie:isStoredAs <{url}> ; nie:mimeType "{mime}" }}', None)
folder = Gio.File.new_for_path(os.path.join(home, 'Documents')).get_uri()
connection.update(
    f'INSERT DATA {{ <{folder}> a nfo:FileDataObject ; nie:url "{folder}" ; nfo:fileName "pdf" ; '
    f'nfo:fileLastModified "2026-10-22T10:00:00Z"^^xsd:dateTime ; nfo:fileSize 0 . '
    f'<urn:content:folder> a nfo:Folder ; nie:isStoredAs <{folder}> ; nie:mimeType "inode/directory" }}', None)


def names(question):
    checked = find.checked(question)
    assert checked is not None, question
    answered = find.answer(find.ask(connection, checked, home), home)
    # What the program prints must be sayable in UTF-8 whatever the index holds
    json.dumps(answered, ensure_ascii=False).encode('utf-8')
    for hit in answered:
        assert hit['path'].startswith(home + os.sep) and isinstance(hit['modified'], int) and hit['size'] == 1
    return [os.path.basename(hit['path']) for hit in answered]


def same(what, got, want):
    if got != want:
        raise SystemExit(f'{what}: expected {want}, got {got}')


same('a kind, newest first, nothing gone, linked, private or outside',
     names({'kinds': ['pdf'], 'order': 'modified'}), ['Lease.pdf', 'Invoice 118.pdf', 'Résumé.pdf'])
same('a date',
     names({'kinds': ['pdf'], 'after': '2026-10-19T00:00:00.000Z', 'before': '2026-10-22T00:00:00.000Z'}),
     ['Invoice 118.pdf'])
same('a kind by its start', names({'kinds': ['video'], 'order': 'modified'}), ['clip.mp4', 'film.mkv'])
same('big, largest first', names({'kinds': ['video'], 'order': 'size', 'min': 10485760}), ['film.mkv', 'clip.mp4'])
same('over', names({'order': 'size', 'min': 100000000}), ['film.mkv'])
same('under, smallest first', names({'kinds': ['pdf'], 'order': 'small', 'max': 800}), ['Lease.pdf', 'Résumé.pdf'])
same('a word of the name', names({'kinds': ['pdf'], 'words': ['lease']}), ['Lease.pdf'])
same('a word without its accents', names({'kinds': ['pdf'], 'words': ['resume']}), ['Résumé.pdf'])
same('every word has to be there', names({'kinds': ['pdf'], 'words': ['invoice', 'lease']}), [])
same('a folder is not a file', names({'after': '2026-10-22T00:00:00Z', 'before': '2026-10-22T23:00:00Z'}), ['Lease.pdf'])
# What was typed cannot change the query
same('a quote is only a word', names({'words': ['") } #']}), [])
same('a query is only a word', names({'words': ["' || true || '"]}), [])
for text in ['") } #', 'lease']:
    assert text not in find.query_text({'kinds': ['pdf'], 'words': [text]})
PY

# With no index to ask it answers nothing, and that is not a fault
out="$(printf '{"kinds": ["pdf"]}' | env -u DBUS_SESSION_BUS_ADDRESS HOME="${WORK}" DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent "${HELPER}")"
[[ "${out}" == "[]" ]]
# Nothing beside the stand-in home, and no bytecode of the helper left where it is installed
[[ "$(find "${WORK}" -maxdepth 1 | wc -l)" -le 3 ]]
if compgen -G "$(dirname "${HELPER}")/__pycache__/$(basename "${HELPER}")*" >/dev/null; then
    echo "loading amethystora-file-find left bytecode beside it"
    exit 1
fi
echo "test-file-find: passed"
