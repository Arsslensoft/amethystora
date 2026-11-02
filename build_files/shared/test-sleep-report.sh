#!/usr/bin/bash
# amethystora-sleep-report (R7-39), against a stand-in /sys and journal: what `why` names and leaves out, that `stop` and
# `allow` write one udev line of a fixed shape and set the plugged-in device, that a bad ID does nothing, and that the
# shape it writes is the one the security watcher accepts.

set -euo pipefail

REPORT="${1:-/usr/libexec/amethystora-sleep-report}"
WATCH="${2:-/usr/libexec/amethystora-security-watch}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
chmod 755 "${T}"
export AMETHYSTORA_SLEEP_ROOT="${T}/root"
S="${AMETHYSTORA_SLEEP_ROOT}/sys"
mkdir -p "${T}/bin" "${S}/class/wakeup" "${S}/bus/usb/devices" "${S}/bus/pci/devices" "${S}/bus/usb/x" "${S}/bus/pci/x"
export PATH="${T}/bin:${PATH}"

fail() { echo "test-sleep-report: $*" >&2; exit 1; }

cat >"${T}/bin/journalctl" <<'EOF'
#!/usr/bin/bash
[[ " $* " == *" MESSAGE_ID=8811e6df2a8e40f58a94cea26f8ebf14 "* ]] || exit 1
[[ -e "${JOURNAL_FAILS:-/nonexistent}" ]] && exit 1
[[ -e "${JOURNAL_EMPTY:-/nonexistent}" ]] && exit 0
printf '{"__REALTIME_TIMESTAMP":"1000000000","__MONOTONIC_TIMESTAMP":"100000000"}\n{"__REALTIME_TIMESTAMP":"3000000000","__MONOTONIC_TIMESTAMP":"300000000"}\n{"__REALTIME_TIMESTAMP":"2000000000","__MONOTONIC_TIMESTAMP":"200000000"}\nnot json\n'
EOF
chmod +x "${T}/bin/journalctl"

# source NAME DEVICE_FOLDER SUBSYSTEM EVENTS CHANGED_MS
source_of() {
    local dir="${S}/class/wakeup/$1"
    mkdir -p "${dir}" "$2" "${S}/bus/$3"
    echo "${1}" >"${dir}/name"
    echo "$4" >"${dir}/event_count"
    echo "$5" >"${dir}/last_change_ms"
    ln -s "$2" "${dir}/device"
    ln -s "${S}/bus/$3" "$2/subsystem"
}

usb() { # BUSID VENDOR PRODUCT CLASS NAME
    local dir="${S}/devices/$1"
    mkdir -p "${dir}/power"
    echo "$2" >"${dir}/idVendor"
    echo "$3" >"${dir}/idProduct"
    echo "$4" >"${dir}/bDeviceClass"
    printf '%s\n' "$5" >"${dir}/product"
    echo enabled >"${dir}/power/wakeup"
    ln -s "${dir}" "${S}/bus/usb/devices/$1"
    echo "${dir}"
}

MOUSE="$(usb 3-2 046d c52b 00 $'Fancy\tReceiver')"
RADIO="$(usb 3-9 8087 0aaa e0 'Bluetooth Radio')"
HUB="$(usb 3-1 05e3 0610 09 'Hub')"
KEYB="$(usb 3-4 1111 3333 00 'Keyboard')"
LINK="$(usb 3-5 2222 4444 00 'Visit https://a.b/c and www.example.org')"
usb usb3 1d6b 0002 09 "xHCI Host Controller" >/dev/null
OLD="$(usb 3-3 1111 2222 00 'Fired before it slept')"
mkdir -p "${S}/devices/pci0" "${S}/devices/pci1" "${S}/devices/lid"
for pair in "pci0 0x8086 0x15bb 0x020000" "pci1 0x8086 0xa0ed 0x0c0330"; do
    read -r name vendor device class <<<"${pair}"
    echo "${vendor}" >"${S}/devices/${name}/vendor"
    echo "${device}" >"${S}/devices/${name}/device"
    echo "${class}" >"${S}/devices/${name}/class"
    mkdir -p "${S}/devices/${name}/power"
    ln -s "${S}/devices/${name}" "${S}/bus/pci/devices/${name}"
done

