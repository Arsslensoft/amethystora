#!/usr/bin/bash
# What amethystora-driver-status check says, for 20-tests.sh, on a stand-in machine and with a stand-in for
# amethystora-notify-users: what was said is not said again, a finding that reached nobody is said later, and a
# finding without a fix offers the manual and never a terminal.

set -euo pipefail

T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
fail() { echo "test-driver-status: $*" >&2; exit 1; }
HELPER="${AMETHYSTORA_DRIVER_STATUS:-/usr/libexec/amethystora-driver-status}"

mkdir -p "${T}/root/proc" "${T}/root/usr/share/amethystora" "${T}/root/sys/bus/usb/devices" "${T}/root/sys/module" "${T}/bin"
echo 'BOOT_IMAGE=/vmlinuz root=UUID=0 rw' >"${T}/root/proc/cmdline"
echo '{"image-name": "amethystora", "image-flavor": "main", "image-tag": "stable"}' >"${T}/root/usr/share/amethystora/image-info.json"
: >"${T}/log"
: >"${T}/mok"
# Writes down what it was asked to say, and says it reached nobody while ${T}/nobody exists
cat >"${T}/notify" <<STANDIN
#!/usr/bin/bash
printf '%s\n' "\$*" >>"${T}/said"
[[ ! -e "${T}/nobody" ]]
STANDIN
chmod +x "${T}/notify"
export AMETHYSTORA_DRIVERS_ROOT="${T}/root" AMETHYSTORA_DRIVERS_LOG="${T}/log" AMETHYSTORA_DRIVERS_MOK="${T}/mok" \
    AMETHYSTORA_DRIVERS_STATE="${T}/state" AMETHYSTORA_DRIVERS_NOTIFY="${T}/notify"
check() { : >"${T}/said"; "${HELPER}" check 2>/dev/null; }

# A finding with no fix: its manual, and the fix field empty rather than the manual's section
echo 'Loading of module with unavailable key is rejected' >"${T}/log"
echo "Failed to insert module 'v4l2loopback': Key was rejected by service" >>"${T}/log"
touch "${T}/nobody"
check
grep -q 'refused:v4l2loopback' "${T}/state/said" && fail "a finding that reached nobody counts as said"
grep -q 'manual=Read more' "${T}/said" || fail "a finding without a fix does not offer the manual: $(cat "${T}/said")"
grep -q 'fix=' "${T}/said" && fail "a finding without a fix offers one: $(cat "${T}/said")"
rm "${T}/nobody"
check
[[ -s "${T}/said" ]] || fail "a finding that reached nobody is not said at the next check"
grep -qx 'refused:v4l2loopback' "${T}/state/said" || fail "a finding that reached somebody is not written down"
check
[[ ! -s "${T}/said" ]] || fail "a finding is said twice: $(cat "${T}/said")"

