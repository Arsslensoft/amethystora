#!/usr/bin/bash
# Backups (amethystora-backups, /usr/libexec/amethystora-restore) on a machine laid out as the installer lays it
# out, the home folders a btrfs subvolume of their own, with the hourly snapshots on and the daily backup going
# to an append-only rest-server: what the helper lists, finds and puts back from both, as the account; that
# putting back shares a snapshot's blocks, takes the SELinux label of where it lands, refuses what leads out of
# the home, and deletes nothing; and the window, used through its accessibility tree as a person would
# (atspi.py): back up now with the journal followed, the daily run, keep both, versions, look, the whole folder,
# a stale and a failed backup, the agent, setting up, Security, and opening it on a folder as Files does.
# `bootc install to-disk` makes no home subvolume, so phase 1 makes one, as the installer would have, and
# restarts.
# vmlab-paths: @ui/modules/pill/moving.js apps/backups/* @sf/usr/libexec/amethystora-restore @sf/usr/libexec/amethystora-backup* @sf/usr/libexec/amethystora-home-snapshot @sf/*just/backup.just @sf/usr/share/user-tmpfiles.d/amethystora-restore.conf @sf/usr/libexec/amethystora-changes
source "$(dirname "$0")/lib.sh"
cd /

HERE="$(cd "$(dirname "$0")" && pwd)"
HOME_DIR="/var/home/${TEST_USER}"
SNAPSHOTS=/var/home/.snapshots
KEEP=/var/lib/vmlab-tests/backups
# restic's rest-server 0.14.0, run with --append-only as the plan asks of a backup server
REST_IMAGE=docker.io/restic/rest-server@sha256:d2aff06f47eb38637dff580c3e6bce4af98f386c396a25d32eb6727ec96214a5
REST_PASSWORD=vmlab-rest-account
REPO_PASSWORD=vmlab-repository-key
REPO="rest:http://${TEST_USER}:${REST_PASSWORD}@127.0.0.1:8000/${TEST_USER}/"
SHOWN="rest:http://127.0.0.1:8000/${TEST_USER}/"
# The window, as the accessibility tree names it, and its program
APP=amethystora-backups
BINARY=/usr/lib/amethystora-backups/amethystora-backups
# atspi.py where the account can run it: the suites' own folder is root's
ATSPI=/run/vmlab-backups/atspi.py
OPENED=/tmp/vmlab-opened

# After a restart, the session comes back by itself (GDM signs the account in). Without one after this suite's
# own restart, the home subvolume is what kept it away.
if ! wait_for 300 has_session; then
    if [[ "${SUITE_PHASE}" == 1 ]]; then
        skip "no session for ${TEST_USER} (10-account.sh)"
    else
        not_ok "no session for ${TEST_USER} after the restart onto the home subvolume" \
            "system: $(systemctl is-system-running 2>&1)" "$(systemctl list-jobs --no-pager 2>&1 | head -n 8)" \
            "$(findmnt /var/home 2>&1)" "$(grep -v '^#' /etc/fstab 2>&1)" \
            "$(journalctl -b -p warning -u gdm -u var-home.mount --no-pager -o cat 2>&1 | tail -n 10)"
    fi
    finish
fi

if [[ "${SUITE_PHASE}" == 1 ]] && ! home_subvolume; then
    make_home_subvolume
    reboot_and_continue
fi

if home_subvolume; then
    ok "the home folders are a btrfs subvolume of their own"
else
    not_ok "/var/home is not a btrfs subvolume after the restart" "$(findmnt /var/home)" "$(grep home /etc/fstab)"
    finish
fi