source_of wakeup0 "${MOUSE}" usb 3 150000
source_of wakeup1 "${RADIO}" usb 1 250000
source_of wakeup2 "${HUB}" usb 1 120000
source_of wakeup3 "${OLD}" usb 9 100
source_of wakeup4 "${S}/devices/pci0" pci 1 100500
source_of wakeup5 "${S}/devices/pci1" pci 1 130000
source_of wakeup6 "${S}/devices/lid" pci 1 299000
# The key that opened the lid fires its source with the wake itself, just before the journal says the system left sleep
source_of wakeup7 "${KEYB}" usb 1 295000
source_of wakeup8 "${LINK}" usb 2 140000
# A lid has no bus of its own in the stand-in: its name says what it is
rm "${S}/devices/lid/subsystem"
echo "PNP0C0D:00" >"${S}/class/wakeup/wakeup6/name"

# The last wake was 10 seconds before "now"
export AMETHYSTORA_SLEEP_NOW_MS=310000
WHY="$(python3 -I "${REPORT}" why --json --after-ms 1000 --since 1)"
jq -e '.wakes == [1000, 2000, 3000]' <<<"${WHY}" >/dev/null || fail "the wakes, in order, without what is not JSON: ${WHY}"
jq -e '.readable == true and .last_wake_ago_s == 10' <<<"${WHY}" >/dev/null || fail "root reads the journal, the last wake 10 s ago: ${WHY}"
jq -e '[.sources[].id] | index("usb:1111:2222") == null' <<<"${WHY}" >/dev/null || fail "a source that fired before it slept was named: ${WHY}"
jq -e '[.sources[].id] | (index("lid") == null) and (index("usb:1111:3333") == null)' <<<"${WHY}" >/dev/null ||
    fail "what fired with the last wake (the lid, a key) is the person's and was named: ${WHY}"
jq -e '.sources[0].id == "usb:8087:0aaa"' <<<"${WHY}" >/dev/null || fail "devices a person can name come first, the last to fire first: ${WHY}"
jq -e '[.sources[-2:][].kind] == ["controller", "hub"]' <<<"${WHY}" >/dev/null || fail "a controller and a hub come after the devices: ${WHY}"
jq -e '[.sources[] | select(.id == "usb:046d:c52b")][0] | .can_stop == true and .kind == "usb" and .name == "Fancy Receiver"'     <<<"${WHY}" >/dev/null || fail "a USB device, its name a line of plain text: ${WHY}"
jq -e '[.sources[] | select(.id == "usb:2222:4444")][0].name | (test("/") | not) and (test("www[.]") | not) and test("Visit")' <<<"${WHY}" >/dev/null ||
    fail "a name that reads as a link: ${WHY}"
jq -e '[.sources[] | select(.id == "usb:8087:0aaa")][0] | .kind == "bluetooth"' <<<"${WHY}" >/dev/null || fail "a Bluetooth adapter: ${WHY}"
jq -e '[.sources[] | select(.id == "usb:05e3:0610")][0] | .can_stop == false and .kind == "hub"' <<<"${WHY}" >/dev/null || fail "a hub stands for all behind it: ${WHY}"
jq -e '[.sources[] | select(.id == "pci:8086:15bb")][0] | .kind == "network" and .can_stop == true' <<<"${WHY}" >/dev/null || fail "a network card: ${WHY}"
jq -e '[.sources[] | select(.id == "pci:8086:a0ed")][0] | .can_stop == false and .kind == "controller"' <<<"${WHY}" >/dev/null || fail "a USB controller stands for all on it: ${WHY}"

# Two sources of one device: the later fire is the one that counts
source_of wakeup9 "${MOUSE}" usb 1 280000
WHY="$(python3 -I "${REPORT}" why --json --after-ms 1000 --since 1)"
jq -e '.sources[0].id == "usb:046d:c52b"' <<<"${WHY}" >/dev/null || fail "the later of two fires: ${WHY}"
rm -rf "${S}/class/wakeup/wakeup9"

# A journal that cannot be read says nothing is known, not that nothing woke it
touch "${T}/journal-fails"
WHY="$(JOURNAL_FAILS="${T}/journal-fails" python3 -I "${REPORT}" why --json --after-ms 1000 --since 1)"
jq -e '.readable == false and .wakes == [] and .last_wake_ago_s == null and .sources == []' <<<"${WHY}" >/dev/null ||
    fail "a journalctl that fails: ${WHY}"
