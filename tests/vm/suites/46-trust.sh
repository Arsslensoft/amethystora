#!/usr/bin/bash
# vmlab-vm: installer
# What the machine trusts, and who may become root:
# - Signature-checked updates from the first boot: the installers' own command (a kickstart's %post) rewrites
#   the origin in place with no network, and the machine then takes updates only signed; after a plain
#   `bootc switch`, amethystora-signed-updates stages the signed one again.
# - Kernel lockdown's switch (ame security lockdown) across restarts, on the installer's layout, whose /boot keeps
#   a staged deployment through a restart (bootc install to-disk's does not): the argument off and on again, the kernel
#   locked down by Secure Boot whatever the argument says, the report reading the live mode, and without
#   Secure Boot (a VM started without it) the argument deciding it, and the image's own modules with it.
# - Developer groups: nobody is joined by the image, `ame system dx-group` joins and leaves (asking first for
#   the ones that amount to root), the report names who is in one of those, Control lists them.
# vmlab-paths: @sf/usr/libexec/amethystora-hardening @sf/usr/libexec/amethystora-signed-updates* @sf/usr/libexec/amethystora-control-list @sf/usr/share/amethystora/developer-groups @sf/etc/containers/*
source "$(dirname "$0")/lib.sh"

STATE=/var/lib/vmlab-tests/46-trust
mkdir -p "${STATE}"

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
# The password, where sudo asks for one (a suite that ran before may have let the account off for its own run), at
# sudo's own prompt: dx-group's words say "password" before it
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
secure_boot() { mokutil --sb-state 2>/dev/null | grep -qi 'SecureBoot enabled'; }
mode() { sed -n 's/.*\[\([a-z]*\)\].*/\1/p' /sys/kernel/security/lockdown; }
has_arg() { grep -qw 'lockdown=integrity' /proc/cmdline; }
# The kernel arguments of the deployment the next restart starts
next_args() { rpm-ostree kargs 2>/dev/null; }
booted() { bootc status --format=json | jq -r "$1"; }
origin_file() { echo "$(readlink -f "/sysroot$(sed -n 's/.*ostree=\([^ ]*\).*/\1/p' /proc/cmdline)").origin"; }
rw_sysroot() { unshare -m sh -c 'mount -o remount,rw /sysroot && "$@"' sh "$@"; }

