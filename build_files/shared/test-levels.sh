#!/usr/bin/bash
# The pill's sound meter taps only the stream it was given (R7-21, audit of 2026-11-02): against a private PipeWire with
# WirePlumber and a virtual microphone, pw-record with the properties pill/levels.js really uses is pointed at a stream
# that is not there (the mixer's list lags PipeWire's). It must end, and must link to nothing: without `node.dont-fallback`
# WirePlumber links such a stream to the default microphone. Run from 20-tests.sh and by hand:
#   bash build_files/shared/test-levels.sh [path to levels.js]
# Where PipeWire cannot be started (a build without the sound server's programs) it says so and passes.
set -uo pipefail

LEVELS="${1:-/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/pill/levels.js}"
[[ -r "${LEVELS}" ]] || LEVELS="$(dirname "$0")/../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/pill/levels.js"

# The properties as the meter passes them
PROPS="$(sed -n "s/^ *'-P', '\(.*\)',\$/\1/p" "${LEVELS}")"
if [[ -z "${PROPS}" ]]; then
    echo "test-levels: no properties found in ${LEVELS}"
    exit 1
fi
for needed in node.dont-fallback=true application.id=org.gnome.VolumeControl node.passive=true; do
    if [[ "${PROPS}" != *"${needed}"* ]]; then
        echo "test-levels: the meter's properties lack ${needed}"
        exit 1
    fi
done

for program in pipewire wireplumber pw-record pw-link pw-cli; do
    command -v "${program}" >/dev/null || { echo "test-levels: SKIP, no ${program}"; exit 0; }
done

RUN="$(mktemp -d)"
chmod 700 "${RUN}"
export XDG_RUNTIME_DIR="${RUN}"
PIDS=()
cleanup() {
    for pid in "${PIDS[@]}"; do kill "${pid}" 2>/dev/null; done
    wait 2>/dev/null
    rm -rf "${RUN}"
}
trap cleanup EXIT

if command -v dbus-daemon >/dev/null; then
    export DBUS_SESSION_BUS_ADDRESS="unix:path=${RUN}/bus"
    dbus-daemon --session --address="${DBUS_SESSION_BUS_ADDRESS}" --nofork --nopidfile >/dev/null 2>&1 &
    PIDS+=("$!")
fi
pipewire >"${RUN}/pipewire.log" 2>&1 &
PIDS+=("$!")
for _ in $(seq 20); do pw-cli info 0 >/dev/null 2>&1 && break; sleep 0.25; done
pw-cli info 0 >/dev/null 2>&1 || { echo "test-levels: SKIP, PipeWire did not start"; exit 0; }
wireplumber -p main >"${RUN}/wireplumber.log" 2>&1 &
PIDS+=("$!")
sleep 2
pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=fake-mic media.class=Audio/Source/Virtual object.linger=true audio.position=[MONO] node.description=FakeMic }' >/dev/null 2>&1
sleep 2
pw-link -o 2>/dev/null | grep -q "fake-mic" || { echo "test-levels: SKIP, no virtual microphone could be made"; exit 0; }

# Whether pw-record, started with these properties and a target that is not there, is linked to anything after a while
try() {
    local props="$1"
    pw-record --rate 4000 --channels 1 --format s16 --latency 150ms -P "${props}" --target 99999 - >/dev/null 2>"${RUN}/record.err" &
    local pid=$!
    sleep 4
    LINKED="$(pw-link -l 2>/dev/null | grep -c "pw-record")"
    if kill -0 "${pid}" 2>/dev/null; then ALIVE=yes; else ALIVE=no; fi
    kill "${pid}" 2>/dev/null
    wait "${pid}" 2>/dev/null
}

# The control: without the property the stand-in must show the fallback, or it proves nothing
try "${PROPS/node.dont-fallback=true/node.dont-fallback=false}"
if [[ "${LINKED}" == 0 ]]; then
    echo "test-levels: the stand-in did not show the fallback to the microphone without the property; nothing proved"
else
    echo "test-levels: control, without dont-fallback it links to the microphone (${LINKED} links)"
fi
try "${PROPS}"
if [[ "${LINKED}" != 0 ]]; then
    echo "test-levels: the meter is linked to ${LINKED} port(s) while its stream is not there: it taps the microphone"
    pw-link -l
    exit 1
fi
echo "test-levels: ok, with its own properties the meter links to nothing when its stream is gone (alive: ${ALIVE})"
