#!/usr/bin/bash
# vmlab-vm: installer
# Finishing an update with a soft reboot (amethystora-soft-reboot: System Updates' Restart now, `ame update
# restart`), on the installer-layout machine, whose /boot keeps a staged deployment through a restart:
# - the initramfs the image was built with holds nothing that changes with every build: no build version in its
#   os-release, no out-of-tree driver, which the system still loads itself (19-initramfs.sh);
# - updates made here from the running system (a hardlink checkout of its commit, changed and committed back, as
#   ostree's own soft-reboot test makes them: a copy of the image would not fit) and staged: one that adds a file
#   and compresses a loaded driver anew, as akmods builds its drivers anew from the same source for every image, is
#   quick; the same with a kernel argument added and one with that driver changed are not, and say why, nor is one
#   while the firmware was asked to start something next (BootNext); nothing staged says none;
# - the status as the account through pkexec without a password, prepare and reset as the account in its
#   session, and an app inhibiting logging out leaving the restart to GNOME's dialog;
# - System Updates: Restart now says the restart takes seconds and asks first, and Restart ends the session with a
#   soft reboot: the same kernel (boot id), SoftRebootsCount one higher, the update running, a file put in /etc
#   after it was staged still there, the account signed in again;
# - a full restart afterwards starts the update too, and the machine is put back on its own image.
# vmlab-paths: @sf/usr/libexec/amethystora-soft-reboot apps/update/* @sf/usr/libexec/amethystora-boot-guard @sf/usr/lib/systemd/system/amethystora-boot-guard*
source "$(dirname "$0")/lib.sh"

HERE="$(dirname "$0")"
STATE=/var/lib/vmlab-tests/89-soft-reboot
mkdir -p "${STATE}"
HELPER=/usr/libexec/amethystora-soft-reboot
MARKER=/usr/share/vmlab-soft-reboot
ETC_MARK=/etc/vmlab-soft-reboot-after-staging
DRIVER=v4l2loopback
# atspi.py where the account can run it: the suites' own folder is root's
ATSPI=/run/vmlab-soft-reboot/atspi.py

# The account, as 10-account.sh signs one in on the other machine: this one has none until a suite makes it
if ! getent passwd "${TEST_USER}" >/dev/null; then
    useradd -m -G wheel -c "Test account" "${TEST_USER}"
    echo "${TEST_USER}:${TEST_USER}" | chpasswd
    printf '[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=%s\n' "${TEST_USER}" >/etc/gdm/custom.conf
    install -d -o "${TEST_USER}" -g "${TEST_USER}" "/var/home/${TEST_USER}/.config"
    install -o "${TEST_USER}" -g "${TEST_USER}" /dev/null "/var/home/${TEST_USER}/.config/gnome-initial-setup-done"
    systemctl restart gdm
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}" "$(loginctl list-sessions --no-legend)"
    finish
fi
# Nothing else stages anything while this runs
systemctl stop uupd.timer 2>/dev/null

same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
remember() { printf '%s\n' "$2" >"${STATE}/$1"; }
recall() { cat "${STATE}/$1" 2>/dev/null; }
bootc_json() { bootc status --format=json | jq -r "$1"; }
# What the helper says, as root or as the account (pkexec, for anyone, without a password), and its WHY
status_root() { "${HELPER}" status 2>&1; }
status_user() { runuser -u "${TEST_USER}" -- "${HELPER}" status 2>&1; }
why() { jq -r 'if .quick then "quick" else .why end' <<<"$1" 2>/dev/null || echo "not JSON: $1"; }
says() { same "$1: root reads ${3}" "$(why "$(status_root)")" "$2"; }
REPO=/sysroot/ostree/repo
rw() { unshare -m sh -c 'mount -o remount,rw /sysroot && "$@"' sh "$@"; }
# An update of the running system, made as ostree's own soft-reboot test makes one: a checkout of the booted commit
# of hard links into the repository, CHANGE run in it, and committed back, so that nothing is copied (the machine's
# 40 GiB hold no second copy of the image). A file CHANGE replaces is written anew, never through its link.
make_update() {
    local dir="${REPO}/tmp/vmlab-$1"
    rw rm -rf "${dir}"
    rw ostree checkout --repo="${REPO}" -H "$(recall original)" "${dir}" &&
        rw sh -c "cd '${dir}' && $2" &&
        rw ostree commit --repo="${REPO}" --parent="$(recall original)" -b "vmlab/$1" -I --consume --bootable \
            --add-metadata-string=version="vmlab-$1" "${dir}" >/dev/null
}
# Staged as an update is: completed at the next shutdown, with the running system's kernel arguments and ARGS
stage() { timeout 1500 ostree admin deploy --stage "vmlab/$1" "${@:2}" 2>&1; }
nextroot() { mountpoint -q /run/nextroot; }
# The account's session start, in seconds
session_started() {
    date -d "$(loginctl show-session "$(loginctl show-user "${TEST_USER}" -p Display --value)" -p Timestamp --value)" +%s
}
atspi() { user_bus python3 "${ATSPI}" amethystora-update "$@"; }
initramfs() { echo "/usr/lib/modules/$(uname -r)/initramfs.img"; }