# A journal with no record: nothing is named from it either
touch "${T}/journal-empty"
WHY="$(JOURNAL_EMPTY="${T}/journal-empty" python3 -I "${REPORT}" why --json --after-ms 1000 --since 1)"
jq -e '.readable == true and .wakes == [] and .last_wake_ago_s == null and .sources == []' <<<"${WHY}" >/dev/null || fail "an empty journal: ${WHY}"

# Someone who cannot read the journal: no times, no names, and it says so
if [[ ${EUID} -eq 0 ]] && command -v setpriv >/dev/null; then
    WHY="$(setpriv --reuid=65534 --regid=65534 --clear-groups python3 -I "${REPORT}" why --json --after-ms 1000 --since 1)"
    jq -e '.readable == false and .wakes == [] and .sources == []' <<<"${WHY}" >/dev/null || fail "without the journal: ${WHY}"
fi

[[ "$(python3 -I "${REPORT}" list)" == "[]" ]] || fail "nothing is kept from waking it yet"
RULES="${AMETHYSTORA_SLEEP_ROOT}/etc/udev/rules.d/90-amethystora-wake.rules"
python3 -I "${REPORT}" stop usb:046d:c52b
python3 -I "${REPORT}" stop pci:8086:15bb
python3 -I "${REPORT}" stop usb:046d:c52b
[[ "$(<"${MOUSE}/power/wakeup")" == disabled ]] || fail "the device plugged in now is told"
[[ "$(<"${S}/devices/pci0/power/wakeup")" == disabled ]] || fail "so is a PCI one"
[[ "$(python3 -I "${REPORT}" list)" == '["usb:046d:c52b", "pci:8086:15bb"]' ]] || fail "listed once each: $(python3 -I "${REPORT}" list)"
[[ "$(grep -c '^ACTION' "${RULES}")" == 2 ]] || fail "one line each"
[[ "$(python3 -I "${REPORT}" list --names)" == $'usb:046d:c52b\tFancy Receiver\npci:8086:15bb\tpci:8086:15bb' ]] ||
    fail "Control's list, a name where one is plugged in: $(python3 -I "${REPORT}" list --names)"

# The watcher's own expression takes exactly what this writes, and nothing added to it
eval "$(sed -n '/^wake_rules_as_written() {$/,/^}$/p' "${WATCH}")"
wake_rules_as_written "${RULES}" || fail "the watcher does not accept the rules as written"
cp "${RULES}" "${T}/bad"
printf 'ACTION=="add", RUN+="/tmp/evil"\n' >>"${T}/bad"
if wake_rules_as_written "${T}/bad"; then fail "the watcher took a line it should not"; fi
sed 's/ATTR{power\/wakeup}="disabled"/ATTR{power\/wakeup}="disabled", RUN+="\/tmp\/evil"/' "${RULES}" >"${T}/bad"
if wake_rules_as_written "${T}/bad"; then fail "the watcher took a changed line"; fi

python3 -I "${REPORT}" allow usb:046d:c52b
[[ "$(<"${MOUSE}/power/wakeup")" == enabled ]] || fail "undone the same way"
[[ "$(python3 -I "${REPORT}" list)" == '["pci:8086:15bb"]' ]] || fail "the other stays"
python3 -I "${REPORT}" allow pci:8086:15bb
[[ ! -e "${RULES}" ]] || fail "no rules left, no file left"

for bad in 'usb:046D:c52b' 'usb:046d:c52b"; RUN+="x' '../etc' 'lid' ''; do
    if python3 -I "${REPORT}" stop "${bad}" 2>/dev/null; then fail "took the ID ${bad}"; fi
done
[[ ! -e "${RULES}" ]] || fail "a bad ID wrote something"

# What stands for every device behind it is never stopped, whoever asks: every root hub, a hub, a USB controller
for protected in usb:1d6b:0002 usb:1d6b:0003 usb:05e3:0610 pci:8086:a0ed; do
    if python3 -I "${REPORT}" stop "${protected}" 2>/dev/null; then fail "stopped ${protected}"; fi
done
[[ ! -e "${RULES}" ]] || fail "a refused ID wrote something"
[[ "$(<"${HUB}/power/wakeup")" == enabled ]] || fail "a refused hub was told"
python3 -I "${REPORT}" allow usb:05e3:0610 || fail "letting a hub wake it again is always allowed"

echo "test-sleep-report: fine"