uid="$(id -u "${TEST_USER}")"
# The account, with its session's bus, as a terminal in its session has it
as_tester() { user_bus env HOME="${HOME_DIR}" USER="${TEST_USER}" LOGNAME="${TEST_USER}" "$@"; }
TTY_TESTER=(runuser -u "${TEST_USER}" -- env HOME="${HOME_DIR}" USER="${TEST_USER}" LOGNAME="${TEST_USER}"
    XDG_RUNTIME_DIR="/run/user/${uid}" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${uid}/bus")
# The helper as the window runs it: its JSON on standard output, whether it succeeded or not
restore() { as_tester /usr/libexec/amethystora-restore "$@" --json; }
# json_check WHAT JSON FILTER: ok when FILTER is true of JSON
json_check() {
    if jq -e "$3" <<<"$2" >/dev/null 2>&1; then ok "$1"; else not_ok "$1" "$(head -c 3000 <<<"$2")"; fi
}
newest_snapshot() { find "${SNAPSHOTS}" -mindepth 1 -maxdepth 1 -regextype egrep -regex '.*/[0-9]{4}-[0-9]{2}-[0-9]{2}_[0-9]{4}' -printf '%f\n' | sort | tail -n 1; }
# A snapshot named for a minute none has yet: snapshots are named to the minute
snapshot() {
    sleep $((61 - $(date +%-S)))
    systemctl start amethystora-home-snapshot.service && newest_snapshot
}
backup_ids() { restore points | jq -r '.points[] | select(.source == "backup") | .id'; }
in_home() { as_tester bash -c "cd ~ && $1"; }

# --- The window, through its accessibility tree ---------------------------------------------------------

# atspi.py on the window, as the account, on its session's accessibility bus
atspi() { user_bus python3 "${ATSPI}" "${APP}" "$@"; }
# Whether the window shows TEXT (a name, a label's text)
has() { atspi has "$1"; }
# What the window says, one line each
says() { atspi text 2>/dev/null; }
# atspi_state TEXT STATE: the first widget named TEXT is checked, selected...
atspi_state() {
    user_bus python3 - "${ATSPI%/*}" "${APP}" "$1" "$2" <<'EOF'
import sys
sys.path.insert(0, sys.argv[1])
import atspi
from gi.repository import Atspi
app = atspi.application(sys.argv[2])
node = atspi.find(app, sys.argv[3]) if app else None
sys.exit(0 if node and node.get_state_set().contains(getattr(Atspi.StateType, sys.argv[4].upper())) else 1)
EOF
}
# The window, started as its launcher starts it, in English; ARGS after its own
WINDOW_UNIT=""
window() {
    WINDOW_UNIT="vmlab-backups-${RANDOM}"
    user_bus systemd-run --user --unit="${WINDOW_UNIT}" --collect --quiet --setenv=LC_ALL=en_US.UTF-8 \
        --setenv=LANGUAGE=en -- /usr/bin/amethystora-backups "$@"
    # What is set up and every point in time read, the folder's heading shown
    atspi wait 120 "as it was" >/dev/null 2>&1
}
# Closed as quitting closes it: its main process told to end
close_window() {
    [[ -z "${WINDOW_UNIT}" ]] || user_systemctl kill --kill-whom=main --signal=SIGTERM "${WINDOW_UNIT}.service" >/dev/null 2>&1
    wait_for 30 sh -c "! pgrep -u ${TEST_USER} -f '^${BINARY}'"
}
# What the window calls a point in time, as a screen reader hears it: its day, its time and where it is kept,
# in American English as the window is started (en_US: "Today, 02:05 PM, On this machine")
point_label() {
    local point time day
    point="$(restore points | jq -c --arg id "$1" '.points[] | select(.id == $id)')"
    time="$(jq -r .time <<<"${point}")"
    case "$(date -d "@${time}" +%F)" in
        "$(date +%F)") day=Today ;;
        "$(date -d yesterday +%F)") day=Yesterday ;;
        *) day="$(LC_ALL=en_US.UTF-8 date -d "@${time}" '+%A, %B %-d')" ;;
    esac
    printf '%s, %s, %s' "${day}" "$(LC_ALL=en_US.UTF-8 date -d "@${time}" '+%I:%M %p')" \
        "$([[ "$(jq -r .source <<<"${point}")" == local ]] && echo 'On this machine' || echo 'In the backup')"
}
# Shown at the point the timeline names LABEL (point_label), with the folder whose heading is HEADING read
showing() {
    atspi_state "$1" selected && has "$2 as it was" && ! has "Reading the folder…"
}
# Shown at the point ID, so
shown_at() {
    local label
    label="$(point_label "$1")" && wait_for "$3" showing "${label}" "$2"
}
# The timeline's point ID picked, and its listing of the folder shown read
go_to() {
    local label
    label="$(point_label "$1")"
    atspi click "${label}" button >/dev/null && wait_for 180 showing "${label}" "$2"
}
# The folder NAME in the listing opened, the window then at POINT
open_folder() {
    atspi click "$1" button >/dev/null && shown_at "$2" "$1" 180
}
# A row of the folder ticked, once the list shows it, and seen ticked
select_file() { atspi wait 60 "Select $1" >/dev/null && atspi check "Select $1" on && wait_for 30 atspi_state "Select $1" checked; }
# The last thing done in the window came to TEXT, once it has
banner_says() {
    if wait_for 300 has "$1"; then
        ok "$2"
    else
        not_ok "$2" "$(says | tail -n 30)"
    fi
}
# What Ctrl+R does: what is set up and every point read again, as the application's own action, whose
# outcome the checks after it wait for
reload() {
    user_bus gdbus call --session --dest org.amethystora.Backups --object-path /org/amethystora/Backups \
        --method org.gtk.Actions.Activate reload '[]' '{}' >/dev/null
}
# Each row of a folder is named for its entry and how it is now
rows_marked() { has 'report.txt, Changed since' && has 'big.bin, Same as now'; }
versions_read() { has 'Versions of report.txt' && ! has 'Looking for every version'; }
stale_said() { has "No backup has succeeded for 8 days" && has "Last good backup 8 days ago"; }
show_status() { atspi click Status button >/dev/null && atspi wait 30 "Daily backup" >/dev/null; }
show_browse() { atspi click Browse button >/dev/null && atspi wait 60 "as it was" >/dev/null; }