# --- Phase 1: the initramfs, then updates to stage, made from the running image ---------------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    check "the helper is there" test -x "${HELPER}"
    settled() { [[ "$(systemctl show -P ActiveState amethystora-signed-updates.service)" != activating ]]; }
    wait_for 1200 settled
    if [[ "$(bootc_json '.status.staged')" != null ]]; then
        echo "# something was staged at the first boot, taken away: $(bootc_json '.status.staged.image.image.image')"
        rpm-ostree cleanup --pending >/dev/null 2>&1
    fi
    remember original "$(bootc_json '.status.booted.ostree.checksum')"
    remember deployments "$(rpm-ostree status --json | jq '.deployments | length')"

    release="$(lsinitrd -f usr/lib/initrd-release "$(initramfs)" 2>&1)"
    if grep -qE '^(OSTREE_VERSION|IMAGE_VERSION|BUILD_ID)=' <<<"${release}" || ! grep -q '^ID=amethystora' <<<"${release}"; then
        not_ok "the initramfs's os-release carries the build's version, or is not the image's" "${release}"
    else
        ok "the initramfs's os-release is the image's, without the build's version: $(sed -n 's/^PRETTY_NAME=//p' <<<"${release}")"
    fi
    extra="$(lsinitrd "$(initramfs)" | grep -E 'lib/modules/[^/]+/(extra|updates)/.*\.ko' | grep -v /nvidia)"
    same "no out-of-tree driver in the initramfs" "${extra}" ""
    if lsmod | grep -q "^${DRIVER} "; then
        ok "${DRIVER} is loaded all the same, by the system's own modules-load.d"
    else
        not_ok "${DRIVER} is not loaded" "$(systemctl status systemd-modules-load --no-pager 2>&1 | tail -n 5)"
    fi
    says "nothing staged" none "none"
    same "nothing staged: the account reads none too, without a password" "$(why "$(status_user)")" none
    check "ame update restart with nothing staged says so, and restarts nothing" sh -c \
        "out=\$(runuser -u ${TEST_USER} -- ame update restart 2>&1); [ \$? = 1 ] && echo \"\$out\" | grep -q 'No update is waiting'"

    # Two updates of the running system: a file added and the driver compressed anew, as the kernel's build
    # compresses modules but with another preset (other bytes, the same driver); and the driver changed
    began=${SECONDS}
    module="$(modinfo -F filename "${DRIVER}")"
    module="usr/lib/modules/${module#*/modules/}"
    replace="chcon --reference=${module} ${module}.new && chmod --reference=${module} ${module}.new && mv -f ${module}.new ${module}"
    if out="$(make_update quick "echo vmlab >${MARKER#/} && chcon system_u:object_r:usr_t:s0 ${MARKER#/} &&
            xz -dc ${module} >${module}.ko && xz -C crc32 --lzma2=preset=0,dict=1MiB -c ${module}.ko >${module}.new &&
            rm ${module}.ko && ${replace}" 2>&1)" &&
        out="$(make_update driver "cp ${module} ${module}.new && printf vmlab >>${module}.new && ${replace}" 2>&1)"; then
        echo "# two updates made of the running system: $((SECONDS - began)) s"
        continue_phase
    fi
    not_ok "could not make the updates to stage" "$(tail -n 15 <<<"${out}")"
    finish
fi

# --- Phase 2: each update staged, what the helper says of it, then the restart from System Updates -------------
if [[ "${SUITE_PHASE}" == 2 ]]; then
    began=${SECONDS}
    out="$(stage quick --karg-append=vmlab.soft=1)" || not_ok "staging the update with a kernel argument added" "${out}"
    echo "# the first update staged: $((SECONDS - began)) s"
    says "an update that adds a kernel argument" kernel "kernel"
    out="$(stage driver)" || not_ok "staging the update with the driver changed" "${out}"
    says "an update that changes a loaded driver" driver "driver"
    out="$(stage quick)" || not_ok "staging the update with a file added" "${out}"
    same "bootc says the update with a file added can be soft rebooted into" "$(bootc_json '.status.staged.softRebootCapable')" true
    next="$(bootc_json '.status.staged.ostree | "/sysroot/ostree/deploy/\(.stateroot)/deploy/\(.checksum).\(.deploySerial)"')"
    file="$(modinfo -F filename "${DRIVER}")"
    if cmp -s "${file}" "${next}/usr/lib/modules/$(uname -r)/${file#*/modules/"$(uname -r)"/}"; then
        skip "a driver built anew from the same source: its compressed file came out the same, so nothing was tried"
    else
        ok "the update's ${DRIVER} is another file than the one loaded, of the same source ($(modinfo -F srcversion "${file}"))"
    fi
    says "an update that adds a file and compresses the driver anew" quick "quick"
    same "the account reads quick too" "$(why "$(status_user)")" quick
    remember staged "$(bootc_json '.status.staged.ostree.checksum')"
    # The firmware asked to start something next, as fwupd's capsule or a mirror's first start ask
    if command -v efibootmgr >/dev/null && current="$(efibootmgr | sed -n 's/^BootCurrent: //p')" && [[ -n "${current}" ]]; then
        efibootmgr -q -n "${current}"
        says "with BootNext set" firmware "firmware"
        efibootmgr -q -N
        says "with BootNext taken away again" quick "quick"
    else
        skip "BootNext: no efibootmgr, or no BootCurrent"
    fi

    # prepare and reset as the account in its session, which the rule lets the person at the machine do
    in_session "${HELPER} prepare" >/dev/null 2>&1
    if nextroot && ostree admin status | grep -q 'soft-reboot'; then
        ok "prepare, as the account in its session and without a password, mounts the update at /run/nextroot"
    else
        not_ok "prepare as the account did not mount /run/nextroot" "$(in_session "${HELPER} prepare" 2>&1 | tail -n 5)" "$(ostree admin status 2>&1)"
    fi
    in_session "${HELPER} reset" >/dev/null 2>&1
    if ! nextroot; then ok "reset, as the account, takes it away"; else not_ok "reset left /run/nextroot mounted"; fi
    # An app asking not to end the session: the restart is GNOME's dialog's, which names it. The helper finds a
    # systemctl here that restarts nothing, should it not see the app and go on.
    install -d -m 0755 /run/vmlab-soft-reboot/stub
    printf '#!/bin/sh\nexit 99\n' >/run/vmlab-soft-reboot/stub/systemctl
    chmod 0755 /run/vmlab-soft-reboot/stub/systemctl
    user_bus systemd-run --user --unit=vmlab-inhibit --collect --quiet -- \
        gnome-session-inhibit --inhibit logout --reason "vmlab: a document not saved" sleep 600
    inhibits() { [[ "$(in_session 'gdbus call --session --dest org.gnome.SessionManager --object-path /org/gnome/SessionManager --method org.gnome.SessionManager.IsInhibited 1')" == "(true,)" ]]; }
    wait_for 30 inhibits || not_ok "gnome-session does not say anything inhibits logging out" \
        "$(in_session 'gdbus call --session --dest org.gnome.SessionManager --object-path /org/gnome/SessionManager --method org.gnome.SessionManager.IsInhibited 1' 2>&1)"
    in_session "PATH=/run/vmlab-soft-reboot/stub:\$PATH ${HELPER} restart" >/dev/null 2>&1
    rc=$?
    if [[ "${rc}" == 3 ]] && ! nextroot; then
        ok "with an app inhibiting logging out, restart leaves it to GNOME's dialog (exit 3) and mounts nothing"
    else
        not_ok "restart with an app inhibiting logging out" "exit ${rc}, /run/nextroot $(nextroot && echo mounted || echo not mounted)"
        nextroot && "${HELPER}" reset
    fi
    user_systemctl stop vmlab-inhibit.service 2>/dev/null

    # Changed after the update was staged: the soft reboot's shutdown carries it over, as a full one's does
    echo "after staging, $(date +%s)" >"${ETC_MARK}"
    remember etc "$(cat "${ETC_MARK}")"

    # System Updates, as a person uses it
    install -d -m 0755 "$(dirname "${ATSPI}")"
    install -m 0644 "${HERE}/atspi.py" "${ATSPI}"
    user_bus systemd-run --user --unit=vmlab-updates --collect --quiet --setenv=LC_ALL=en_US.UTF-8 \
        --setenv=LANGUAGE=en -- /usr/bin/amethystora-update
    click=""
    if atspi wait 120 "the restart takes seconds" >/dev/null 2>&1; then
        ok "System Updates says the update leaves the kernel as it is, so the restart takes seconds"
        atspi click "Restart now" button >/dev/null 2>&1
        if atspi wait 30 "Restart now?" >/dev/null 2>&1 && atspi has "the login screen is back in a few seconds" >/dev/null 2>&1; then
            ok "Restart now asks first: the apps close, the login screen is back in a few seconds"
            click="python3 ${ATSPI} amethystora-update click Restart button"
        else
            not_ok "Restart now did not ask first" "$(atspi text 2>&1 | head -n 30)"
        fi
    else
        not_ok "System Updates does not say the restart takes seconds" "$(atspi text 2>&1 | head -n 30)"
    fi
    # Without the window's own way, the helper's, so that the soft reboot is tested all the same. Either from the
    # account's own manager, as the session's programs run, once this phase has ended
    [[ -n "${click}" ]] || click="${HELPER} restart"
    remember boot "$(cat /proc/sys/kernel/random/boot_id)"
    remember soft "$(systemctl show -P SoftRebootsCount)"
    remember clicked "$(($(date +%s) + 20))"
    # shellcheck disable=SC2086
    user_bus systemd-run --user --on-active=20 --unit=vmlab-soft-restart --collect --quiet -- ${click}
    next_boot '{"itself": true}'
    reboot_and_continue
fi

# --- Phase 3: after the soft reboot -------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 3 ]]; then
    same "the kernel kept running: the same boot id" "$(cat /proc/sys/kernel/random/boot_id)" "$(recall boot)"
    same "systemd counts one soft reboot more" "$(systemctl show -P SoftRebootsCount)" "$(($(recall soft) + 1))"
    same "the update is the system running now" "$(bootc_json '.status.booted.ostree.checksum')" "$(recall staged)"
    check "its file is there" test -f "${MARKER}"
    same "a file put in /etc after the update was staged is still there" "$(cat "${ETC_MARK}" 2>&1)" "$(recall etc)"
    same "nothing is left staged" "$(bootc_json '.status.staged')" null
    same "nor mounted for another soft reboot" "$(nextroot && echo mounted)" ""
    same "the previous system is the one to roll back to" "$(bootc_json '.status.rollback.ostree.checksum')" "$(recall original)"
    same "the account is signed in again" "$(loginctl show-user "${TEST_USER}" -p State --value)" active
    # The system's encrypted disk was kept attached, as it has to be for the update's mounts to stay
    check "the system's LUKS line in /etc/crypttab carries x-initrd.attach" grep -qE '^luks-[^ ]+ [^ ]+ [^ ]+ ([^ ]*,)?x-initrd\.attach' /etc/crypttab
    echo "# soft reboot: $(($(session_started) - $(recall clicked))) s from Restart to the account's new session"
    if lsmod | grep -q "^${DRIVER} "; then ok "${DRIVER} is still loaded"; else not_ok "${DRIVER} is no longer loaded"; fi
    says "after it" none "none"
    remember restarted "$(date +%s)"
    reboot_and_continue
fi

# --- Phase 4: a full restart, then back to the machine's own image ------------------------------------------
if [[ "${SUITE_PHASE}" == 4 ]]; then
    if [[ "$(cat /proc/sys/kernel/random/boot_id)" != "$(recall boot)" && "$(systemctl show -P SoftRebootsCount)" == 0 ]]; then
        ok "the full restart started the computer again"
    else
        not_ok "this was no full restart" "boot id $(cat /proc/sys/kernel/random/boot_id), soft reboots $(systemctl show -P SoftRebootsCount)"
    fi
    same "it started the update: the soft reboot's shutdown had made it the default" \
        "$(bootc_json '.status.booted.ostree.checksum')" "$(recall staged)"
    check "its file is there" test -f "${MARKER}"
    echo "# full restart: $(($(session_started) - $(recall restarted))) s from the restart to the account's new session"
    out="$(bootc rollback 2>&1)" || not_ok "bootc rollback to the machine's own image" "${out}"
    reboot_and_continue
fi

if [[ "${SUITE_PHASE}" == 5 ]]; then
    same "back on the machine's own image" "$(bootc_json '.status.booted.ostree.checksum')" "$(recall original)"
    rpm-ostree cleanup -r >/dev/null 2>&1
    rw ostree refs --repo="${REPO}" --delete vmlab >/dev/null 2>&1
    rw ostree prune --repo="${REPO}" --refs-only >/dev/null 2>&1
    rm -f "${ETC_MARK}"
    if [[ "$(rpm-ostree status --json | jq '.deployments | length')" == "$(recall deployments)" ]] &&
        [[ -z "$(ostree refs --repo="${REPO}" vmlab 2>/dev/null)" ]]; then
        ok "the machine is left with its deployments as they were, and no update of this suite's"
    else
        not_ok "the machine is not as it was" "$(rpm-ostree status 2>&1 | head -n 20)"
    fi
    rm -rf "${STATE}"
fi
finish
