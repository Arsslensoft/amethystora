#!/usr/bin/bash
# vmlab-vm: installer
# Drivers (plans/done/PLAN-driver-compatibility.md), on the machine installed as the installer lays a disk out,
# whose /boot keeps a staged deployment through a restart, since two of these change the kernel's arguments:
# - The driver check: its timer, the status it saves for everyone, nothing found on a machine whose devices all
#   have their driver, and a module the kernel refused found as one, from the kernel's own words.
# - FireWire's switch (ame security firewire): off lets the modules load and leaves the other blocked ones blocked,
#   on blocks them again; the report says it is off.
# - The rebase helper's Secure Boot guard: no switch to an NVIDIA image while the key its driver needs is not
#   enrolled (mokutil stood in for).
# - The Xbox wireless adapter (ame apps xbox-adapter): Microsoft's firmware downloaded and checked, the blacklist
#   and the firmware path, which the watcher accepts exactly; after a restart the kernel looks there first; off
#   takes all of it away again.
# - Waiting firmware updates said once by the update alert (fwupdmgr and notify-send stood in for), and the
#   report's firmware check.
# vmlab-paths: @sf/usr/libexec/amethystora-driver-* @sf/usr/libexec/amethystora-gpu-check @sf/usr/lib/modprobe.d/*
source "$(dirname "$0")/lib.sh"

STATE=/var/lib/vmlab-tests/49-drivers
mkdir -p "${STATE}"
FIRMWARE=/var/lib/amethystora/firmware
KARG="firmware_class.path=${FIRMWARE}"
PINS=/usr/share/amethystora/xone/firmware
# Stand-ins the account runs, where it can reach them: the state folder above may be root's alone
STANDIN=/var/tmp/vmlab-49-drivers
install -d -m 0755 "${STANDIN}"

# The account, as 10-account.sh signs one in on the other machine: this one has none until a suite makes it
if ! getent passwd "${TEST_USER}" >/dev/null; then
    useradd -m -G wheel -c "Test account" "${TEST_USER}"
    echo "${TEST_USER}:${TEST_USER}" | chpasswd
    printf '[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=%s\n' "${TEST_USER}" >/etc/gdm/custom.conf
    install -d -o "${TEST_USER}" -g "${TEST_USER}" "/var/home/${TEST_USER}/.config"
    install -o "${TEST_USER}" -g "${TEST_USER}" /dev/null "/var/home/${TEST_USER}/.config/gnome-initial-setup-done"
    systemctl restart gdm
fi
UID_TEST="$(id -u "${TEST_USER}")"
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}" "$(loginctl list-sessions --no-legend)"
    finish
fi
PASSWORD=('\[sudo\] password for' "${TEST_USER}"$'\r')
runuser -u "${TEST_USER}" -- sudo -n true 2>/dev/null && PASSWORD=()