# A DisplayLink dock: what each state says and offers, and when it is said again
rm -rf "${T}/state"
: >"${T}/log"
mkdir -p "${T}/root/sys/bus/usb/devices/1-1" "${T}/root/sys/firmware/efi/efivars"
printf '17e9\n' >"${T}/root/sys/bus/usb/devices/1-1/idVendor"
printf 'Test Dock\n' >"${T}/root/sys/bus/usb/devices/1-1/product"
cat >"${T}/bin/systemctl" <<STANDIN
#!/usr/bin/bash
[[ -e "${T}/service-runs" ]]
STANDIN
chmod +x "${T}/bin/systemctl"
PATH="${T}/bin:${PATH}"
KEYS=/usr/share/amethystora/secure-boot
VAR="${T}/root/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c"
machine() { # SECURE_BOOT(0|1|none) STATE_OF_EVDI'S_KEY LOADED(yes|no) CMDLINE [SEALED]
    rm -f "${VAR}" "${T}/root/usr/share/amethystora/sealed"
    [[ "$1" == none ]] || printf "\\x06\\x00\\x00\\x00\\x0$1" >"${VAR}"
    # The other key enrolled or not must not decide anything
    printf 'enrolled %s/akmods-amethystora.der\n%s %s/amethystora-modules.der\n' "${KEYS}" "$2" "${KEYS}" >"${T}/mok"
    rm -rf "${T}/root/sys/module/evdi"
    [[ "$3" == no ]] || mkdir -p "${T}/root/sys/module/evdi"
    echo "$4" >"${T}/root/proc/cmdline"
    [[ -z "${5:-}" ]] || touch "${T}/root/usr/share/amethystora/sealed"
}
expect() { # WHAT ID FIX
    local got
    got="$("${HELPER}" --json | jq -r '.findings[] | select(.kind == "displaylink") | "\(.id)|\(.fix // "")"')"
    [[ "${got}" == "${2:+$2|$3}" ]] || fail "$1: expected $2|$3, got ${got}"
    [[ -z "$2" ]] || [[ "$("${HELPER}" --json | jq -r '.findings[] | select(.kind == "displaylink") | .text | length')" -lt 400 ]] ||
        fail "$1: amethystora-notify-users cuts a text at 400 characters"
}
LOCK="BOOT_IMAGE=/vmlinuz lockdown=integrity rw"
PLAIN="BOOT_IMAGE=/vmlinuz rw"
machine 1 not-enrolled no "${PLAIN}"
expect "Secure Boot on, key not enrolled" displaylink:needs-enrollment "ame security secure-boot"
machine 1 pending no "${PLAIN}"
expect "Secure Boot on, key queued" displaylink:enrollment-pending ""
machine 1 enrolled no "${PLAIN}"
expect "Secure Boot on, key enrolled, refused all the same" displaylink:refused ""
machine 0 not-enrolled no "${LOCK}"
expect "Secure Boot off, lockdown: enrolling cannot help" displaylink:lockdown "ame security lockdown off"
machine none unknown no "${LOCK}"
expect "no EFI, lockdown: enrolling cannot help" displaylink:lockdown "ame security lockdown off"
machine 0 not-enrolled no "${LOCK}" sealed
expect "a sealed image without Secure Boot: its arguments cannot change" displaylink:sealed-no-secure-boot ""
machine 0 not-enrolled no "${PLAIN}"
expect "Secure Boot off, no lockdown, and yet not loaded" displaylink:refused ""
machine 1 not-enrolled yes "${PLAIN}"
expect "loaded, the service stopped" displaylink:service ""
touch "${T}/service-runs"
machine 0 not-enrolled yes "${PLAIN}"
expect "loaded and running, as on a machine without Secure Boot or lockdown" "" ""
machine 1 enrolled yes "${PLAIN}"
expect "loaded and running, the key enrolled" "" ""

# The module refused by name beside the dock is said; refused with no name, it is the dock's
machine 0 not-enrolled no "${LOCK}"
echo 'Lockdown: modprobe: unsigned module loading is restricted; see man kernel_lockdown.7' >"${T}/log"
[[ "$("${HELPER}" --json | jq -r '[.findings[].kind] | join(" ")')" == displaylink ]] ||
    fail "evdi refused is said twice: $("${HELPER}" --json)"
: >"${T}/log"

# Once per state, in the words of a message, with the button that starts its own command
machine 1 not-enrolled no "${PLAIN}"
check
grep -q -- '--msgid .*A device does not work' "${T}/said" || fail "a dock's text is not a message: $(cat "${T}/said")"
grep -q 'fix=Enroll the key…' "${T}/said" || fail "no button to enroll: $(cat "${T}/said")"
check
[[ ! -s "${T}/said" ]] || fail "the same state is said twice"
# Unplugged and plugged in again, nothing has changed
rm -rf "${T}/root/sys/bus/usb/devices/1-1.saved"
mv "${T}/root/sys/bus/usb/devices/1-1" "${T}/root/sys/bus/usb/devices/1-1.saved"
check
mv "${T}/root/sys/bus/usb/devices/1-1.saved" "${T}/root/sys/bus/usb/devices/1-1"
check
[[ ! -s "${T}/said" ]] || fail "plugging the dock in again says the same state again"
machine 1 pending no "${PLAIN}"
check
grep -q 'enrollment-pending\|accept the driver' "${T}/said" || fail "a new state is not said: $(cat "${T}/said")"
machine 1 not-enrolled no "${PLAIN}"
check
grep -q 'fix=Enroll the key…' "${T}/said" || fail "back to a state after another is not said: $(cat "${T}/said")"
machine 0 not-enrolled no "${LOCK}"
check
grep -q 'fix=Turn lockdown off…' "${T}/said" || fail "lockdown offers no button of its own: $(cat "${T}/said")"
grep -q 'Enroll the key' "${T}/said" && fail "lockdown offers enrolling: $(cat "${T}/said")"
# Each button starts its own command in the session of who pressed it, never the other's
cat >"${T}/bin/runuser" <<STANDIN
#!/usr/bin/bash
printf '%s\n' "\$*" >>"${T}/ran"
STANDIN
chmod +x "${T}/bin/runuser"
cat >"${T}/notify-answers" <<STANDIN
#!/usr/bin/bash
echo "$(id -un) \$(cat "${T}/answer")"
STANDIN
chmod +x "${T}/notify-answers"
ran_on_answer() { # ANSWER
    echo "$1" >"${T}/answer"
    : >"${T}/ran"
    rm -rf "${T}/state"
    AMETHYSTORA_DRIVERS_NOTIFY="${T}/notify-answers" "${HELPER}" check 2>/dev/null
    cat "${T}/ran"
}
machine 1 not-enrolled no "${PLAIN}"
ran_on_answer fix | grep -q 'amethystora-in-terminal ame security secure-boot$' || fail "the button does not open the enrollment in a terminal"
ran_on_answer manual | grep -q 'amethystora-manual hardware#secure-boot$' || fail "Read more does not open the manual"
[[ -z "$(ran_on_answer "")" ]] || fail "a notice closed without an answer starts something"

machine 1 enrolled yes "${PLAIN}"
check
grep -q displaylink "${T}/state/said" && fail "a dock that works leaves a state said"
[[ ! -s "${T}/said" ]] || fail "a dock that works is told about: $(cat "${T}/said")"
exit 0
