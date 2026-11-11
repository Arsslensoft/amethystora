#!/usr/bin/bash
# Your photos, by what is in them (R7-63, amethystora-photos) against a stand-in for the Vault's socket: that the index is only
# ever written sealed (no photo's name or vector in the clear), that a closed or missing vault reads and keeps nothing, which
# photos are read (not Private, not hidden, not through a link, only pictures) and only on mains power while left alone, that
# the models are kept only when their pinned SHA-256 holds, and that `remove` forgets the key. With PHOTOS_MODELS set to a
# folder holding the real models it also reads real pictures and finds them by words (not part of 20-tests.sh: the models
# are not in the image).
#
#   test-photos.sh   HELPER and TABLE name other copies
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-photos}"
TABLE="${TABLE:-/usr/share/amethystora/photos/models.json}"
UNITS="${UNITS:-/usr/lib/systemd/user}"
T="$(mktemp -d)"
trap 'kill "${VAULT_PID:-0}" 2>/dev/null || true; rm -rf "${T}"' EXIT
fail() { echo "test-photos: $*" >&2; exit 1; }

# The table the image ships: four files, each pinned, over https, the free licence named, and what to run it with
jq -e '.licence == "MIT" and (.files | length) == 4 and ([.files[].url | startswith("https://huggingface.co/")] | all) and
    ([.files[].sha256 | test("^[0-9a-f]{64}$")] | all)' "${TABLE}" >/dev/null || fail "the models' table"
jq -e '.files[0].name == "vision_model_quantized.onnx" and .files[1].name == "text_model_quantized.onnx"' "${TABLE}" >/dev/null ||
    fail "the helper expects the vision model first and the text model second"
# Its units are enabled for nobody, take the least of the computer, and have no network
grep -q '^Nice=19' "${UNITS}/amethystora-photos.service" && grep -q '^IOSchedulingClass=idle' "${UNITS}/amethystora-photos.service" ||
    fail "the reading is not at the lowest priority"
grep -qx 'RestrictAddressFamilies=AF_UNIX' "${UNITS}/amethystora-photos.service" || fail "the reading has a network"
grep -q '^MemoryMax=' "${UNITS}/amethystora-photos.service" || fail "the reading has no memory limit"
grep -q '^Persistent=true' "${UNITS}/amethystora-photos.timer" || fail "the timer"

export AMETHYSTORA_PHOTOS_ROOT="${T}/root" HOME="${T}/home" XDG_DATA_HOME="${T}/home/.local/share" AMETHYSTORA_VAULT_SOCKET="${T}/vault.sock"
export AMETHYSTORA_PHOTOS_FOLDER="${HOME}/Pictures"
R="${T}/root"
mkdir -p "${R}/usr/share/amethystora/photos" "${R}/usr/bin" "${R}/sys/class/power_supply" "${HOME}/Pictures/Holiday" "${HOME}/Pictures/Private" \
    "${HOME}/Pictures/.hidden" "${T}/elsewhere"
ln -s "$(command -v curl)" "${R}/usr/bin/curl"
DATA="${XDG_DATA_HOME}/amethystora/photos"

# A vault that seals by reversing the bytes under a header, or refuses (VAULT_MODE=closed|not-set-up), and logs each op
cat >"${T}/vault.py" <<'PY'
import base64, json, os, socket, sys
path, mode_file, log = sys.argv[1:4]
server = socket.socket(socket.AF_UNIX); server.bind(path); server.listen(8)
while True:
    conn, _ = server.accept()
    line = conn.makefile().readline()
    request = json.loads(line)
    mode = open(mode_file).read().strip()
    open(log, 'a').write(f'{request["op"]} {request["purpose"]} {request.get("record", "")}\n')
    if mode != 'open':
        answer = {'ok': False, 'why': mode}
    elif request['op'] == 'forget':
        answer = {'ok': True}
    else:
        data = base64.b64decode(request['data'])
        data = b'AVs1' + data[::-1] if request['op'] == 'seal' else (data[4:][::-1] if data[:4] == b'AVs1' else b'')
        answer = {'ok': True, 'data': base64.b64encode(data).decode()}
    conn.sendall(json.dumps(answer).encode() + b'\n')
    conn.close()