# --- Phase 1: the origin, the groups, then lockdown off ---------------------------------------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    ORIGIN="$(origin_file)"
    cp -a "${ORIGIN}" "${STATE}/origin"
    reference="$(rpm-ostree status --booted --json | jq -r '.deployments[0]."container-image-reference"')"
    # A machine whose image came from the registry unchecked, as a disk made from a published image does: at its
    # first boot with a network the safety net stages the switch to checked updates. Taken away once seen, so that
    # what follows starts from one deployment, as an installer's %post does.
    if [[ "${reference}" == ostree-unverified-registry:ghcr.io/arsslensoft/* ]]; then
        settled() { [[ "$(systemctl show -P ActiveState amethystora-signed-updates.service)" != activating ]]; }
        wait_for 1200 settled
        same "at its first boot with a network, amethystora-signed-updates staged the switch to checked updates" \
            "$(booted '.status.staged.image.image.signature | tostring')" containerPolicy
        rpm-ostree cleanup --pending >/dev/null 2>&1
    fi
    # What both kickstarts run in %post, inside the installed system, with the network off as the installer ISO
    # has it (its Network module is off)
    nmcli networking off
    out="$(rw_sysroot bootc switch --mutate-in-place --enforce-container-sigpolicy --transport registry \
        ghcr.io/arsslensoft/amethystora:stable 2>&1)"
    status=$?
    nmcli networking on
    if [[ "${status}" == 0 ]]; then ok "the kickstarts' bootc switch --mutate-in-place --enforce-container-sigpolicy works with no network"; else
        not_ok "the kickstarts' switch" "exit status ${status}" "${out}"; fi
    same "the machine then follows ghcr.io/arsslensoft/amethystora:stable, signature-checked" \
        "$(booted '.status.booted.image.image.image + " " + (.status.booted.image.image.signature | tostring)')" \
        "ghcr.io/arsslensoft/amethystora:stable containerPolicy"
    systemctl start amethystora-signed-updates.service
    if journalctl -u amethystora-signed-updates.service -n 3 --no-pager -o cat | grep -q 'are signature-checked$' &&
        [[ "$(booted '.status.staged')" == null ]]; then
        ok "amethystora-signed-updates finds nothing to do on such a machine"
    else
        not_ok "amethystora-signed-updates on a signed origin" "$(journalctl -u amethystora-signed-updates.service -n 5 --no-pager -o cat)"
    fi
    # A plain `bootc switch` afterwards, here in place: the next start of the service stages the signed one again.
    # Staging fetches the image, so only where the image booted is the registry's own (a disk of a published
    # image), and not a lab build, which it would download whole.
    if [[ "${reference}" == ostree-*registry:ghcr.io/arsslensoft/* ]]; then
        image="${reference#*registry:}"
        rw_sysroot bootc switch --mutate-in-place --transport registry "${image}" >/dev/null 2>&1
        unchecked="$(booted '.status.booted.image.image.signature | tostring')"
        if [[ "${unchecked}" != containerPolicy ]]; then ok "a plain bootc switch leaves updates unchecked (${unchecked})"; else
            not_ok "a plain bootc switch left updates signature-checked"; fi
        timeout 1200 systemctl start amethystora-signed-updates.service
        same "amethystora-signed-updates stages the signed switch again" \
            "$(booted '(.status.staged.image.image.image // "") + " " + (.status.staged.image.image.signature | tostring)')" \
            "${image} containerPolicy"
        rpm-ostree cleanup --pending >/dev/null 2>&1
    else
        skip "amethystora-signed-updates after a plain bootc switch: this disk is a lab build (${reference}), which it would download whole"
    fi
    rw_sysroot cp -a "${STATE}/origin" "${ORIGIN}"
    same "the origin put back as it was" "$(cmp -s "${STATE}/origin" "${ORIGIN}" && echo same)" same

    # --- Developer groups
    check "no service joins anyone to a group by itself" sh -c '! systemctl cat amethystora-dx-groups.service >/dev/null 2>&1'
    roots=()
    while read -r group root _; do
        [[ "${root}" == root ]] && getent group "${group}" >/dev/null && roots+=("${group}")
    done < <(grep -v '^#' /usr/share/amethystora/developer-groups | awk NF)
    joined="$(for group in "${roots[@]}"; do id -nG "${TEST_USER}" | tr ' ' '\n' | grep -x "${group}"; done)"
    same "the administrator ${TEST_USER} is in none of the groups that amount to root (${roots[*]:-none here})" "${joined}" ""
    same "and the report names nobody" "$(record root-groups)" null
    if [[ " ${roots[*]} " == *" docker "* ]]; then
        out="$(session_terminal 180 'become root without your password\?' y "${PASSWORD[@]}" -- "ame system dx-group docker dialout")"
        status=$?
        if [[ "${status}" == 0 && "${out}" == *"Joined docker."* && "${out}" == *"Joined dialout."* ]] &&
            id -nG "${TEST_USER}" | tr ' ' '\n' | grep -qx docker; then
            ok "ame system dx-group docker dialout asks first for docker, which amounts to root, and joins both"
        else
            not_ok "ame system dx-group docker dialout" "exit status ${status}" "$(tail -n 15 <<<"${out}")"
        fi
        check "docker has its line in /etc/group and /etc/gshadow, as grpck wants" sh -c "grep -q ^docker: /etc/group && grep -q ^docker: /etc/gshadow"
        same "the report names the account in docker" "$(record root-groups | jq -r '[.state, (.details[] | select(startswith("docker")))] | join(" ")')" \
            "info docker: ${TEST_USER}"
        check "a login of the account runs docker without sudo" as_user "docker version --format '{{.Server.Version}}'"
        listed="$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -c '[.. | objects | select(.id? == "dx-group")][0]')"
        if jq -e '.command == ["ame", "system", "dx-group"] and (.detail | test("docker"))' <<<"${listed}" >/dev/null; then
            ok "Control offers the groups and says the account is in docker"
        else
            not_ok "Control's dx-group entry" "${listed}"
        fi
        out="$(session_terminal 120 "${PASSWORD[@]}" -- "ame system dx-group none")"
        if [[ "${out}" == *"Left docker."* && "${out}" == *"Left dialout."* ]] && ! id -nG "${TEST_USER}" | tr ' ' '\n' | grep -qxE 'docker|dialout'; then
            ok "ame system dx-group none leaves them, without asking but for the password"
        else
            not_ok "ame system dx-group none" "$(tail -n 10 <<<"${out}")"
        fi
        same "and the report names nobody again" "$(record root-groups)" null
    else
        out="$(session_terminal 120 "${PASSWORD[@]}" -- "ame system dx-group dialout")"
        if [[ "${out}" == *"Joined dialout."* && "${out}" != *"become root"* ]]; then ok "ame system dx-group dialout joins a group that is not root without asking"; else
            not_ok "ame system dx-group dialout" "$(tail -n 10 <<<"${out}")"; fi
        session_terminal 120 "${PASSWORD[@]}" -- "ame system dx-group none" >/dev/null
        same "and none leaves it" "$(id -nG "${TEST_USER}" | tr ' ' '\n' | grep -x dialout)" ""
        skip "joining docker, incus-admin and libvirt: this image does not have them (the developer image does)"
    fi

    # --- Lockdown: the argument off
    if [[ -e /usr/lib/bootc/kargs.d/11-amethystora-lockdown.toml ]]; then
        check "the image asks for lockdown=integrity, and the machine started with it" has_arg
    else
        skip "lockdown=integrity: an NVIDIA image leaves it out"
    fi
    out="$(session_terminal 900 'Leave lockdown out of the kernel arguments' y "${PASSWORD[@]}" -- "ame security lockdown off")"
    if [[ $? == 0 ]] && ! grep -qw 'lockdown=integrity' <<<"$(next_args)"; then
        ok "ame security lockdown off takes the argument off the next start"
    else
        not_ok "ame security lockdown off" "$(tail -n 10 <<<"${out}")" "next: $(next_args)"
    fi
    reboot_and_continue
fi

# --- Phase 2: the argument off -------------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 2 ]]; then
    if has_arg; then not_ok "the machine started with lockdown=integrity again" "$(cat /proc/cmdline)"; else ok "the machine started without the argument"; fi
    lockdown="$(record lockdown)"
    if secure_boot; then
        same "with Secure Boot on, the kernel is locked down all the same" "$(mode)" integrity
        if jq -e '.state == "ok" and (.text | test("integrity"))' <<<"${lockdown}" >/dev/null; then
            ok "and the report says so, from the live mode"
        else
            not_ok "the report's lockdown" "${lockdown}"
        fi
        skip "Secure Boot off: run this suite in a VM started without it (vm_start secure_boot false)"
    else
        same "with Secure Boot off, the kernel is not locked down" "$(mode)" none
        if jq -e '.state == "check" and .command == "ame security lockdown on"' <<<"${lockdown}" >/dev/null; then
            ok "and the report says so, with the command that turns it on"
        else
            not_ok "the report's lockdown" "${lockdown}"
        fi
        check "the image's own modules load then (v4l2loopback)" modprobe v4l2loopback
        modprobe -r v4l2loopback 2>/dev/null
    fi
    out="$(session_terminal 900 'Turn kernel lockdown on' y "${PASSWORD[@]}" -- "ame security lockdown on")"
    if [[ $? == 0 ]] && grep -qw 'lockdown=integrity' <<<"$(next_args)"; then
        ok "ame security lockdown on puts the argument back for the next start"
    else
        not_ok "ame security lockdown on" "$(tail -n 10 <<<"${out}")" "next: $(next_args)"
    fi
    reboot_and_continue
fi

# --- Phase 3: on again ----------------------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 3 ]]; then
    check "the machine started with lockdown=integrity again" has_arg
    same "and the kernel is locked down" "$(mode)" integrity
    if ! secure_boot; then
        if out="$(modprobe v4l2loopback 2>&1)"; then
            not_ok "without Secure Boot, under lockdown, a module signed with a key the kernel does not trust loaded"
            modprobe -r v4l2loopback
        else
            ok "without Secure Boot, under lockdown, the image's own modules are refused (${out##*: })"
        fi
    fi
    same "the report says so" "$(record lockdown | jq -r .state)" ok
    # The lockdown switches made a deployment each: the one left to roll back to goes, as suites 76 and 89, which run
    # after this one on the same machine, expect the machine to have only the deployment it started from
    rpm-ostree cleanup -r >/dev/null 2>&1
    same "the machine is left with one deployment" "$(rpm-ostree status --json | jq '.deployments | length')" 1
fi
finish
