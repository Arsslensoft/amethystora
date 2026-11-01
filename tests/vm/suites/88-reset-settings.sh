#!/usr/bin/bash
# Resetting settings (ame system reset-settings, Control's Reset settings card), the step before starting over,
# on an account and a machine that changed things. Two starts:
#   1. The account's settings reset from the card in a terminal, what stays checked, and undone. Then the
#      machine's: a file changed, a file and a unit of its own, one of the image's units turned off, one masked,
#      Control's memory choice, beside what it keeps (an account, a network, a file Security watches), reset
#      from the card in a terminal.
#   2. Started after it: nothing failed that had not before, the session is back, and what was reset stays so.
#      Then undone, everything as it was, and reset again to leave the machine as the image ships it.
# vmlab-paths: @sf/usr/libexec/amethystora-reset-settings
source "$(dirname "$0")/lib.sh"

HELPER=/usr/libexec/amethystora-reset-settings
LIST=/usr/libexec/amethystora-control-list
SUDOERS=/etc/sudoers.d/zz-vmlab-reset
STATE=/var/lib/vmlab-tests/reset-settings
HOME_DIR="/var/home/${TEST_USER}"
ZRAM=/etc/systemd/zram-generator.conf.d/90-amethystora-choice.conf
WATCHED=/etc/sysctl.d/99-vmlab-reset.conf
CONNECTION=vmlab-reset

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
TUID="$(id -u "${TEST_USER}")"

as_window() {
    user_bus systemd-run --user --pipe --wait --quiet --collect --setenv=LC_ALL=C.UTF-8 --setenv=LANGUAGE= \
        -- "$@" </dev/null
}
# Control's Reset settings card on a terminal in the account's session, its questions answered: OUT, STATUS
in_card() {
    local argv=()
    mapfile -t argv < <(as_window "${LIST}" --json 2>/dev/null | jq -r '.items[] | select(.id == "reset-settings") | .command[]')
    CARD="${argv[*]}"
    OUT="$(in_terminal "$@" -- runuser -u "${TEST_USER}" -- env XDG_RUNTIME_DIR="/run/user/${TUID}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${TUID}/bus" \
        systemd-run --user --pty --wait --quiet --collect --setenv=LC_ALL=C.UTF-8 --setenv=LANGUAGE= -- "${argv[@]}" 2>&1)"
    STATUS=$?
}
dconf_get() { runuser -u "${TEST_USER}" -- dconf read "$1"; }
tiling_on() { [[ "$(runuser -u "${TEST_USER}" -- gsettings get org.gnome.shell.extensions.amethystora-ui.tiling enabled)" == true ]]; }
theme() { as_user amethystora-theme current; }
failed_units() { systemctl --failed --no-legend --plain | awk '{print $1}' | sort; }
enabled() { [[ "$(systemctl is-enabled "$1" 2>/dev/null)" == "$2" ]]; }