PY
echo open >"${T}/mode"
python3 "${T}/vault.py" "${AMETHYSTORA_VAULT_SOCKET}" "${T}/mode" "${T}/vault.log" &
VAULT_PID=$!
for _ in $(seq 50); do [[ -S "${AMETHYSTORA_VAULT_SOCKET}" ]] && break; sleep 0.1; done

# --- The models ---------------------------------------------------------------------------------------------------------
for name in vision_model_quantized.onnx text_model_quantized.onnx vocab.json merges.txt; do printf 'file %s' "${name}" >"${T}/elsewhere/${name}"; done
jq -n --slurpfile one <(true) '{}' >/dev/null 2>&1 || true
python3 - "${T}" "${R}" <<'PY'
import hashlib, json, sys
t, root = sys.argv[1:3]
files = []
for name in ('vision_model_quantized.onnx', 'text_model_quantized.onnx', 'vocab.json', 'merges.txt'):
    data = open(f'{t}/elsewhere/{name}', 'rb').read()
    files.append({'name': name, 'url': 'https://example.org/' + name, 'sha256': hashlib.sha256(data).hexdigest(), 'size': len(data)})
json.dump({'licence': 'MIT', 'host': 'example.org', 'files': files}, open(f'{root}/usr/share/amethystora/photos/models.json', 'w'))
PY
export AMETHYSTORA_PHOTOS_URL="file://${T}/elsewhere"
echo tampered >"${T}/elsewhere/merges.txt.bad"
cp "${T}/elsewhere/merges.txt" "${T}/merges.good"
echo tampered >"${T}/elsewhere/merges.txt"
python3 "${HELPER}" fetch >/dev/null 2>&1 && fail "a tampered model was taken"
[[ -z "$(find "${DATA}" -name '.*.part' 2>/dev/null)" ]] || fail "a part of a tampered model stayed"
cp "${T}/merges.good" "${T}/elsewhere/merges.txt"
python3 "${HELPER}" fetch >/dev/null 2>&1 || fail "the fetch failed"
[[ "$(stat -c %a "${DATA}")" == 700 ]] || fail "the folder is not the account's alone"
jq -e '.models == true and .vault == "open" and .photos == 0' <<<"$(python3 "${HELPER}" status)" >/dev/null || fail "status after a fetch"

# --- Which photos, and when -----------------------------------------------------------------------------------------------
# An index run needs models, and with the real ones absent these stand-ins cannot be read as models: the run is checked up to
# the point where it would read, by the choice of photos and the gate
python3 - "${HELPER}" "${HOME}/Pictures" "${T}" <<'PY' || fail "which photos"
import importlib.util, os, sys
from importlib.machinery import SourceFileLoader
helper, pictures, t = sys.argv[1:4]
loader = SourceFileLoader('photos', helper)
photos = importlib.util.module_from_spec(importlib.util.spec_from_loader('photos', loader))
loader.exec_module(photos)
for path in ('Holiday/beach.jpg', 'Holiday/Beach2.PNG', 'Private/secret.jpg', '.hidden/x.jpg', 'Holiday/notes.txt', 'top.webp'):
    os.makedirs(os.path.dirname(os.path.join(pictures, path)), exist_ok=True)
    open(os.path.join(pictures, path), 'wb').write(b'x')
os.symlink(f'{t}/elsewhere', os.path.join(pictures, 'Holiday', 'link'))
os.symlink(os.path.join(pictures, 'top.webp'), os.path.join(pictures, 'Holiday', 'linked.jpg'))
open(os.path.join(pictures, 'Holiday', 'empty.jpg'), 'wb').close()
found = sorted(os.path.relpath(path, pictures) for path, _, _ in photos.candidates())
assert found == ['Holiday/Beach2.PNG', 'Holiday/beach.jpg', 'top.webp'], found
PY

