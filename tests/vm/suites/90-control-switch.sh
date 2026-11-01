#!/usr/bin/bash
# vmlab-order: last
# Control's two cards that change the whole machine, after 80-control.sh tried the rest, on the image without
# developer tools: Developer mode switches to the published developer image, and Start over makes a new
# installation beside the old one (bootc install reset), which leaves the machine without the test account, so
# this is the last suite, after 91 to 99 too (run.mjs runs it after every other suite of its machine). Four starts:
#   1. The ESP's automount taken off /boot, as an installed machine has /boot in /etc/fstab: on the lab's disks
#      systemd's gpt-auto mounts it there, and ostree cannot finish a staged deployment at shutdown.
#   2. Developer mode turned on in a terminal: the developer image pulled, its signature checked, staged.
#   3. Started on it: the image, the card's state, the developer groups card; then Start over, both questions
#      answered yes, with a file left in /etc, /var and the home folder.
#   4. Started on the new installation: what it kept and what it left on the disk.
# vmlab-paths: @sf/usr/libexec/amethystora-control-list apps/control/*
source "$(dirname "$0")/lib.sh"

IMAGE_INFO=/usr/share/amethystora/image-info.json
LIST=/usr/libexec/amethystora-control-list
SUDOERS=/etc/sudoers.d/zz-vmlab-control
POLKIT=/etc/polkit-1/rules.d/00-vmlab-control.rules
HOME_DIR="/var/home/${TEST_USER}"
MARK=vmlab-start-over
image="$(jq -r '."image-name"' "${IMAGE_INFO}")"

if [[ "${SUITE_PHASE}" == 1 && "${image}" != amethystora ]]; then
    skip "developer mode and starting over are tried from amethystora, and this is ${image}"
    finish
fi
if [[ "${SUITE_PHASE}" -lt 4 ]] && ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
TUID="$(id -u "${TEST_USER}" 2>/dev/null)"

as_window() {
    user_bus systemd-run --user --pipe --wait --quiet --collect --setenv=LC_ALL=C.UTF-8 --setenv=LANGUAGE= \
        -- "$@" </dev/null
}
LIST_JSON=""
read_list() { LIST_JSON="$(as_window "${LIST}" --json 2>/dev/null)"; }
item() { jq -c --arg id "$1" '.items[] | select(.id == $id)' <<<"${LIST_JSON}"; }
argv_of() { item "$1" | jq -r --arg v "${2:-}" 'if $v == "" then .command[] else .choices[] | select(.value == $v) | .command[] end'; }
# ID's command for VALUE (or its own) on a terminal in the account's session, its questions answered: OUT, STATUS
in_card() {
    local steps=() argv=()
    while [[ "$1" != -- ]]; do steps+=("$1"); shift; done
    shift
    read_list
    mapfile -t argv < <(argv_of "$1" "${2:-}")
    CARD="${argv[*]}"
    OUT="$(in_terminal "${steps[@]}" -- runuser -u "${TEST_USER}" -- env XDG_RUNTIME_DIR="/run/user/${TUID}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${TUID}/bus" \
        systemd-run --user --pty --wait --quiet --collect --setenv=LC_ALL=C.UTF-8 --setenv=LANGUAGE= -- "${argv[@]}" 2>&1)"
    STATUS=$?
}
allow_root() {
    printf '%s ALL=(ALL) NOPASSWD: ALL\n' "${TEST_USER}" >"${SUDOERS}"
    chmod 0440 "${SUDOERS}"
    printf 'polkit.addRule(function (action, subject) {\n    if (subject.user == "%s") { return polkit.Result.YES; }\n});\n' \
        "${TEST_USER}" >"${POLKIT}"
    sleep 2
}
staged() { bootc status --json 2>/dev/null | jq -c '.status.staged // empty'; }

# --- Phase 1 ------------------------------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    # Developer mode switches to the published developer image of the stream followed, which a build of a stream
    # nothing is published for (beta, on the next Fedora) has not got
    stream="$(jq -r '."image-tag"' "${IMAGE_INFO}")"
    if skopeo inspect --raw "docker://ghcr.io/arsslensoft/amethystora-dx:${stream}" 2>&1 | grep -q 'manifest unknown'; then
        skip "developer mode and starting over need a published amethystora-dx:${stream}, and there is none"
        finish
    fi
    ln -sf /dev/null /etc/systemd/system/boot.automount
    reboot_and_continue
fi

# --- Phase 2: Developer mode on ------------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 2 ]]; then
    if findmnt -n -t autofs /boot >/dev/null; then
        not_ok "the ESP's automount is still on /boot" "$(findmnt -R /boot)"
        finish
    fi
    allow_root
    in_card 2400 'Would you like to enable developer mode\?' y 'extra monospace fonts\?' n -- devmode on
    target="$(staged | jq -r '.image.image.image // ""')"
    if [[ ${STATUS} == 0 && "${target}" == ghcr.io/arsslensoft/amethystora-dx:* ]]; then
        ok "Developer mode on stages ${target}, its signature checked: ${CARD}"
    else
        not_ok "Developer mode on did not stage the developer image" "exit status ${STATUS}, staged: $(staged)" "$(tail -n 25 <<<"${OUT}")"
        finish
    fi
    echo "# staged: $(staged | jq -c '{image: .image.image, version: .image.version}')"
    reboot_and_continue
