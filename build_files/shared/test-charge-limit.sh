#!/usr/bin/bash
# Which batteries take a charge limit that changes (R7-70, /usr/libexec/amethystora-charge-limit), against stand-ins for the kernel's
# power supplies: the laptops that have the limit, a read-only one, a firmware without it, a desktop with no battery, and a mouse's battery.
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-charge-limit}"
ROOT="$(mktemp -d)"
trap 'rm -rf "${ROOT}"' EXIT

supply() {
    local folder="$1" name="$2" type="$3"
    mkdir -p "${folder}/${name}"
    printf '%s\n' "${type}" >"${folder}/${name}/type"
}
json() { python3 -I "${HELPER}" support --json --folder "$1" || true; }
check() {
    local what="$1" got="$2" wanted="$3"
    [[ "${got}" == "${wanted}" ]] || { echo "FAIL ${what}: wanted ${wanted}, got ${got}"; exit 1; }
}

# A laptop with a limit (ThinkPad-like), an AC adapter beside it
LAPTOP="${ROOT}/laptop"
supply "${LAPTOP}" AC Mains
supply "${LAPTOP}" BAT0 Battery
echo 80 >"${LAPTOP}/BAT0/charge_control_end_threshold"
echo 40 >"${LAPTOP}/BAT0/charge_control_start_threshold"
answer="$(json "${LAPTOP}")"
check "laptop supported" "$(jq -r .supported <<<"${answer}")" true
check "laptop battery" "$(jq -r .battery <<<"${answer}")" BAT0
check "laptop limit" "$(jq -r .limit <<<"${answer}")" 80
python3 -I "${HELPER}" support --folder "${LAPTOP}" | grep -q "takes a charge limit (now 80%)"
python3 -I "${HELPER}" support --folder "${LAPTOP}" >/dev/null

# A firmware without the file
NONE="${ROOT}/none"
supply "${NONE}" BAT0 Battery
check "no threshold" "$(jq -r '[.supported, .why] | join(" ")' <<<"$(json "${NONE}")")" "false no-threshold"
! python3 -I "${HELPER}" support --folder "${NONE}" >/dev/null

# A limit the kernel shows and will not take a change to
RO="${ROOT}/readonly"
supply "${RO}" BAT0 Battery
echo 100 >"${RO}/BAT0/charge_control_end_threshold"
chmod 0444 "${RO}/BAT0/charge_control_end_threshold"
check "read-only" "$(jq -r '[.supported, .why] | join(" ")' <<<"$(json "${RO}")")" "false read-only"

# A file that holds no percentage
JUNK="${ROOT}/junk"
supply "${JUNK}" BAT0 Battery
echo 'max' >"${JUNK}/BAT0/charge_control_end_threshold"
check "unreadable" "$(jq -r .why <<<"$(json "${JUNK}")")" unreadable
echo 0 >"${JUNK}/BAT0/charge_control_end_threshold"
check "out of range" "$(jq -r .why <<<"$(json "${JUNK}")")" unreadable

# Two batteries: the one that takes it is chosen
TWO="${ROOT}/two"
supply "${TWO}" BAT0 Battery
supply "${TWO}" BAT1 Battery
echo 100 >"${TWO}/BAT1/charge_control_end_threshold"
check "second battery" "$(jq -r .battery <<<"$(json "${TWO}")")" BAT1

# A desktop: no battery, and a folder that is not there at all
DESK="${ROOT}/desk"
supply "${DESK}" AC Mains
check "desktop" "$(jq -r '[.supported, .why] | join(" ")' <<<"$(json "${DESK}")")" "false no-battery"
check "no folder" "$(jq -r .why <<<"$(json "${ROOT}/absent")")" no-battery

# A mouse's battery is not the computer's, whatever it has
PERIPHERAL="${ROOT}/peripheral"
supply "${PERIPHERAL}" hidpp_battery_0 Battery
echo Device >"${PERIPHERAL}/hidpp_battery_0/scope"
echo 80 >"${PERIPHERAL}/hidpp_battery_0/charge_control_end_threshold"
check "peripheral" "$(jq -r .why <<<"$(json "${PERIPHERAL}")")" no-battery

# Nothing is ever written
before="$(find "${LAPTOP}" -type f -exec sha256sum {} + | sort)"
python3 -I "${HELPER}" support --json --folder "${LAPTOP}" >/dev/null
[[ "$(find "${LAPTOP}" -type f -exec sha256sum {} + | sort)" == "${before}" ]]

# Anything else is a usage error
! python3 -I "${HELPER}" set 80 2>/dev/null
echo "test-charge-limit: ok"