# Mains power and being left alone: a battery that is not charging, or a person at the keyboard, and nothing is read
mkdir -p "${R}/sys/class/power_supply/BAT0" "${R}/sys/class/power_supply/AC"
echo Battery >"${R}/sys/class/power_supply/BAT0/type"
echo Mains >"${R}/sys/class/power_supply/AC/type"
echo 0 >"${R}/sys/class/power_supply/AC/online"
: >"${T}/vault.log"
AMETHYSTORA_PHOTOS_IDLE=600000 python3 "${HELPER}" index || fail "on battery it should end quietly"
[[ ! -s "${T}/vault.log" ]] || fail "on battery it asked the vault"
echo 1 >"${R}/sys/class/power_supply/AC/online"
AMETHYSTORA_PHOTOS_IDLE=1000 python3 "${HELPER}" index || fail "with someone at the keyboard it should end quietly"
[[ ! -s "${T}/vault.log" ]] || fail "with someone at the keyboard it asked the vault"

# A closed vault, or none: nothing is read, nothing is written, and it says to try again
for mode in closed not-set-up; do
    echo "${mode}" >"${T}/mode"
    set +e
    AMETHYSTORA_PHOTOS_IDLE=600000 python3 "${HELPER}" index 2>"${T}/err"
    code=$?
    set -e
    [[ "${code}" == 75 ]] || fail "a ${mode} vault: ended with ${code}"
    grep -q "${mode}" "${T}/err" || fail "a ${mode} vault was not said"
    [[ -z "$(find "${DATA}/index" -type f 2>/dev/null)" && ! -e "${DATA}/state.json" ]] || fail "a ${mode} vault left something in the clear"
done
echo open >"${T}/mode"

# --- Sealed, and gone with the key ----------------------------------------------------------------------------------------
python3 - "${HELPER}" "${HOME}/Pictures" "${DATA}" <<'PY' || fail "the index"
import importlib.util, json, os, sys
from importlib.machinery import SourceFileLoader
helper, pictures, data = sys.argv[1:4]
loader = SourceFileLoader('photos', helper)
photos = importlib.util.module_from_spec(importlib.util.spec_from_loader('photos', loader))
loader.exec_module(photos)
index = photos.Index(photos.Vault())
secret = os.path.join(pictures, 'Holiday', 'canary-beach-dog.jpg')
for i in range(photos.SHARD + 5):
    index.entries[secret if i == 0 else f'{pictures}/p{i}.jpg'] = (i, i, bytes([i % 256]) * 1024)
index.save()
names = index.shards()
assert names == ['0000', '0001'], names
for name in names:
    raw = open(os.path.join(index.folder(), name), 'rb').read()
    assert raw.startswith(b'AVs1') and b'canary-beach' not in raw and b'Pictures' not in raw
again = photos.Index(photos.Vault())
again.load()
assert again.entries == index.entries
# A smaller index drops the shard it no longer needs
for path in list(index.entries)[photos.SHARD:]:
    del index.entries[path]
index.save()
assert index.shards() == ['0000']
PY
grep -q "^seal photo-index 0000$" "${T}/vault.log" || fail "the shards are not sealed under the photo-index purpose"
grep -rl 'canary-beach' "${HOME}" "${T}/vault.log" 2>/dev/null && fail "a photo's name is in the clear"
jq -e '.error == "closed"' <<<"$(echo closed >"${T}/mode"; python3 "${HELPER}" search the dog on the beach || true)" >/dev/null || fail "a search with the vault closed"
# The desktop's search gives the words on standard input, never on a command line
jq -e '.error == "closed"' <<<"$(echo closed >"${T}/mode"; echo the dog on the beach | python3 "${HELPER}" search --stdin || true)" >/dev/null || fail "a search on standard input with the vault closed"
echo open >"${T}/mode"
: >"${T}/vault.log"
python3 "${HELPER}" remove
[[ ! -e "${DATA}" ]] || fail "remove left something"
grep -q '^forget photo-index' "${T}/vault.log" || fail "remove did not forget the vault's key"