cleanup() {
    close_window
    pkill -TERM -o -u "${TEST_USER}" -f '^/usr/lib/amethystora-security/amethystora-security' 2>/dev/null
    pkill -u "${TEST_USER}" -f 'amethystora-agent diagnose|amethystora-in-terminal ame backup' 2>/dev/null
    as_tester systemctl --user disable --now amethystora-backup.timer >/dev/null 2>&1
    rm -f "${HOME_DIR}/.config/amethystora/backup.env" "${HOME_DIR}/.config/amethystora/backup-password" \
        "${HOME_DIR}/.local/state/amethystora/backup-last-success" "${HOME_DIR}/.local/share/applications/vmlab-opener.desktop" \
        "${HOME_DIR}/escape" "${OPENED}"
    sed -i '/vmlab-opener/d' "${HOME_DIR}/.config/mimeapps.list" 2>/dev/null
    chmod -R u+w "${HOME_DIR}/.cache/amethystora-restore" 2>/dev/null
    rm -rf "${HOME_DIR}/.cache/amethystora-restore"
    podman rm -f vmlab-rest >/dev/null 2>&1
    rm -rf "${KEEP}" /run/vmlab-backups
}
trap cleanup EXIT
since="$(date '+%F %T')"

# --- What it starts from -------------------------------------------------------------------------------

# Nothing of a backup that a suite before this one set up
as_tester systemctl --user disable --now amethystora-backup.timer >/dev/null 2>&1
rm -f "${HOME_DIR}/.config/amethystora/backup.env" "${HOME_DIR}/.config/amethystora/backup-password" \
    "${HOME_DIR}/.local/state/amethystora/backup-last-success"

# The hourly snapshots, through Security's own switch
out="$(in_terminal 120 'Turn ransomware protection on' 'y' -- ame security ransomware on 2>&1)"
if systemctl is-enabled --quiet amethystora-home-snapshot.timer && [[ -n "$(newest_snapshot)" ]]; then
    ok "ame security ransomware on turns the hourly snapshots on and takes the first"
else
    not_ok "the hourly snapshots did not come on" "$(tail -n 15 <<<"${out}")"
    finish
fi

# The backup server: rest-server in append-only mode, an account for the test account, and its repository
# private to it, as the plan asks of a server
mkdir -p "${KEEP}/rest"
podman rm -f vmlab-rest >/dev/null 2>&1
if ! out="$(podman pull -q "${REST_IMAGE}" 2>&1)"; then
    not_ok "rest-server could not be pulled" "${out}"
    finish
fi
install -m 0640 /dev/null "${KEEP}/rest/.htpasswd"
podman run --rm -v "${KEEP}/rest:/data:Z" --entrypoint create_user "${REST_IMAGE}" "${TEST_USER}" "${REST_PASSWORD}" >/dev/null 2>&1
podman run -d --name vmlab-rest -p 127.0.0.1:8000:8000 -v "${KEEP}/rest:/data:Z" \
    -e OPTIONS="--append-only --private-repos" "${REST_IMAGE}" >/dev/null
check "an append-only rest-server answers" wait_for 60 curl -s -o /dev/null http://127.0.0.1:8000/

# What there is to go back to: files as they were at the first snapshot (S1), the first backup (B1, with
# report.txt changed once more) and the second snapshot (S2)
in_home 'mkdir -p Documents/notes .ssh && printf "version 1\n" >Documents/report.txt && printf "a 1\n" >Documents/notes/a.txt &&
    printf "b 1\n" >Documents/notes/b.txt && printf "gone\n" >Documents/gone.txt && printf "known 1\n" >.ssh/vmlab-known &&
    head -c 33554432 /dev/urandom >Documents/big.bin'
S1="$(snapshot)"
check "a snapshot holds the test files" test -f "${SNAPSHOTS}/${S1}/${TEST_USER}/Documents/report.txt"

# Setting the backup up, as Backups' "Set up a backup…" does: ame backup setup in a terminal
out="$(in_terminal 180 'Repository' "${REPO}"$'\r' 'Repository password' "${REPO_PASSWORD}"$'\r' \
    'What to back up' $'\r' -- "${TTY_TESTER[@]}" ame backup setup 2>&1)"
if as_tester systemctl --user is-enabled --quiet amethystora-backup.timer; then
    ok "ame backup setup sets up a backup to an append-only rest-server, the daily run on"