session_terminal() {
    local steps=()
    while [[ $# -gt 0 && "$1" != -- ]]; do steps+=("$1"); shift; done
    shift
    in_terminal "${steps[@]}" -- runuser -u "${TEST_USER}" -- env XDG_RUNTIME_DIR="/run/user/${UID_TEST}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${UID_TEST}/bus" \
        systemd-run --user --pty --wait --quiet --collect -- /usr/bin/bash -lc "$*"
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
record() { /usr/libexec/amethystora-security-status --json 2>/dev/null | jq -c --arg id "$1" '[.. | objects | select(.id? == $id)][0]'; }
# The kernel arguments of the deployment the next restart starts
next_args() { rpm-ostree kargs 2>/dev/null; }
# The drift the security watcher saw at a run started now
drift() { systemctl start amethystora-security-watch.service; jq -c '.drift // []' /var/lib/amethystora/security/status.json; }

# --- Phase 1: the driver check, FireWire, the guard, the adapter set up ------------------------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    same "the driver check's timer is enabled" "$(systemctl is-enabled amethystora-driver-check.timer)" enabled
    started="$(date +%s)"
    systemctl start --no-block amethystora-driver-check.service
    saved() { [[ "$(jq -r '.checked // 0' /var/lib/amethystora/drivers/status.json 2>/dev/null)" -ge "${started}" ]]; }
    if wait_for 120 saved; then
        same "it saves what it found for everyone" "$(stat -c '%a %U' /var/lib/amethystora/drivers/status.json)" "644 root"
        same "nothing is missing on this machine, whose devices all have their driver" \
            "$(jq -c '.findings' /var/lib/amethystora/drivers/status.json)" "[]"
        as_tester="$(as_user "ame system drivers --json" 2>&1)"
        same "ame system drivers, without root, says what the check saved" "$(jq -c '.findings' <<<"${as_tester}" 2>&1)" "[]"
    else
        not_ok "the driver check saved nothing" "$(journalctl -b -u amethystora-driver-check.service --no-pager -o cat | tail -n 10)"
    fi

    # A module the kernel refuses: evdi, which a lab build signs with a throwaway key that is not enrolled
    if modprobe evdi 2>"${STATE}/evdi.err"; then
        modprobe -r evdi
        skip "a refused module: evdi loaded, so this build signed it with a key the machine trusts"
    else
        found="$(/usr/libexec/amethystora-driver-status --json)"
        if jq -e 'any(.findings[]; .kind == "refused")' <<<"${found}" >/dev/null; then
            ok "a module the kernel refused is found, from the kernel's own words ($(cat "${STATE}/evdi.err"))"
        else
            not_ok "the refused evdi is not found" "${found}" "$(journalctl -k -b --no-pager -o cat -g 'rejected|Lockdown' | tail -n 5)"
        fi
    fi

    # A DisplayLink dock, on a stand-in machine (the lab has no such device): what each state offers, and that the
    # machine itself, with none plugged in, has nothing to say about one
    same "a dock's arrival starts the check" "$(udevadm verify /usr/lib/udev/rules.d/99-amethystora-displaylink.rules >/dev/null 2>&1; echo $?)" 0
    same "the check runs after the service that tries to load its driver" \
        "$(systemctl show -P After amethystora-driver-check.service | grep -c displaylink.service)" 1
    same "no dock, nothing about one" "$(jq -c '[.findings[] | select(.kind == "displaylink")]' /var/lib/amethystora/drivers/status.json)" "[]"
    dock="$(mktemp -d)"
    mkdir -p "${dock}/sys/bus/usb/devices/1-1" "${dock}/proc" "${dock}/sys/firmware/efi/efivars"
    printf '17e9\n' >"${dock}/sys/bus/usb/devices/1-1/idVendor"
    dock_fix() { # SECURE_BOOT MOK_STATE CMDLINE
        printf "\\x06\\x00\\x00\\x00\\x0$1" >"${dock}/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c"
        echo "$3" >"${dock}/proc/cmdline"
        echo "$2 /usr/share/amethystora/secure-boot/amethystora-modules.der" >"${dock}/mok"
        : >"${dock}/log"
        AMETHYSTORA_DRIVERS_ROOT="${dock}" AMETHYSTORA_DRIVERS_LOG="${dock}/log" AMETHYSTORA_DRIVERS_MOK="${dock}/mok" \
            /usr/libexec/amethystora-driver-status --json | jq -r '.findings[] | select(.kind == "displaylink") | "\(.id) \(.fix // "")"'
    }
    same "Secure Boot on, the key not enrolled: enrolling is offered" "$(dock_fix 1 not-enrolled "")" \
        "displaylink:needs-enrollment ame security secure-boot"
    same "Secure Boot off under lockdown: only lockdown is offered" "$(dock_fix 0 not-enrolled "lockdown=integrity")" \
        "displaylink:lockdown ame security lockdown off"
    rm -rf "${dock}"

    # FireWire, a switch of its own: off, its modules load, and the other blocked modules stay blocked
    /usr/libexec/amethystora-hardening set firewire off
    same "off masks its file" "$(readlink /etc/modprobe.d/61-amethystora-firewire.conf)" /dev/null
    if modprobe -n -v firewire_core 2>&1 | grep -q 'install /usr/bin/false'; then not_ok "firewire_core is still blocked"; else
        ok "firewire_core can load"; fi
    if modprobe -n -v dccp 2>&1 | grep -q 'install /usr/bin/false'; then ok "dccp is still blocked"; else not_ok "dccp is no longer blocked"; fi
    if modinfo firewire_core >/dev/null 2>&1; then
        check "firewire_core loads" modprobe firewire_core
    else
        skip "loading firewire_core: this kernel has no FireWire modules"
    fi
    same "the report says FireWire was turned off, with the command that turns it on" \
        "$(record firewire | jq -c '[.state, .switched_off, .command]')" '["info",true,"ame security firewire on"]'
    /usr/libexec/amethystora-hardening set firewire on
    same "on takes the mask away" "$(readlink /etc/modprobe.d/61-amethystora-firewire.conf || echo none)" none
    if lsmod | grep -q '^firewire_core '; then not_ok "firewire_core is still loaded"; else ok "on unloads FireWire, which nothing used"; fi
    same "the report has nothing to say about FireWire on" "$(record firewire)" null

    # The rebase helper: no NVIDIA image while Secure Boot is on and the key its driver is signed with is not
    # enrolled, which would start to a black screen
    install -d -m 0755 "${STANDIN}/bin"
    cat >"${STANDIN}/bin/mokutil" <<'EOF'
#!/usr/bin/bash
case "$1" in
    --sb-state) echo "SecureBoot enabled" ;;
    --test-key) echo "$2 is not enrolled" ;;
esac
EOF
    chmod 0755 "${STANDIN}/bin/mokutil"
    out="$(session_terminal 120 'Select your image' $'\r' 'Enroll the keys now' n -- \
        "PATH=${STANDIN}/bin:\$PATH /usr/bin/amethystora-rollback-helper amethystora-nvidia-lts stable")"
    if [[ $? != 0 && "${out}" == *"would start to a black screen"* && "${out}" != *"Confirm rebase"* ]]; then
        ok "with Secure Boot on and the key not enrolled, the rebase helper offers to enroll it instead of switching"
    else
        not_ok "the rebase helper's Secure Boot guard" "$(tail -n 15 <<<"${out}")"
    fi

    # The Xbox wireless adapter: Microsoft's firmware, checked against the image's pins, and the path to it
    out="$(session_terminal 900 'Set up the Xbox wireless adapter\?' y "${PASSWORD[@]}" -- "ame apps xbox-adapter on")"
    if [[ $? == 0 ]]; then ok "ame apps xbox-adapter on"; else not_ok "ame apps xbox-adapter on" "$(tail -n 15 <<<"${out}")"; fi
    while read -r id _ _ _ sum; do
        same "the firmware of adapter ${id} is Microsoft's, as pinned" \
            "$(sha256sum "${FIRMWARE}/xone_dongle_${id}.bin" 2>&1 | cut -d' ' -f1)" "${sum}"
    done < <(grep -v '^#' "${PINS}")
    same "the folder is root's alone" "$(stat -c '%a %U' "${FIRMWARE}")" "755 root"
    same "mt76x2u, which would take the adapter, is blacklisted" \
        "$(modprobe --showconfig | grep -x 'blacklist mt76x2u')" "blacklist mt76x2u"
    if grep -qw "${KARG}" <<<"$(next_args)"; then ok "the next start has ${KARG}"; else not_ok "the next start's arguments" "$(next_args)"; fi
    same "the security watcher takes the adapter's files as they are" \
        "$(drift | jq -c '[.[] | select(.path | test("xone|/var/lib/amethystora/firmware|firmware_class"))]')" "[]"
    # Anything else in the folder replaces the image's firmware of that name for every device: a bad regulatory
    # database, which the next phase also uses to see that the kernel looks there first
    head -c 64 /dev/urandom >"${FIRMWARE}/regulatory.db"
    same "the watcher flags any other file there" \
        "$(drift | jq -c '[.[] | select(.path == "/var/lib/amethystora/firmware/regulatory.db") | .change]')" '["replaces"]'
    reboot_and_continue
fi

# --- Phase 2: the kernel looks there first; then the adapter taken away -----------------------------------------
if [[ "${SUITE_PHASE}" == 2 ]]; then
    if grep -qw "${KARG}" /proc/cmdline; then ok "the machine started with ${KARG}"; else not_ok "the arguments it started with" "$(cat /proc/cmdline)"; fi
    same "the kernel's firmware path is the folder" "$(cat /sys/module/firmware_class/parameters/path)" "${FIRMWARE}"
    # cfg80211 reads the regulatory database as firmware when it loads: the bad one in the folder, before the
    # image's own good one, shows in what it says
    if lsmod | grep -q '^cfg80211 '; then
        skip "the folder read first: cfg80211 was already loaded"
    else
        since="$(date '+%Y-%m-%d %H:%M:%S')"
        modprobe cfg80211
        loaded() { journalctl -k --since "${since}" --no-pager -o cat | grep -q 'regulatory.db'; }
        wait_for 30 loaded
        said="$(journalctl -k --since "${since}" --no-pager -o cat | grep 'regulatory')"
        if grep -q 'regulatory.db is malformed or signature is missing/invalid' <<<"${said}"; then
            ok "the kernel reads firmware from the folder before the image's own"
        else
            not_ok "cfg80211 did not take the folder's regulatory.db" "${said}"
        fi
        modprobe -r cfg80211 2>/dev/null
    fi
    rm -f "${FIRMWARE}/regulatory.db"

    out="$(session_terminal 300 "${PASSWORD[@]}" -- "ame apps xbox-adapter off")"
    if [[ $? == 0 ]]; then ok "ame apps xbox-adapter off"; else not_ok "ame apps xbox-adapter off" "$(tail -n 10 <<<"${out}")"; fi
    same "the firmware and the blacklist are gone" \
        "$(find "${FIRMWARE}" -mindepth 1 | wc -l) $(test -e /etc/modprobe.d/xone.conf && echo blacklist)" "0 "
    if grep -qw "${KARG}" <<<"$(next_args)"; then not_ok "the next start still has ${KARG}" "$(next_args)"; else ok "the next start has no firmware path"; fi

    # Firmware updates waiting are said once, opening System Updates; fwupd and the notification stood in for
    install -d -m 0755 "${STANDIN}/fw"
    cat >"${STANDIN}/fw/fwupdmgr" <<'EOF'
#!/usr/bin/bash
echo '{"Devices": [{"Name": "System Firmware & Co", "DeviceId": "d1", "Version": "1.0", "Releases": [{"Version": "1.1"}]},
  {"Name": "Touchpad", "DeviceId": "d2", "Version": "2.0"}]}'
