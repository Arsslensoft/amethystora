#!/usr/bin/bash
# Look good in every call (R7-68, amethystora-camera) against stand-ins: which cameras it finds (a real one, never its metadata
# node, and the virtual one), a model kept only when its pinned SHA-256 holds, the settings, when PipeWire's dump says an app is
# reading (a graph kept from each change, and the programs that open the device themselves), that off and `status` write nothing, and that no frame is ever written. The cost check itself needs a machine's own
# processor and is run by `ame desktop camera on`.
#
#   test-camera.sh   HELPER and TABLE name other copies of the helper and the table
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-camera}"
TABLE="${TABLE:-/usr/share/amethystora/camera/models.json}"
UNIT="${UNIT:-/usr/lib/systemd/user/amethystora-camera.service}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
fail() { echo "test-camera: $*" >&2; exit 1; }

# The table the image ships: one model, over https, pinned, under a free licence, whose host is the only one it names
jq -e '.model.url | startswith("https://media.githubusercontent.com/")' "${TABLE}" >/dev/null || fail "the model's address"
jq -e '.model.host == "media.githubusercontent.com" and .model.licence == "Apache-2.0" and (.model.sha256 | test("^[0-9a-f]{64}$"))' \
    "${TABLE}" >/dev/null || fail "the model is not pinned, or not under a free licence"
jq -e '.model.size > 1000000 and .model.size < 20000000' "${TABLE}" >/dev/null || fail "the model's size"
# Its unit is enabled for nobody, has no network and a share of the computer it cannot go over
grep -q '^WantedBy=graphical-session.target' "${UNIT}" || fail "the unit's install section"
grep -qx 'RestrictAddressFamilies=AF_UNIX' "${UNIT}" || fail "the unit has a network"
grep -q '^CPUQuota=' "${UNIT}" && grep -q '^MemoryMax=' "${UNIT}" || fail "the unit has no limits"

export AMETHYSTORA_CAMERA_ROOT="${T}/root" HOME="${T}/home" XDG_DATA_HOME="${T}/home/.local/share" XDG_CONFIG_HOME="${T}/home/.config"
R="${T}/root"
mkdir -p "${R}/usr/share/amethystora/camera" "${R}/usr/bin" "${R}/sys/class/video4linux" "${HOME}"
cp "${TABLE}" "${R}/usr/share/amethystora/camera/models.json"
ln -s "$(command -v curl)" "${R}/usr/bin/curl"
printf '#!/usr/bin/bash\n[[ "${FAKE_RUNNING:-}" == yes ]]\n' >"${R}/usr/bin/systemctl"
chmod +x "${R}/usr/bin/systemctl"
node() { # NAME INDEX PLACE
    mkdir -p "${R}/sys/devices/$3/$1"
    echo "$2" >"${R}/sys/devices/$3/$1/index"
    ln -s "../../devices/$3/$1" "${R}/sys/class/video4linux/$1"
}
node video0 0 virtual/video4linux
node video1 0 pci0000:00/usb1/1-1
node video2 1 pci0000:00/usb1/1-1
node video3 0 pci0000:00/usb1/1-2

# What it finds: the loopback device, and each real camera's capture node only
info="$(python3 "${HELPER}" status 2>&1)" || { echo "${info}"; fail "status"; }
jq -e '.virtual == ["/dev/video0"] and .real == ["/dev/video1", "/dev/video3"]' <<<"${info}" >/dev/null || fail "the cameras: ${info}"

# The model: kept only when it is the pinned file
printf 'a model' >"${T}/model.onnx"
jq -n --arg sha "$(sha256sum "${T}/model.onnx" | cut -d' ' -f1)" \
    '{model: {url: "https://example.org/m.onnx", host: "example.org", sha256: $sha, size: 7}}' >"${R}/usr/share/amethystora/camera/models.json"
export AMETHYSTORA_CAMERA_URL="file://${T}/model.onnx"
echo tampered >"${T}/other.onnx"
AMETHYSTORA_CAMERA_URL="file://${T}/other.onnx" python3 "${HELPER}" fetch >/dev/null 2>&1 && fail "a tampered model was taken"
[[ -z "$(find "${XDG_DATA_HOME}" -type f 2>/dev/null)" ]] || fail "a tampered model left something"
python3 "${HELPER}" fetch >/dev/null 2>&1 || fail "the fetch failed"
[[ "$(stat -c %a "${XDG_DATA_HOME}/amethystora/camera")" == 700 ]] || fail "the folder is not the account's alone"
jq -e '.model == true and .blur == true and .light == true' <<<"$(python3 "${HELPER}" status)" >/dev/null || fail "status after a fetch"

# The two effects: kept in a file of the account's own, and nothing else is a setting
python3 "${HELPER}" set blur off || fail "set"
jq -e '.blur == false and .light == true' <<<"$(python3 "${HELPER}" status)" >/dev/null || fail "blur off"
python3 "${HELPER}" set sharpen on 2>/dev/null && fail "a setting that is not one"
python3 "${HELPER}" set light maybe 2>/dev/null && fail "a value that is not one"
[[ "$(cat "${XDG_CONFIG_HOME}/amethystora/camera")" == $'blur=off\nlight=on' ]] || fail "the settings file"
FAKE_RUNNING=yes jq -e '.running == true' <<<"$(FAKE_RUNNING=yes python3 "${HELPER}" status)" >/dev/null || fail "running"