else
    not_ok "ame backup setup did not finish" "$(tail -n 20 <<<"${out}")"
    finish
fi
grep -q "Nothing there yet, creating the repository" <<<"${out}" &&
    echo "# ame backup setup: rest-server --append-only let restic init create the repository"

in_home 'printf "version 1b\n" >Documents/report.txt'
began=${SECONDS}
if as_tester systemctl --user start --wait amethystora-backup.service; then
    ok "the daily backup's run succeeds"
    echo "# the first backup of the test account's home: $((SECONDS - began)) s"
else
    not_ok "the backup failed" "$(journalctl _SYSTEMD_USER_UNIT=amethystora-backup.service -n 20 --no-pager -o cat)"
    finish
fi
B1="$(backup_ids | head -n 1)"
check "the run wrote when it last succeeded" test -s "${HOME_DIR}/.local/state/amethystora/backup-last-success"
if journalctl _UID="${uid}" --no-pager -o cat | grep -qF "${REST_PASSWORD}"; then
    not_ok "the rest-server's password is in the journal"
else
    ok "the journal holds no password of the repository's address"
fi

in_home 'printf "version 2\n" >Documents/report.txt && rm Documents/gone.txt && printf "a 2\n" >Documents/notes/a.txt &&
    printf "c 2\n" >Documents/notes/c.txt && printf "known 2\n" >.ssh/vmlab-known'
S2="$(snapshot)"

# --- The helper ------------------------------------------------------------------------------------------

st="$(restore status)"
json_check "status: the snapshots are possible, on and cover the account" "${st}" \
    '.snapshots.possible and .snapshots.on and .snapshots.covers and .snapshots.count >= 3'
json_check "status: the backup is set up, its address shown without the password, last good just now" "${st}" \
    ".backup.set_up and .backup.repository == \"${SHOWN}\" and .backup.daily and (.backup.stale | not)
        and .backup.last_success > (now - 900)"
grep -qF "${REST_PASSWORD}" <<<"${st}" && not_ok "status shows the rest-server's password"