fi

# --- Phase 3: on the developer image; Start over -------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 3 ]]; then
    allow_root
    read_list
    if [[ "${image}" == amethystora-dx ]] && jq -e '(.items[] | select(.id == "devmode") | .state == "on") and
            any(.items[]; .id == "dx-group")' <<<"${LIST_JSON}" >/dev/null; then
        ok "started on amethystora-dx: Developer mode says on, and the developer groups card is there"
    else
        not_ok "not on the developer image after the restart" "image: ${image}" "$(bootc status 2>&1 | head -n 20)"
    fi
    # What starting over keeps is what these say
    echo kept >"/etc/${MARK}"
    echo kept >"/var/lib/${MARK}"
    runuser -u "${TEST_USER}" -- sh -c "echo kept >${HOME_DIR}/${MARK}"
    stateroot="$(bootc status --json | jq -r '.status.booted.ostree.stateroot // "default"')"
    in_card 2400 'experimental feature' $'\e[B\r' 'Are you sure' $'\e[B\r' -- powerwash
    # bootc install reset deploys into a stateroot of its own, which bootc status, showing the booted one's,
    # does not list as staged; the restart shows which starts
    new="$(ls /sysroot/ostree/deploy | grep -vx "${stateroot}" | head -n 1)"
    if [[ ${STATUS} == 0 && -n "${new}" ]] && ls -d "/sysroot/ostree/deploy/${new}/deploy/"*.0 >/dev/null 2>&1; then
        ok "Start over, asked twice, makes a new installation in a stateroot of its own (${new}): ${CARD}"
    else
        not_ok "Start over made no new installation" "exit status ${STATUS}, stateroots: $(ls /sysroot/ostree/deploy | paste -sd ' ')" \
            "$(ostree admin status 2>&1 | head -n 12)" "$(tail -n 25 <<<"${OUT}")"
        finish
    fi
    echo "# after Start over, before the restart: $(ostree admin status 2>&1 | paste -sd ' ' | cut -c1-400)"
    rm -f "${SUDOERS}" "${POLKIT}"
    reboot_and_continue
fi

# --- Phase 4: the new installation -------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 4 ]]; then
    booted="$(bootc status --json | jq -r '.status.booted.ostree.stateroot')"
    # The installation it started over from: the other stateroot on the disk
    old="$(ls /sysroot/ostree/deploy | grep -vx "${booted}" | head -n 1)"
    if [[ -n "${booted}" && "${booted}" != "${old}" ]]; then
        ok "the machine starts on the new installation (${booted}), not on ${old}"
    else
        not_ok "the machine still starts on ${old}" "$(bootc status 2>&1 | head -n 25)"
        finish
    fi
    check "with no test account" sh -c "! getent passwd ${TEST_USER}"
    check "and none of the old /etc" test ! -e "/etc/${MARK}"
    check "and none of the old /var" test ! -e "/var/lib/${MARK}"
    check "and no home folders" test ! -e "${HOME_DIR}"
    # In its /var, or in a subvolume of their own at the top of the btrfs, as the installer makes them (and
    # 70-backups.sh does on a lab disk)
    homes_kept() {
        local device kept=1
        test -e "/sysroot/ostree/deploy/${old}/var/home/${TEST_USER}/${MARK}" && return 0
        device="$(findmnt -no SOURCE /sysroot | sed 's/\[.*//')"
        mkdir -p /run/vmlab-top
        mount -o ro,subvolid=5 "${device}" /run/vmlab-top || return 1
        test -e "/run/vmlab-top/home/${TEST_USER}/${MARK}" && kept=0
        umount /run/vmlab-top
        return "${kept}"
    }
    check "while the old installation's home folders stay on the disk" homes_kept
    check "and its /etc" sh -c "ls /sysroot/ostree/deploy/${old}/deploy/*/etc/${MARK}"
    entries="$(ls /boot/loader/entries/ 2>/dev/null | wc -l)"
    old_entries="$(grep -l "/${old}/" /boot/loader/entries/*.conf 2>/dev/null | wc -l)"
    if ((old_entries > 0)); then
        ok "the boot menu can still start the old installation (${old_entries} of ${entries} entries)"
    else
        not_ok "the boot menu has no entry for the old installation" "$(grep -H '^options' /boot/loader/entries/*.conf 2>&1)"
    fi
    echo "# after Start over: $(ostree admin status 2>&1 | paste -sd ' ' | cut -c1-400)"
    echo "# old installation's size on the disk: $(du -sh "/sysroot/ostree/deploy/${old}/var" 2>/dev/null | cut -f1)"
fi
finish
