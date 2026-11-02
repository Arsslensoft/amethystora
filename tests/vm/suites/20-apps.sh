#!/usr/bin/bash
# Amethystora's own apps open in the test account's session and stay open: each started as its launcher does,
# given 25 seconds, then closed as quitting closes it. Control is left open for the screenshot run.mjs takes
# afterwards. None may leave a core dump.
# vmlab-paths: apps/* @sf/usr/lib/amethystora-* @sf/usr/bin/amethystora-* @sf/usr/share/applications/* @sf/usr/libexec/amethystora-start-times @sf/usr/libexec/amethystora-wrong @sf/usr/libexec/amethystora-changes
source "$(dirname "$0")/lib.sh"

if ! has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
since="$(date '+%F %T')"
for app in manual security update logs notes backups phone control; do
    unit="test-app-${app}"
    started="$(user_bus systemd-run --user --unit="${unit}" --collect --quiet -- "/usr/bin/amethystora-${app}" 2>&1)"
    sleep 25
    # By its process: an app already open (the Manual's welcome page) takes a second start over itself
    if pgrep -u "${TEST_USER}" -f "^/usr/lib/amethystora-${app}/amethystora-${app}" >/dev/null; then
        ok "${app} opens and stays open"
    else
        not_ok "${app} did not stay open" "${started}" "$(journalctl -b _SYSTEMD_USER_UNIT="${unit}.service" --no-pager -o cat | tail -n 15)"
    fi
    # Its main process told to end, as quitting does, not every process of it at once
    [[ "${app}" == control ]] || user_systemctl kill --kill-whom=main --signal=SIGTERM "${unit}" >/dev/null 2>&1
done
dumps="$(crashes_since "${since}" COREDUMP_UID="$(id -u "${TEST_USER}")")"
if [[ -z "${dumps}" ]]; then ok "no app crashed"; else not_ok "an app crashed" "${dumps}"; fi
finish
