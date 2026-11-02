#!/usr/bin/bash
# The trash emptied after 30 days (plans/done/PLAN-privacy-defaults.md): the image's default in a session, GNOME's
# housekeeping running, and `ame desktop privacy trash 7` taken by that running session at once. Three items in
# the trash, deleted 40 days, 10 days and a moment ago, put there before a restart as a month's trash would be,
# and the hourly purge of the next session waited for: the first two go, the third stays. What was deleted is
# still in a snapshot taken before, and a restic repository beside the trash is not touched. Then off, and Reset
# settings puts the image's default back.
# The purge runs an hour after gsd-housekeeping started and every hour after, so the wait outlasts a phase's 30
# minutes: each phase waits up to 20, then the next one goes on in the same boot (continue_phase). The hour is of
# time awake, and an idle session suspends after 15 minutes, so the session is kept from suspending meanwhile.
# vmlab-paths: @ui/modules/pill/moving.js @sf/usr/libexec/amethystora-reset-settings @sf/usr/share/glib-2.0/schemas/zz* @sf/*just/desktop.just @sf/usr/libexec/amethystora-free-space @ui/modules/privacy.js @ui/*quick/tv.js @ui/modules/windows/handover.js @sf/usr/libexec/amethystora-mirror
source "$(dirname "$0")/lib.sh"

STATE=/var/lib/vmlab-tests/93-privacy
HOME_DIR="/var/home/${TEST_USER}"
TRASH="${HOME_DIR}/.local/share/Trash"
REPO="${HOME_DIR}/vmlab-restic"
SERVICE=org.gnome.SettingsDaemon.Housekeeping.service
PHASE_START="${SECONDS}"
mkdir -p "${STATE}"

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

privacy() { runuser -u "${TEST_USER}" -- gsettings get org.gnome.desktop.privacy "$1"; }
now_s() { date +%s; }
started() { user_systemctl show "${SERVICE}" -p ExecMainStartTimestamp --value --timestamp=unix | tr -d @; }
in_trash() { [[ -e "${TRASH}/files/$1" && -e "${TRASH}/info/$1.trashinfo" ]]; }
gone() { [[ ! -e "${TRASH}/files/$1" && ! -e "${TRASH}/info/$1.trashinfo" ]]; }
repo_sum() { (cd "${REPO}" && find . -type f -printf '%p %s\n' | sort && find . -type f -exec cat {} + | sha256sum); }
# A file in the account's trash, as Files leaves one: the file, and its .trashinfo with when it was deleted
put_in_trash() {
    local name="$1" when="$2"
    runuser -u "${TEST_USER}" -- mkdir -p "${TRASH}/files" "${TRASH}/info"
    runuser -u "${TEST_USER}" -- sh -c "echo vmlab > '${TRASH}/files/${name}'"
    runuser -u "${TEST_USER}" -- sh -c "printf '[Trash Info]\nPath=%s\nDeletionDate=%s\n' '${HOME_DIR}/${name}' \
        '$(date -d "${when}" +%Y-%m-%dT%H:%M:%S)' > '${TRASH}/info/${name}.trashinfo'"
}

# --- Phase 1: the default, the trash, the snapshot and the repository, the new age ----------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    rm -f "${STATE}"/*
    check "the trash empties itself after 30 days, as the image ships it" test \
        "$(privacy remove-old-trash-files) $(privacy old-files-age)" == "true uint32 30"
    check "GNOME's housekeeping runs in the session" wait_for 120 pgrep -u "${TEST_USER}" -x gsd-housekeepin

    put_in_trash vmlab-forty "-40 days"
    put_in_trash vmlab-ten "-10 days"
    put_in_trash vmlab-today "now"
    listed="$(in_session gio list -a standard::display-name trash:/// 2>&1)"
    if grep -qF vmlab-forty <<<"${listed}" && grep -qF vmlab-ten <<<"${listed}" && grep -qF vmlab-today <<<"${listed}"; then
        ok "the three files are in the trash, as GIO lists it"
    else
        not_ok "GIO does not list the files in the trash" "${listed}"
    fi

    runuser -u "${TEST_USER}" -- mkdir -p "${HOME_DIR}/vmlab-backed-up"
    runuser -u "${TEST_USER}" -- sh -c "echo vmlab > ${HOME_DIR}/vmlab-backed-up/file"
    out="$(runuser -u "${TEST_USER}" -- env RESTIC_PASSWORD=vmlab sh -c \
        "restic init --repo ${REPO} && restic backup --repo ${REPO} ${HOME_DIR}/vmlab-backed-up" 2>&1)" ||
        not_ok "a restic repository could not be made beside the trash" "${out}"
    repo_sum >"${STATE}/repo"
    # The hourly snapshots need the home folders to be a subvolume of their own, as the installer makes them
    home_subvolume || make_home_subvolume
    # A trash that holds something a month old was there before this session: the rest in the next one
    reboot_and_continue
fi

