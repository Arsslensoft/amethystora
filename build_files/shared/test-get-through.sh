#!/usr/bin/bash
# The mark of the pill's Get me through (R7-45), against a stand-in for busctl: while it is there, written by a GNOME Shell
# that still runs, and the machine is on battery, the account's timed backup and containers' upgrade wait as they do on a
# metered connection (the same stamp, so never longer than a week), a backup to a disk of this machine never waits, and a
# run a person started, the system update, a machine on mains, a mark left by a shell that is gone and the mark's absence
# all go as before.
#   test-get-through.sh /path/to/amethystora-metered
set -euo pipefail

METERED="${1:?the helper}"
WORK="$(mktemp -d)"
trap 'kill "${SHELL_PID:-0}" 2>/dev/null || true; rm -rf -- "${WORK}"' EXIT
mkdir -p "${WORK}/bin" "${WORK}/run/amethystora" "${WORK}/state" "${WORK}/config/amethystora" "${WORK}/disk"
cat >"${WORK}/bin/busctl" <<'STANDIN'
#!/usr/bin/bash
case "$*" in
    *org.freedesktop.UPower*) echo "${STANDIN_BATTERY:-b false}" ;;
    *) echo "${STANDIN_METERED:-u 2}" ;;
esac
STANDIN
chmod +x "${WORK}/bin/busctl"
# A process that calls itself gnome-shell, as the shell that writes the mark does
cp "$(command -v sleep)" "${WORK}/bin/gnome-shell"
"${WORK}/bin/gnome-shell" 300 &
SHELL_PID=$!

run() {
    env PATH="${WORK}/bin:${PATH}" XDG_RUNTIME_DIR="${WORK}/run" XDG_STATE_HOME="${WORK}/state" \
        XDG_CONFIG_HOME="${WORK}/config" "$@"
}
expect() {
    local what="$1" want="$2" got=0
    shift 2
    run "$@" >/dev/null 2>&1 || got=$?
    [[ "${got}" == "${want}" ]] || { echo "test-get-through: ${what}: exit ${got}, expected ${want}"; exit 1; }
}
MARK="${WORK}/run/amethystora/get-through"
STAMP="${WORK}/state/amethystora/metered/backup"

ON=(STANDIN_BATTERY="b true" TRIGGER_UNIT=x.timer)
OFF=(STANDIN_BATTERY="b false" TRIGGER_UNIT=x.timer)

# No mark: all as before
expect "no mark, on battery: the backup runs" 0 "${ON[@]}" "${METERED}" wait backup
expect "no mark, on battery: the containers run" 0 "${ON[@]}" "${METERED}" wait containers

# The mark holds the process number of whoever wrote it, in a folder only the account enters
mine="$(run bash -c '"$0" through on; echo $$' "${METERED}")"
[[ "$(tr -dc 0-9 <"${MARK}")" == "${mine}" ]] || { echo "test-get-through: the mark is not the caller's process"; exit 1; }
rm -rf -- "${WORK}/run/amethystora"
run "${METERED}" through on
[[ "$(stat -c %a "${WORK}/run/amethystora")" == 700 ]]

# A mark left by a process that is not a GNOME Shell (gone, or another program's number) is ignored
expect "a mark of a shell that is gone: the backup runs" 0 "${ON[@]}" "${METERED}" wait backup
printf '%s\n' "$$" >"${MARK}"
expect "a mark of another program: the backup runs" 0 "${ON[@]}" "${METERED}" wait backup
printf 'x\n' >"${MARK}"
expect "a mark with no number: the containers run" 0 "${ON[@]}" "${METERED}" wait containers

printf '%s\n' "${SHELL_PID}" >"${MARK}"
expect "the mark, on battery: the backup waits" 1 "${ON[@]}" "${METERED}" wait backup
expect "the mark, on battery: the containers wait" 1 "${ON[@]}" "${METERED}" wait containers
[[ -s "${STAMP}" ]] || { echo "test-get-through: the first wait was not counted"; exit 1; }
expect "the mark, on mains: the backup runs" 0 "${OFF[@]}" "${METERED}" wait backup
[[ ! -e "${STAMP}" ]] || { echo "test-get-through: a run on mains kept the stamp"; exit 1; }
expect "the mark, on mains: the containers run" 0 "${OFF[@]}" "${METERED}" wait containers
expect "the mark: a run a person started goes" 0 STANDIN_BATTERY="b true" TRIGGER_UNIT= "${METERED}" wait backup
# As root, the system update never reads an account's mark
if [[ "$(id -u)" == 0 ]]; then
    expect "the mark: the system update is uupd's own business" 0 "${ON[@]}" "${METERED}" wait system
fi

# Never longer than a week: the stamp is the first wait, and seven days later the run goes
expect "the wait counts" 1 "${ON[@]}" "${METERED}" wait containers
echo "$(($(date +%s) - 6 * 24 * 3600))" >"${WORK}/state/amethystora/metered/containers"
expect "a week less a day: still waiting" 1 "${ON[@]}" "${METERED}" wait containers
echo "$(($(date +%s) - 7 * 24 * 3600 - 5))" >"${WORK}/state/amethystora/metered/containers"
expect "a week: it runs" 0 "${ON[@]}" "${METERED}" wait containers
# And the week is the metered connection's too: one count for both
rm -f -- "${WORK}/state/amethystora/metered/containers"
expect "metered first" 1 STANDIN_BATTERY="b false" STANDIN_METERED="u 1" TRIGGER_UNIT=x.timer "${METERED}" wait containers
echo "$(($(date +%s) - 8 * 24 * 3600))" >"${WORK}/state/amethystora/metered/containers"
expect "metered and the mark together: a week from the first wait" 0 STANDIN_METERED="u 1" "${ON[@]}" "${METERED}" wait containers

# A backup to a disk of this machine never waits, mark or not
rm -f -- "${STAMP}" "${WORK}/state/amethystora/metered/containers"
printf 'export RESTIC_REPOSITORY="%s"\n' "${WORK}/disk" >"${WORK}/config/amethystora/backup.env"
expect "the mark: a backup to this machine's disk runs" 0 "${ON[@]}" "${METERED}" wait backup
expect "the mark: the containers still wait" 1 "${ON[@]}" "${METERED}" wait containers
[[ ! -e "${STAMP}" ]] || { echo "test-get-through: a local backup was counted"; exit 1; }
printf 'export RESTIC_REPOSITORY="sftp:host:/srv/backups"\n' >"${WORK}/config/amethystora/backup.env"
expect "the mark: a backup to another machine waits" 1 "${ON[@]}" "${METERED}" wait backup

run "${METERED}" through off
test ! -e "${MARK}"
expect "the mark gone: the backup runs" 0 "${ON[@]}" "${METERED}" wait backup
expect "a word it does not know" 2 "${METERED}" through maybe
echo "test-get-through: passed"