# Without the model the cost check says so, and without the unit's switch nothing runs
rm "${XDG_DATA_HOME}/amethystora/camera/model.onnx"
jq -e '.error == "no-model"' <<<"$(python3 "${HELPER}" bench || true)" >/dev/null || fail "bench without a model"
set +e
python3 "${HELPER}" run 2>/dev/null
[[ "$?" == 78 ]] || fail "without a model it must end with 78, which the unit does not restart on"
# No camera: it ends with 69 and the unit does not start it again (a start every few seconds, for as long as the session lasts)
printf 'a model' >"${XDG_DATA_HOME}/amethystora/camera/model.onnx"
AMETHYSTORA_CAMERA_ROOT="${T}/nocamera" python3 "${HELPER}" run 2>/dev/null
[[ "$?" == 69 ]] || fail "without a camera it must end with 69"
set -e
grep -qx 'RestartPreventExitStatus=69 78' "${UNIT}" || fail "the unit starts it again without a camera"
rm "${XDG_DATA_HOME}/amethystora/camera/model.onnx"

# PipeWire's graph: `pw-dump --monitor` prints the whole graph once and then only what changed, so a reader is told from a graph
# that is kept and moved by each change, and only for the virtual camera's own node
python3 - "${HELPER}" <<'PY' || fail "the graph of readers"
import sys, importlib.util
from importlib.machinery import SourceFileLoader
loader = SourceFileLoader('camera', sys.argv[1])
camera = importlib.util.module_from_spec(importlib.util.spec_from_loader('camera', loader))
loader.exec_module(camera)
NODE = 'PipeWire:Interface:Node'
def node(ident, state, path, klass='Video/Source'):
    return {'id': ident, 'type': NODE, 'info': {'state': state, 'props': {'media.class': klass, 'api.v4l2.path': path}}}
audio = {'id': 80, 'type': NODE, 'info': {'state': 'running', 'props': {'media.class': 'Audio/Source'}}}
g = camera.Graph()
assert not g.reading('/dev/video0')
g.update([node(52, 'idle', '/dev/video0'), node(53, 'idle', '/dev/video2'), audio])
assert not g.reading('/dev/video0')
# An app starts reading the virtual camera: a change of that node's state, which carries no properties at all
g.update([{'id': 52, 'type': NODE, 'info': {'state': 'running'}}])
assert g.reading('/dev/video0')
# A change that is not about the camera (a volume key, a call's audio) must not end it
g.update([{'id': 80, 'type': NODE, 'info': {'state': 'idle', 'props': {'media.class': 'Audio/Source'}}}])
g.update([{'id': 99, 'type': 'PipeWire:Interface:Port', 'info': {'direction': 'output'}}, 'junk', None, {'id': 'x'}])
assert g.reading('/dev/video0')
# Another camera's node running is not this one's reader
g.update([{'id': 52, 'type': NODE, 'info': {'state': 'idle'}}, {'id': 53, 'type': NODE, 'info': {'state': 'running'}}])
assert not g.reading('/dev/video0') and g.reading('/dev/video2')
# The node goes, and what is not a list is nothing
g.update([{'id': 53, 'info': None}])
assert not g.reading('/dev/video2')
g.update({'info': 'not a list'})
assert not g.nodes.get(53)
# A reader found by its object path, as a node without the V4L2 property has it
g.update([{'id': 60, 'type': NODE, 'info': {'state': 'running', 'props': {'media.class': 'Video/Source', 'object.path': 'v4l2:/dev/video0'}}}])
assert g.reading('/dev/video0')
PY

# A program that opens the device itself (V4L2, as most browsers do) is a reader too, found from /proc; this process is not
: >"${T}/device"
python3 - "${HELPER}" "${T}/device" <<'PY' || fail "the openers"
import sys, subprocess, time, importlib.util
from importlib.machinery import SourceFileLoader
loader = SourceFileLoader('camera', sys.argv[1])
camera = importlib.util.module_from_spec(importlib.util.spec_from_loader('camera', loader))
loader.exec_module(camera)
device = sys.argv[2]
mine = open(device)
assert not camera.openers(device), 'this process counted as a reader'
child = subprocess.Popen([sys.executable, '-c', 'import sys, time; f = open(sys.argv[1]); time.sleep(30)', device])
for _ in range(50):
    if camera.openers(device):
        break
    time.sleep(0.1)
assert camera.openers(device), 'a program with the device open was not found'
child.kill(); child.wait()
assert not camera.openers(device), 'a program that is gone still counts'
# ...and the open and the close of it are told, by inotify, on a file and on a device node
from gi.repository import GLib
for target in (device, '/dev/null'):
    loop = GLib.MainLoop(); told = []
    assert camera.watch_opens(target, lambda: (told.append(1), loop.quit()), GLib) is not None
    GLib.timeout_add(100, lambda: (open(target).close(), GLib.SOURCE_REMOVE)[1])
    GLib.timeout_add(3000, loop.quit)
    loop.run()
    assert told, f'no event for an open of {target}'
PY

# Off and idle: `status` and `set` are all that touch the account, and the folder goes with `remove`
python3 "${HELPER}" remove
[[ ! -e "${XDG_DATA_HOME}/amethystora/camera" && ! -e "${XDG_CONFIG_HOME}/amethystora/camera" ]] || fail "remove left something"

# What the code may not do: a shell, a frame written anywhere, a connection of its own
grep -q 'shell=True' "${HELPER}" && fail "a shell"
# (the one .save is the benchmark's own test picture, into memory)
[[ -z "$(grep -E "\.save\(|imwrite" "${HELPER}" | grep -v "\.save(out, 'JPEG'")" ]] || fail "a frame could be written"
[[ "$(grep -cE "open\([^)]*, 'w'" "${HELPER}")" == 1 ]] || fail "the helper writes more than its settings file"
for word in socket urllib requests tempfile; do
    grep -qw "${word}" "${HELPER}" && fail "the helper names ${word}"
done
echo "test-camera: ok"