began=${SECONDS}
points="$(restore points)"
echo "# points: 3 or more snapshots and one backup on the append-only rest-server in $((SECONDS - began)) s"
json_check "points: both sources, newest first, the setup's snapshot left out" "${points}" "
    .backup == \"ok\" and (.points | map(.time) | . == (sort | reverse))
    and ([.points[] | select(.source == \"backup\")] | length) == 1
    and ([.points[].id] | index(\"${S1}\") and index(\"${S2}\") and index(\"${B1}\"))"
out="$(as_tester ame backup snapshots 2>&1)"
if grep -qF "${S2}" <<<"${out}" && grep -qF "${B1}" <<<"${out}" && ! grep -qF "${REST_PASSWORD}" <<<"${out}"; then
    ok "ame backup snapshots lists the same points"
else
    not_ok "ame backup snapshots does not list the points" "${out}"
fi
out="$(in_terminal 60 'From which snapshot' $'\x1b' -- "${TTY_TESTER[@]}" ame security ransomware restore 2>&1)"
grep -qF "${S2}" <<<"${out}" && ok "ame security ransomware restore offers the snapshots" ||
    not_ok "ame security ransomware restore does not offer ${S2}" "$(tail -n 15 <<<"${out}")"

# One folder, its own entries only, each with how it is now
STATES='[.entries[] | {(.name): .now}] | add'
for point in "${S1}" "${B1}"; do
    listing="$(restore ls "${point}" Documents)"
    json_check "ls ${point} Documents: one level, and what is gone, changed and the same now" "${listing}" "
        (${STATES}) as \$now | \$now[\"report.txt\"] == \"changed\" and \$now[\"gone.txt\"] == \"missing\"
        and \$now.notes == \"present\" and \$now[\"big.bin\"] == \"same\"
        and all(.entries[]; .name | contains(\"/\") | not) and (any(.entries[]; .name == \"a.txt\") | not)"
done
json_check "ls of the whole home folder in the backup" "$(restore ls "${B1}")" 'any(.entries[]; .name == "Documents" and .type == "dir")'

began=${SECONDS}
versions="$(restore versions Documents/report.txt)"
echo "# versions of one file, from the snapshots and the append-only rest-server: $((SECONDS - began)) s"
json_check "versions: the three that differ, from both sources, newest first" "${versions}" '
    .backup == "ok" and (.versions | length) == 3 and ([.versions[].source] | index("backup") and index("local"))
    and (.versions | map(.time) | . == (sort | reverse)) and .now.type == "file"'

# --- The window: backing up ------------------------------------------------------------------------------

install -d -m 0755 /run/vmlab-backups
install -m 0644 "${HERE}/atspi.py" "${ATSPI}"
window
newest="$(restore points | jq -r '.points[0].id')"
check "the window opens on the home folder at the newest point" shown_at "${newest}" "Your home folder" 60
said="$(says)"
local_points="$(grep -c ', On this machine$' <<<"${said}")"
backup_points="$(grep -c ', In the backup$' <<<"${said}")"
if ((local_points >= 3 && backup_points == 1)) && ! grep -qF "The backup could not be read" <<<"${said}"; then
    ok "its timeline has both, marked by where each is kept"
else
    not_ok "its timeline does not have both" "${local_points} on this machine, ${backup_points} in the backup" "$(tail -n 30 <<<"${said}")"
fi

show_status
# The run followed as it goes: what the window shows of it read until it says how it went
: >"${KEEP}/window-lines"
rm -f "${KEEP}/first-line"
followed=false
# The window shows the run's last lines only: its first is looked for all along, beside the reading below
(atspi wait 600 "Backing up /" >/dev/null 2>&1 && touch "${KEEP}/first-line") &
first_line=$!
if atspi click "Back up now" button >/dev/null; then
    deadline=$((SECONDS + 600))
    while ((SECONDS < deadline)); do
        said="$(says)"
        printf '%s\n' "${said}" >>"${KEEP}/window-lines"
        pgrep -u "${TEST_USER}" -f "journalctl --user --user-unit=amethystora-backup.service --follow" >/dev/null && followed=true
        grep -qx -e "Your home folder is in the backup as it is now." -e "The backup did not succeed" <<<"${said}" && break
    done
    if grep -qx "Your home folder is in the backup as it is now." <<<"${said}"; then
        ok "Back up now starts the backup, and it says the backup succeeded"
    else
        not_ok "the backup started from the window did not end well within 10 minutes" "$(tail -n 30 <<<"${said}")"
    fi
    [[ "${followed}" == true ]] && ok "it follows the run in the journal as it goes" ||
        not_ok "it did not follow the run in the journal" "$(sort -u "${KEEP}/window-lines" | tail -n 20)"
    if [[ -e "${KEEP}/first-line" ]] || grep -q '^Backing up /' "${KEEP}/window-lines"; then
        ok "from its first line"
    else
        not_ok "the run's first line was not shown" "$(sort -u "${KEEP}/window-lines" | tail -n 20)"
    fi
    grep -qF "${REST_PASSWORD}" "${KEEP}/window-lines" && not_ok "what it followed has a password" ||
        ok "what it followed has no password"
else
    not_ok "there is no Back up now on the status page" "$(says | tail -n 30)"
fi
pkill -u "${TEST_USER}" -f "${ATSPI} ${APP} wait 600 Backing up" 2>/dev/null
wait "${first_line}" 2>/dev/null
backups_shown() { [[ "$(says | grep -c ', In the backup$')" == 2 ]]; }
show_browse
check "the new backup is on the timeline" wait_for 120 backups_shown
B2="$(backup_ids | head -n 1)"

show_status
check "Turn off the daily run" atspi click "Turn off the daily run" button
if wait_for 60 has "Turn on the daily run" && ! as_tester systemctl --user is-enabled --quiet amethystora-backup.timer; then
    ok "the daily run is off, its timer disabled"
else
    not_ok "the daily run did not go off" "$(as_tester systemctl --user is-enabled amethystora-backup.timer 2>&1)"
fi
check "Turn on the daily run" atspi click "Turn on the daily run" button
if wait_for 60 has "Turn off the daily run" && as_tester systemctl --user is-enabled --quiet amethystora-backup.timer; then
    ok "the daily run is on again, its timer enabled"
else
    not_ok "the daily run did not come back on"
fi

# --- The helper: putting back ----------------------------------------------------------------------------

# A file Look hands to the desktop is recorded by an app made the default for text, which the account can
# run: not under /var/lib/vmlab-tests, which 50-raid.sh keeps to root
cat >/run/vmlab-backups/opener <<EOF
#!/bin/sh
printf '%s\n' "\$1" >>${OPENED}
EOF
chmod 0755 /run/vmlab-backups/opener
in_home "mkdir -p .local/share/applications && printf '%s\n' '[Desktop Entry]' 'Type=Application' 'Name=vmlab opener' \
    'Exec=/run/vmlab-backups/opener %f' 'MimeType=text/plain;' 'NoDisplay=true' >.local/share/applications/vmlab-opener.desktop &&
    xdg-mime default vmlab-opener.desktop text/plain"

when() { date -d "@$(jq -r --arg id "$1" '.points[] | select(.id == $id) | .time' <<<"${points}")" '+%Y-%m-%d %H.%M'; }
B1_WHEN="$(when "${B1}")"

out="$(restore put-back "${B1}" Documents/report.txt --keep-both)"
json_check "put-back --keep-both from the backup: kept beside" "${out}" '.kept == 1'
check "the backup's version is beside the newer one, its time in its name" \
    grep -qx "version 1b" "${HOME_DIR}/Documents/report (${B1_WHEN}).txt"
check "and the newer one is untouched" grep -qx "version 2" "${HOME_DIR}/Documents/report.txt"

out="$(restore put-back "${B1}" Documents/gone.txt --keep-both)"
json_check "put-back of a file deleted since, from the backup" "${out}" '.added == 1'
check "it is back as it was" grep -qx gone "${HOME_DIR}/Documents/gone.txt"
check "it has the label SELinux gives its folder (user_home_t), not the cache's" \
    sh -c "stat -c %C '${HOME_DIR}/Documents/gone.txt' | grep -q ':user_home_t:' && [ -z \"\$(restorecon -nv '${HOME_DIR}/Documents/gone.txt')\" ]"
rm -f "${HOME_DIR}/.ssh/vmlab-known"
out="$(restore put-back "${B1}" .ssh/vmlab-known --keep-both)"
json_check "put-back into ~/.ssh from the backup" "${out}" '.added == 1'
check "it has the label of ~/.ssh (ssh_home_t)" \
    sh -c "stat -c %C '${HOME_DIR}/.ssh/vmlab-known' | grep -q ':ssh_home_t:' && [ -z \"\$(restorecon -nv '${HOME_DIR}/.ssh/vmlab-known')\" ]"
check "what the backup was brought down into is gone again" \
    sh -c "[ -z \"\$(find '${HOME_DIR}/.cache/amethystora-restore' -maxdepth 1 -name 'run.*' 2>/dev/null)\" ]"

out="$(restore put-back "${S1}" Documents/report.txt --replace)"
json_check "put-back --replace from a snapshot" "${out}" '.replaced == 1'
check "the snapshot's version took the newer one's place" grep -qx "version 1" "${HOME_DIR}/Documents/report.txt"

rm -f "${HOME_DIR}/Documents/notes/b.txt"
out="$(restore put-back "${S1}" Documents/notes --skip)"
json_check "put-back --skip of a folder (cp --update=none)" "${out}" '.skipped == 1'
check "--skip brings back what is missing" grep -qx "b 1" "${HOME_DIR}/Documents/notes/b.txt"
check "--skip leaves what differs as it is" grep -qx "a 2" "${HOME_DIR}/Documents/notes/a.txt"
check "--skip deletes nothing the snapshot lacks" grep -qx "c 2" "${HOME_DIR}/Documents/notes/c.txt"

rm -f "${HOME_DIR}/Documents/big.bin"
out="$(restore put-back "${S1}" Documents/big.bin --keep-both)"
json_check "put-back of a large file from a snapshot" "${out}" '.added == 1'
exclusive="$(btrfs filesystem du -s --raw "${HOME_DIR}/Documents/big.bin" 2>&1 | awk 'NR == 2 { print $2 }')"
if [[ "${exclusive}" == 0 ]] && cmp -s "${HOME_DIR}/Documents/big.bin" "${SNAPSHOTS}/${S1}/${TEST_USER}/Documents/big.bin"; then
    ok "it shares the snapshot's blocks (a reflink: no time, no space)"
else
    not_ok "the file put back does not share the snapshot's blocks" "$(btrfs filesystem du -s --raw "${HOME_DIR}/Documents/big.bin" 2>&1)"
fi

ln -sfn /etc "${HOME_DIR}/escape"
before="$(sha256sum /etc/passwd)"
for path in ../../etc/passwd /etc/passwd -x escape/passwd; do
    json_check "put-back refuses ${path}" "$(restore put-back "${S1}" "${path}" --replace)" '.error == "outside"'
done
check "nothing outside the home was written" sh -c "[ \"\$(sha256sum /etc/passwd)\" = '${before}' ]"

rm -f "${OPENED}"
out="$(as_tester env XDG_CURRENT_DESKTOP=GNOME /usr/libexec/amethystora-restore open "${B1}" Documents/report.txt --json)"
opened="$(jq -r '.opened // empty' <<<"${out}")"
if [[ "${opened}" == "${HOME_DIR}/.cache/amethystora-restore/look/${B1}."*/report.txt ]] && grep -qx "version 1b" "${opened}"; then
    ok "open from the backup: a copy under the cache"
else
    not_ok "open from the backup did not bring the copy down" "${out}"
fi
check "the copy is read-only to the account" sh -c "! runuser -u ${TEST_USER} -- test -w '${opened}'"
check "it is handed to the app that opens text" wait_for 30 grep -qxF "${opened}" "${OPENED}"
out="$(as_tester env XDG_CURRENT_DESKTOP=GNOME /usr/libexec/amethystora-restore open "${S1}" Documents/report.txt --json)"
json_check "open from a snapshot: the snapshot's own file" "${out}" ".opened == \"${SNAPSHOTS}/${S1}/${TEST_USER}/Documents/report.txt\""

# --- The window: putting back ----------------------------------------------------------------------------

in_home 'printf "version 3\n" >Documents/report.txt && printf "a 3\n" >Documents/notes/a.txt'
S2_WHEN="$(when "${S2}")"
show_browse
if go_to "${S2}" "Your home folder" && open_folder Documents "${S2}"; then
    ok "the window shows Documents as it was at a snapshot"
    check "it marks what is gone and what changed since" rows_marked
    select_file report.txt
    if atspi click "Put back" button >/dev/null && atspi wait 30 "Keep both" >/dev/null; then
        if atspi_state "Keep both" checked; then
            ok "Put back asks what to do with what is there now, Keep both first"
        else
            not_ok "the question does not have Keep both first" "$(says | tail -n 20)"
        fi
        # The page behind the question is hidden from the accessibility tree while it is asked: this is its own
        atspi click "Put back" button >/dev/null
    else
        not_ok "Put back asks nothing" "$(says | tail -n 20)"
    fi
    banner_says "is beside the newer one" "keep both puts it back beside"
    check "the snapshot's version is beside, the newer one untouched" sh -c \
        "grep -qx 'version 2' '${HOME_DIR}/Documents/report (${S2_WHEN}).txt' && grep -qx 'version 3' '${HOME_DIR}/Documents/report.txt'"

    select_file report.txt
    check "Versions lists every version that differs" atspi click Versions button
    if wait_for 300 versions_read; then
        said="$(says)"
        # One line of each row says where it is kept and when it was changed
        rows="$(grep -c -e '^On this machine · ' -e '^In the backup · ' <<<"${said}")"
        if ((rows >= 3)) && grep -q '^On this machine · ' <<<"${said}" && grep -q '^In the backup · ' <<<"${said}" &&
            ! grep -qF "The backup could not be read" <<<"${said}"; then
            ok "with a row each, from both sources"
        else
            not_ok "the versions are not from both sources" "${rows} rows" "$(tail -n 30 <<<"${said}")"
        fi
        atspi click "Back to the folder" button >/dev/null
    else
        not_ok "the versions were not found within 5 minutes" "$(says | tail -n 20)"
    fi
else
    not_ok "the window did not show Documents at ${S2}" "$(says | tail -n 30)"
fi

if go_to "${B2}" Documents; then
    lines="$(wc -l <"${OPENED}" 2>/dev/null || echo 0)"
    select_file report.txt
    check "Look, at a point in the backup" atspi click Look button
    banner_says "Opened read-only" "says it opened a read-only copy"
    check "and handed the backup's copy to the app" wait_for 30 sh -c \
        "tail -n +$((lines + 1)) '${OPENED}' | grep -q '/.cache/amethystora-restore/look/${B2}\.'"
else
    not_ok "the window did not go to the backup ${B2}" "$(says | tail -n 30)"
fi

if go_to "${S1}" Documents && open_folder notes "${S1}"; then
    if atspi click "Put the whole folder back" button >/dev/null && atspi wait 30 "nothing is deleted" >/dev/null; then
        ok "Put the whole folder back asks first"
        atspi click "Put the folder back" button >/dev/null
    else
        not_ok "Put the whole folder back gives no warning" "$(says | tail -n 20)"
    fi
    banner_says "What had the same name was replaced." "puts it back"
    check "what had the same name is the snapshot's again" grep -qx "a 1" "${HOME_DIR}/Documents/notes/a.txt"
    check "what the snapshot lacked is still there" grep -qx "c 2" "${HOME_DIR}/Documents/notes/c.txt"
else
    not_ok "the window did not show Documents/notes at ${S1}" "$(says | tail -n 30)"
fi

# --- The window: opened on a folder ----------------------------------------------------------------------

folders="$(as_tester env XDG_CURRENT_DESKTOP=GNOME gio mime inode/directory 2>&1)"
grep -q '^Default application.*org.gnome.Nautilus.desktop' <<<"${folders}" && ok "Files stays the default for folders" ||
    not_ok "Files is not the default for folders" "${folders}"
grep -q 'amethystora-backups.desktop' <<<"${folders}" && ok "Backups is offered to open a folder (Open With)" ||
    not_ok "Backups is not among the apps that open a folder" "${folders}"
# As Files' Open With does it: the launcher with the folder, which the open window takes over. The window is
# on the home folder first, so that only the launch can take it to the folder.
atspi click Home button >/dev/null
atspi wait 60 "Your home folder as it was" >/dev/null
user_bus systemd-run --user --collect --quiet -p KillMode=process -- \
    gio launch /usr/share/applications/amethystora-backups.desktop "${HOME_DIR}/Documents/notes"
check "Open With on a folder shows that folder in the open window" atspi wait 60 "notes as it was"
check "and the second start ends, leaving that window" wait_for 30 sh -c \
    "! pgrep -u ${TEST_USER} -f '^${BINARY} ${HOME_DIR}/Documents/notes'"
close_window
window "--at=${S1}" "${HOME_DIR}/Documents"
check "amethystora-backups --at=POINT FOLDER opens there" shown_at "${S1}" Documents 120

# --- The window: when the backup goes wrong --------------------------------------------------------------

# The window offers the agent only while the account has the agentic features on, which are off as shipped
as_tester ame agent toggle on >/dev/null 2>&1
check "the account turns the agentic features on" as_tester amethystora-agent status
as_tester sh -c 'date -d "8 days ago" +%s >"$HOME/.local/state/amethystora/backup-last-success"'
json_check "a week without a good backup is stale" "$(restore status)" '.backup.stale'
report="$(as_tester /usr/libexec/amethystora-security-status --json 2>/dev/null)"
json_check "the security report warns about it (backup-runs)" "${report}" '.checks[] | select(.id == "backup-runs") | .state == "check"'
reload
show_status
if wait_for 120 stale_said; then
    ok "the window says so, at the top of every page too"
    asked="$(date '+%F %T')"
    check "with the agent offered" atspi click "Ask the agent" button
else
    not_ok "the window does not say the backup is stale" "$(says | tail -n 30)"
fi
# The terminal the agent reopens itself in is kitty's, which logs nothing of its command (Ptyxis made a scope the
# account's manager logged it with): the command is looked for in the account's processes, polled fast because the agent
# itself may end within a second (35-detection-coverage.sh leaves a claude that exits at once), and in the journal in
# case something logs it
agent_asked() {
    local end=$((SECONDS + 60))
    while ((SECONDS < end)); do
        pgrep -u "${TEST_USER}" -f 'amethystora-agent diagnose amethystora-backup.service' >/dev/null && return 0
        journalctl --since "${asked:-now}" --no-pager -o cat _UID="$(id -u "${TEST_USER}")" |
            grep -qF 'amethystora-agent diagnose amethystora-backup.service' && return 0
        sleep 0.2
    done
    return 1
}
check "Ask the agent hands it the backup's unit" agent_asked
pkill -u "${TEST_USER}" -f 'amethystora-agent diagnose' 2>/dev/null
as_tester sh -c 'date +%s >"$HOME/.local/state/amethystora/backup-last-success"'
report="$(as_tester /usr/libexec/amethystora-security-status --json 2>/dev/null)"
json_check "the report's check is good again after a good backup" "${report}" '.checks[] | select(.id == "backup-runs") | .state == "ok"'

# A backup that cannot be read: the wrong key for the repository
as_tester sh -c "printf wrong >\"\$HOME/.config/amethystora/backup-password\""
reload
show_browse
wait_for 180 has "The backup could not be read"
said="$(says)"
if grep -qF "The backup could not be read" <<<"${said}" && grep -q ', On this machine$' <<<"${said}"; then
    ok "with the backup unreadable, the snapshots are still shown and it says why"
else
    not_ok "it does not say the backup could not be read" "$(tail -n 30 <<<"${said}")"
fi
show_status
atspi click "Back up now" button >/dev/null
if wait_for 300 has "The backup did not succeed"; then
    said="$(says)"
    if grep -qF "wrong password" <<<"${said}" && grep -qx "Ask the agent" <<<"${said}"; then
        ok "a backup that fails says so, with what it logged and the agent"
    else
        not_ok "the failed backup's log or the agent is not shown" "$(tail -n 30 <<<"${said}")"
    fi
else
    not_ok "the failing backup did not end within 5 minutes" "$(says | tail -n 30)"
fi
as_tester sh -c "printf '%s' '${REPO_PASSWORD}' >\"\$HOME/.config/amethystora/backup-password\""
as_tester ame agent toggle off >/dev/null 2>&1

check "Change… opens ame backup setup in a terminal" atspi click "Change…" button
check "where the recipe runs" wait_for 60 pgrep -u "${TEST_USER}" -f '00-entry.just backup setup'
pkill -u "${TEST_USER}" -f 'amethystora-in-terminal ame backup|ame backup setup' 2>/dev/null
check "Security settings opens Security" atspi click "Security settings" button
check "which starts" wait_for 90 pgrep -u "${TEST_USER}" -f '^/usr/lib/amethystora-security/amethystora-security'
pkill -TERM -o -u "${TEST_USER}" -f '^/usr/lib/amethystora-security/amethystora-security' 2>/dev/null

close_window
dumps="$(crashes_since "${since}" COREDUMP_UID="${uid}")"
if [[ -z "${dumps}" ]]; then ok "nothing crashed"; else not_ok "something crashed" "${dumps}"; fi
finish