# --- Phase 1: the account's, undone; then the machine's --------------------------------------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    install -d -m 0700 "${STATE}"
    since="$(date '+%F %T')"
    # The account as somebody left it: seconds on the clock, a theme, classic windows, a French keyboard beside
    # the English one, the on-screen keyboard and larger text
    user_bus dconf write /org/gnome/desktop/interface/clock-show-seconds true
    user_bus dconf write /org/gnome/desktop/input-sources/sources "[('xkb', 'us'), ('xkb', 'fr')]"
    user_bus dconf write /system/locale/region "'fr_FR.UTF-8'"
    user_bus dconf write /org/gnome/desktop/a11y/applications/screen-keyboard-enabled true
    user_bus dconf write /org/gnome/desktop/interface/text-scaling-factor 1.25
    in_session amethystora-theme set tokyo-night >/dev/null 2>&1
    in_session ame desktop layout classic >/dev/null 2>&1
    in_session ame desktop icons neon >/dev/null 2>&1
    icons_flag="/var/home/${TEST_USER}/.config/amethystora/icons"
    welcome="$(dconf_get /org/gnome/shell/welcome-dialog-last-shown-version)"
    if [[ "$(theme)" != tokyo-night ]] || tiling_on; then
        not_ok "the account could not be changed first" "theme $(theme), tiling $(tiling_on && echo on || echo off)"
        finish
    fi

    in_card 300 'Reset which settings\?' $'\r' "Reset your account's settings\?" y
    if [[ ${STATUS} == 0 && -z "$(dconf_get /org/gnome/desktop/interface/clock-show-seconds)" ]]; then
        ok "Control's Reset settings card, Your account, puts the desktop's settings back: ${CARD}"
    else
        not_ok "the card did not reset the account's settings" "exit status ${STATUS}, clock-show-seconds: $(dconf_get /org/gnome/desktop/interface/clock-show-seconds)" \
            "$(tail -n 25 <<<"${OUT}")"
    fi
    check "the theme is the image's again" test "$(theme)" == amethystora
    check "and the layout tiles again, with the tiling on" sh -c "runuser -u ${TEST_USER} -- gsettings get org.gnome.shell.extensions.amethystora-ui.tiling enabled | grep -qx true"
    check "the icon pack it chose goes with the other settings" sh -c "[ ! -e ${icons_flag} ] && runuser -u ${TEST_USER} -- gsettings get org.gnome.desktop.interface icon-theme | grep -qx '.amethystora-glass.'"
    check "the keyboards stay" test "$(dconf_get /org/gnome/desktop/input-sources/sources)" == "[('xkb', 'us'), ('xkb', 'fr')]"
    check "the date and number formats stay" test "$(dconf_get /system/locale/region)" == "'fr_FR.UTF-8'"
    check "the on-screen keyboard and the larger text stay" test \
        "$(dconf_get /org/gnome/desktop/a11y/applications/screen-keyboard-enabled) $(dconf_get /org/gnome/desktop/interface/text-scaling-factor)" == "true 1.25"
    check "and GNOME's welcome dialog, seen already, stays seen" test "$(dconf_get /org/gnome/shell/welcome-dialog-last-shown-version)" == "${welcome}"
    save="$(runuser -u "${TEST_USER}" -- "${HELPER}" saved account)"
    check "what it replaced is saved first, in ${save}" grep -qx "clock-show-seconds=true" "${save}/dconf.ini"
    crashed="$(crashes_since "${since}" "COREDUMP_UID=${TUID}")"
    [[ -z "${crashed}" ]] && ok "nothing of the session crashed" || not_ok "something of the session crashed" "${crashed}"

    out="$(in_session ame system reset-settings undo account 2>&1)"
    if [[ "$(dconf_get /org/gnome/desktop/interface/clock-show-seconds)" == true && "$(theme)" == tokyo-night ]] && ! tiling_on &&
        [[ -d "${save}.undone" ]]; then
        grep -qx amethystora-neon "${icons_flag}" 2>/dev/null || not_ok "undo did not bring the icon pack it chose back"
        ok "undo brings the account's settings, theme and layout back, and keeps the save as ${save##*/}.undone"
    else
        not_ok "undo did not bring the account's settings back" "clock-show-seconds $(dconf_get /org/gnome/desktop/interface/clock-show-seconds), theme $(theme), tiling $(tiling_on && echo on || echo off)" \
            "${out}" "$(ls -a "${save%/*}")"
    fi
    # The account as the suites after this one expect it
    in_session "${HELPER}" account >/dev/null 2>&1
    user_bus dconf reset /system/locale/region
    for key in input-sources/sources a11y/applications/screen-keyboard-enabled interface/text-scaling-factor; do
        user_bus dconf reset "/org/gnome/desktop/${key}"
    done

    # The machine, before anything here changes it: what it already differs from the image in, which a reset
    # puts back too, and what failed
    failed_units >"${STATE}/failed.before"
    if "${HELPER}" list >"${STATE}/baseline" 2>"${STATE}/baseline.err"; then
        echo "# a machine as installed differs in $(wc -l <"${STATE}/baseline") files outside what a reset keeps:"
        sed 's/^/#   /' "${STATE}/baseline"
    else
        not_ok "the list of what a reset changes failed, as root" "$(cat "${STATE}/baseline.err")"
    fi
    cp -a /etc/passwd "${STATE}/passwd"
    # What it puts back
    echo "10.9.9.9 vmlab-reset" >>/etc/hosts
    echo vmlab >/etc/vmlab-reset.conf
    systemctl enable sshd.service 2>/dev/null
    systemctl disable fwupd-refresh.timer 2>/dev/null
    systemctl mask fstrim.timer 2>/dev/null
    mkdir -p "${ZRAM%/*}"
    printf '# mode: fast\n[zram0]\ncompression-algorithm = lzo-rle\n' >"${ZRAM}"
    cp -a /etc/hosts "${STATE}/hosts"
    # ...and what it keeps
    nmcli connection add type dummy ifname vmlab0 con-name "${CONNECTION}" >/dev/null
    echo "# vmlab" >"${WATCHED}"

    printf '%s ALL=(ALL) NOPASSWD: ALL\n' "${TEST_USER}" >"${SUDOERS}"
    chmod 0440 "${SUDOERS}"
    in_card 300 'Reset which settings\?' $'\e[B\r' 'Put these [0-9]+ files back' y
    rm -f "${SUDOERS}"
    shown=1
    for line in "put back +/etc/hosts" "taken away +/etc/vmlab-reset.conf" \
        "taken away +/etc/systemd/system/multi-user.target.wants/sshd.service" "taken away +/etc/systemd/system/fstrim.timer" \
        "brought back +/etc/systemd/system/timers.target.wants/fwupd-refresh.timer" "taken away +${ZRAM}"; do
        grep -qE "${line}" <<<"${OUT}" || shown=0
    done
    grep -qE "${CONNECTION}\.nmconnection|99-vmlab-reset|/etc/passwd|zz-vmlab-reset" <<<"${OUT}" && shown=0
    if [[ ${STATUS} == 0 && ${shown} == 1 ]]; then
        ok "Control's card, This machine, shows each file it puts back and none it keeps, then asks: ${CARD}"
    else
        not_ok "the card did not show what it puts back, or showed what it keeps" "exit status ${STATUS}" "$(tail -n 40 <<<"${OUT}")"
    fi
    check "/etc/hosts is the image's again" cmp -s /etc/hosts /usr/etc/hosts
    check "a file of the machine's own is taken away" test ! -e /etc/vmlab-reset.conf
    check "the unit turned on is off, the one masked unmasked, the image's own on again" sh -c \
        "! test -L /etc/systemd/system/multi-user.target.wants/sshd.service && ! test -L /etc/systemd/system/fstrim.timer && test -L /etc/systemd/system/timers.target.wants/fwupd-refresh.timer"
    check "Control's memory choice is the image's default" test ! -e "${ZRAM}"
    check "the network, the file Security watches and the accounts stay" sh -c \
        "test -e /etc/NetworkManager/system-connections/${CONNECTION}.nmconnection && test -e ${WATCHED} && cmp -s /etc/passwd ${STATE}/passwd"
    save="$("${HELPER}" saved machine)"
    check "what it replaced is saved first, in ${save}" cmp -s "${save}/etc/hosts" "${STATE}/hosts"
    reboot_and_continue
fi

# --- Phase 2: started after the reset; undone; reset again ----------------------------------------------------
if [[ "${SUITE_PHASE}" == 2 ]]; then
    ok "the session is back after the restart"
    new="$(comm -13 "${STATE}/failed.before" <(failed_units))"
    [[ -z "${new}" ]] && ok "no unit failed that had not before the reset" ||
        not_ok "units failed after the reset that had not before" "${new}" "$(systemctl --failed --no-pager 2>&1 | head -n 20)"
    if ! enabled sshd.service enabled && enabled fwupd-refresh.timer enabled && ! enabled fstrim.timer masked &&
        nmcli -t connection show "${CONNECTION}" >/dev/null 2>&1; then
        ok "after the restart sshd is off, fwupd-refresh.timer on, fstrim.timer unmasked, and the network still there"
    else
        not_ok "the restart did not keep the reset" "sshd $(systemctl is-enabled sshd.service 2>&1), fwupd-refresh.timer $(systemctl is-enabled fwupd-refresh.timer 2>&1), fstrim.timer $(systemctl is-enabled fstrim.timer 2>&1)"
    fi

    out="$("${HELPER}" undo machine 2>&1)"
    if cmp -s /etc/hosts "${STATE}/hosts" && test -e /etc/vmlab-reset.conf -a -e "${ZRAM}" && enabled sshd.service enabled &&
        enabled fstrim.timer masked && ! enabled fwupd-refresh.timer enabled; then
        ok "undo puts back every file the reset replaced"
    else
        not_ok "undo did not put the machine's files back" "${out}" "sshd $(systemctl is-enabled sshd.service 2>&1), fstrim.timer $(systemctl is-enabled fstrim.timer 2>&1)"
    fi

    # The machine as the image ships it, for the suites after this one
    nmcli connection delete "${CONNECTION}" >/dev/null 2>&1
    rm -f "${WATCHED}"
    out="$("${HELPER}" machine 2>&1)" || not_ok "the reset to leave the machine as the image ships it failed" "${out}"
    left="$("${HELPER}" list 2>&1)"
    [[ $? == 0 && -z "${left}" ]] && ok "a second reset leaves nothing to put back" || not_ok "a second reset still finds files" "${left}"
fi
finish