# --- Phase 2: after a restart, the new age, and the purge's time ---------------------------------------------
if [[ "${SUITE_PHASE}" == 2 ]]; then
    check "GNOME's housekeeping runs in the session after the restart" wait_for 120 pgrep -u "${TEST_USER}" -x gsd-housekeepin
    # Its hourly timer stands still while the machine sleeps
    user_bus gsettings set org.gnome.settings-daemon.plugins.power sleep-inactive-ac-type nothing

    if home_subvolume; then
        if /usr/libexec/amethystora-home-snapshot >/dev/null 2>&1; then
            snapshot="$(find /var/home/.snapshots -mindepth 1 -maxdepth 1 -type d | sort | tail -n 1)"
            echo "${snapshot}" >"${STATE}/snapshot"
            check "a snapshot taken before the purge has the oldest of them" test -e "${snapshot}/${TEST_USER}/.local/share/Trash/files/vmlab-forty"
        else
            not_ok "the home folders could not be snapshotted" "$(/usr/libexec/amethystora-home-snapshot 2>&1 | tail -n 5)"
        fi
    else
        skip "the home folders are not a subvolume here, so no snapshot is taken"
    fi

    out="$(in_session ame desktop privacy trash 7 2>&1)"
    if [[ "$(privacy old-files-age)" == "uint32 7" ]] && grep -q "more than 7 days ago" <<<"${out}"; then
        ok "ame desktop privacy trash 7 sets seven days, and says so"
    else
        not_ok "ame desktop privacy trash 7 did not set seven days" "${out}" "$(privacy old-files-age)"
    fi
    check "and leaves the housekeeping key it changed to make GNOME read it at its default" test -z \
        "$(runuser -u "${TEST_USER}" -- dconf read /org/gnome/settings-daemon/plugins/housekeeping/min-notify-period)"

    # The purge after next: an hour from when gsd-housekeeping started, and every hour after
    start="$(started)"
    if [[ ! "${start}" =~ ^[0-9]+$ ]]; then
        not_ok "${SERVICE} has no start time" "$(user_systemctl status "${SERVICE}" 2>&1 | head -n 10)"
        finish
    fi
    now="$(now_s)"
    echo "${start}" >"${STATE}/start"
    echo $((start + ((now - start) / 3600 + 1) * 3600 + 30)) >"${STATE}/purge"
    echo "# gsd-housekeeping started $(date -d "@${start}"), it is $(date -d "@${now}"), the purge comes $(date -d "@$(cat "${STATE}/purge")")"
fi

# --- Every phase: wait for the purge, 25 minutes at a time ----------------------------------------------------
purge="$(cat "${STATE}/purge" 2>/dev/null)"
if [[ -z "${purge}" ]]; then
    not_ok "phase 2 left no time to wait for"
    finish
fi
# Until the purge has come and taken the oldest file, or ten minutes after it was due; never more than 20
# minutes a phase, so that what was found always gets back to the lab
while (($(now_s) < purge)) || { ! gone vmlab-forty && (($(now_s) < purge + 600)); }; do
    if ((SECONDS - PHASE_START > 1200)); then
        echo "# at ${SECONDS} s of phase ${SUITE_PHASE}: $((purge - $(now_s))) s to the purge"
        continue_phase
    fi
    sleep 20
done

# --- The last phase: what the purge took, and what it left ----------------------------------------------------
if [[ "$(started)" != "$(cat "${STATE}/start")" ]]; then
    not_ok "gsd-housekeeping started again while the suite waited, so its purge did not come when it was waited for"
fi
if gone vmlab-forty; then
    ok "the purge took the file deleted 40 days ago"
else
    # With what says whether the machine slept, and what gsd-housekeeping itself said
    not_ok "the file deleted 40 days ago is still in the trash" "$(ls -la "${TRASH}/files" "${TRASH}/info" 2>&1)" \
        "$(python3 -c 'import time; print("monotonic", time.clock_gettime(time.CLOCK_MONOTONIC), "boottime", time.clock_gettime(time.CLOCK_BOOTTIME))')" \
        "$(user_systemctl show "${SERVICE}" -p MainPID -p ExecMainStartTimestamp -p ActiveState 2>&1)" \
        "$(journalctl -b --no-pager _UID="$(id -u "${TEST_USER}")" 2>/dev/null | grep -E 'gsd-housekeeping|Housekeeping|gvfsd' | tail -n 15)" \
        "$(journalctl -b --no-pager 2>/dev/null | grep -iE 'suspend|PM: |sleep' | tail -n 5)"
fi
if gone vmlab-ten; then
    ok "and the one deleted 10 days ago: the running session took the 7 days at once"
else
    not_ok "the file deleted 10 days ago is still in the trash: the running session kept 30 days"
fi
check "the one deleted today stays" in_trash vmlab-today
snapshot="$(cat "${STATE}/snapshot" 2>/dev/null)"
if [[ -n "${snapshot}" ]]; then
    check "the snapshot still has what the purge took" test -e "${snapshot}/${TEST_USER}/.local/share/Trash/files/vmlab-forty"
fi
if [[ "$(repo_sum)" == "$(cat "${STATE}/repo")" ]]; then
    ok "the restic repository beside the trash is as it was"
else
    not_ok "the restic repository changed" "$(diff <(cat "${STATE}/repo") <(repo_sum) | head -n 20)"
fi

out="$(in_session ame desktop privacy trash off 2>&1)"
check "ame desktop privacy trash off turns it off" test "$(privacy remove-old-trash-files)" == false
in_session /usr/libexec/amethystora-reset-settings account >/dev/null 2>&1
check "Reset settings puts the image's 30 days back" test \
    "$(privacy remove-old-trash-files) $(privacy old-files-age)" == "true uint32 30"

# As the suites after this one expect the account and the machine
user_bus gsettings reset org.gnome.settings-daemon.plugins.power sleep-inactive-ac-type
rm -rf "${REPO}" "${HOME_DIR}/vmlab-backed-up"
rm -f "${TRASH}/files/vmlab-today" "${TRASH}/info/vmlab-today.trashinfo"
[[ -z "${snapshot}" ]] || btrfs subvolume delete "${snapshot}" >/dev/null 2>&1
finish