EOF
    cat >"${STANDIN}/fw/notify-send" <<EOF
#!/usr/bin/bash
printf '%s\n' "\$@" >>"${STANDIN}/fw/said"
EOF
    chmod 0755 "${STANDIN}/fw/fwupdmgr" "${STANDIN}/fw/notify-send"
    chown "${TEST_USER}" "${STANDIN}/fw"
    alert() {
        runuser -u "${TEST_USER}" -- env PATH="${STANDIN}/fw:/usr/bin" XDG_STATE_HOME="${STANDIN}/fw/state" \
            XDG_RUNTIME_DIR="/run/user/${UID_TEST}" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${UID_TEST}/bus" \
            timeout 120 /usr/libexec/amethystora-update-alert
    }
    alert
    if grep -q '^Firmware updates are waiting$' "${STANDIN}/fw/said" 2>/dev/null &&
        grep -q 'System Firmware &amp; Co' "${STANDIN}/fw/said" && ! grep -q Touchpad "${STANDIN}/fw/said"; then
        ok "the update alert says which firmware updates wait, and only those with a release"
    else
        not_ok "the update alert's firmware notification" "$(cat "${STANDIN}/fw/said" 2>&1)"
    fi
    rm -f "${STANDIN}/fw/said"
    alert
    if [[ -e "${STANDIN}/fw/said" ]]; then not_ok "the same firmware update was said again" "$(cat "${STANDIN}/fw/said")"; else
        ok "the same release is not said again"; fi
    firmware="$(record firmware)"
    if jq -e '.state | IN("ok", "info", "check")' <<<"${firmware}" >/dev/null; then
        ok "the report has its firmware check: $(jq -r .text <<<"${firmware}")"
    else
        not_ok "the report's firmware check" "${firmware}"
    fi
    reboot_and_continue
fi

# --- Phase 3: started without the firmware path -----------------------------------------------------------------
if grep -qw "${KARG}" /proc/cmdline; then not_ok "the machine started with ${KARG} still"; else ok "the machine started without the firmware path"; fi
same "the kernel's firmware path is empty" "$(cat /sys/module/firmware_class/parameters/path)" ""
if modprobe --showconfig | grep -qx 'blacklist mt76x2u'; then not_ok "mt76x2u is still blacklisted"; else ok "mt76x2u can load again"; fi
finish
