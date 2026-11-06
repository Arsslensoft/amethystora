#!/usr/bin/bash
# Look good in every call (R7-68, amethystora-camera) against stand-ins: which cameras it finds (a real one, never its metadata
# node, and the virtual one), a model kept only when its pinned SHA-256 holds, the settings, when PipeWire's dump says an app is
# reading, that off and `status` write nothing, and that no frame is ever written. The cost check itself needs a machine's own
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
python3 "${HELPER}" run 2>/dev/null && fail "it ran without a model"

# PipeWire's dump: an app is reading only when a video source runs
python3 - "${HELPER}" <<'PY' || fail "readers"
import sys
import importlib.util
from importlib.machinery import SourceFileLoader
loader = SourceFileLoader('camera', sys.argv[1])
camera = importlib.util.module_from_spec(importlib.util.spec_from_loader('camera', loader))
loader.exec_module(camera)
source = lambda state: {'type': 'PipeWire:Interface:Node', 'info': {'state': state, 'props': {'media.class': 'Video/Source'}}}
audio = {'type': 'PipeWire:Interface:Node', 'info': {'state': 'running', 'props': {'media.class': 'Audio/Source'}}}
assert camera.readers([source('running')])
assert not camera.readers([source('idle')]) and not camera.readers([audio]) and not camera.readers([])
assert not camera.readers({'info': 'not a list'}) and not camera.readers([None, 3, {'info': None}])
PY

# Off and idle: `status` and `set` are all that touch the account, and the folder goes with `remove`
python3 "${HELPER}" remove
[[ ! -e "${XDG_DATA_HOME}/amethystora/camera" && ! -e "${XDG_CONFIG_HOME}/amethystora/camera" ]] || fail "remove left something"

# What the code may not do: a shell, a frame written anywhere, a connection of its own
grep -q 'shell=True' "${HELPER}" && fail "a shell"
grep -qE "\.save\(|imwrite" "${HELPER}" && fail "a frame could be written"
[[ "$(grep -cE "open\([^)]*, 'w'" "${HELPER}")" == 1 ]] || fail "the helper writes more than its settings file"
for word in socket urllib requests tempfile; do
    grep -qw "${word}" "${HELPER}" && fail "the helper names ${word}"
done
echo "test-camera: ok"
