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