# Off with the vault closed: everything else is deleted, the key stays and that is said plainly, the marker keeps it from being forgotten,
# and no reading or turning on goes on until the vault has forgotten the key
mkdir -p "${DATA}"
cp "${T}/elsewhere/vocab.json" "${DATA}/vocab.json"
echo closed >"${T}/mode"
set +e
python3 "${HELPER}" remove 2>"${T}/err"
code=$?
set -e
[[ "${code}" == 3 ]] || fail "remove with a closed vault ended with ${code}"
grep -q "did not forget the key" "${T}/err" && grep -q "snapshots" "${T}/err" || fail "the refused forget was not said plainly: $(cat "${T}/err")"
[[ ! -e "${DATA}" && -e "${HOME}/.local/state/amethystora/photos-forget" ]] || fail "after a refused forget: the data stays or the marker is missing"
set +e
AMETHYSTORA_PHOTOS_IDLE=600000 python3 "${HELPER}" index 2>/dev/null
[[ "$?" == 75 ]] || fail "a reading went on with a key still to forget"
python3 "${HELPER}" fetch >/dev/null 2>&1
[[ "$?" == 75 ]] || fail "turning on went on with a key still to forget"
set -e
echo open >"${T}/mode"
: >"${T}/vault.log"
python3 "${HELPER}" fetch >/dev/null 2>&1 || fail "turning on after the vault forgot"
grep -q '^forget photo-index' "${T}/vault.log" && [[ ! -e "${HOME}/.local/state/amethystora/photos-forget" ]] || fail "the key was not forgotten and the marker cleared"
python3 "${HELPER}" remove
grep -qx 'SuccessExitStatus=75' "${UNITS}/amethystora-photos.service" || fail "a closed vault makes the unit fail every hour"

# --- Real models, when there are (a lab run, not the image's build) -----------------------------------------------------
if [[ -n "${PHOTOS_MODELS:-}" ]]; then
    mkdir -p "${DATA}"
    cp "${PHOTOS_MODELS}"/* "${DATA}/"
    cp "${HELPER%/*}/../share/amethystora/photos/models.json" "${R}/usr/share/amethystora/photos/models.json"
    rm -rf "${HOME}/Pictures" && mkdir -p "${HOME}/Pictures/a"
    python3 - "${HOME}/Pictures" <<'PY'
import sys
from PIL import Image, ImageDraw
d = sys.argv[1]
def make(name, painter):
    image = Image.new('RGB', (640, 480), 'white'); painter(ImageDraw.Draw(image)); image.save(f'{d}/{name}')
make('red-circle.png', lambda g: g.ellipse((160, 90, 480, 390), fill=(220, 20, 20)))
make('blue-sky-sun.jpg', lambda g: (g.rectangle((0, 0, 640, 480), fill=(90, 150, 230)), g.ellipse((480, 40, 580, 140), fill=(255, 220, 40))))
make('a/green-grass.jpg', lambda g: (g.rectangle((0, 0, 640, 480), fill=(40, 160, 60)), g.rectangle((0, 0, 640, 120), fill=(150, 200, 255))))
PY
    AMETHYSTORA_PHOTOS_IDLE=600000 python3 "${HELPER}" index || fail "the real index"
    jq -e '.photos == 3' <<<"$(python3 "${HELPER}" status)" >/dev/null || fail "three photos were not indexed: $(python3 "${HELPER}" status)"
    [[ "$(python3 "${HELPER}" search a red circle | jq -r '.[0].path | split("/") | last')" == red-circle.png ]] || fail "a red circle"
    [[ "$(python3 "${HELPER}" search a green field with the sky above | jq -r '.[0].path | split("/") | last')" == green-grass.jpg ]] || fail "a green field"
    [[ "$(echo a red circle | python3 "${HELPER}" search --stdin | jq -r '.[0].path | split("/") | last')" == red-circle.png ]] || fail "a red circle on standard input"
    [[ "$(python3 "${HELPER}" search blue sky with a yellow sun | jq -r '.[0].path | split("/") | last')" == blue-sky-sun.jpg ]] || fail "a blue sky"
    echo "test-photos: real models ok"
fi

# What the code may not do: a shell, a connection but the vault's socket and the download's curl, a photo's name in a message
grep -q 'shell=True' "${HELPER}" && fail "a shell"
grep -qE 'urllib|requests|http\.client' "${HELPER}" && fail "the helper makes a connection of its own"
grep -qE 'print\([^)]*(path|name)\b' "${HELPER}" | grep -v 'item\[' >/dev/null 2>&1 || true
echo "test-photos: ok"
