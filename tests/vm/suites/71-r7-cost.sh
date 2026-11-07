#!/usr/bin/bash
# What round 7 costs the shell at rest (plans/done/PLAN-ui-ideas-r7-build.md, Performance): its memory, how often it wakes,
# what it uses and how many frames it draws while nothing happens, and how long it takes to start. The numbers are
# printed; with a baseline beside this file (71-r7-cost.baseline, the numbers of an image without round 7) they are held
# to the plan's limits. Only the maintainer raises a limit. R7_COST_IDLE: seconds at rest (default 300).
# The lab renders in software, so frame times are the real machine's to check; memory and wakeups count here.
# vmlab-paths: @sf/usr/share/gnome-shell/extensions/amethystora-ui@*/* @sf/usr/lib/systemd/user/amethystora-*
source "$(dirname "$0")/lib.sh"

IDLE="${R7_COST_IDLE:-300}"
BASELINE="$(dirname "$0")/71-r7-cost.baseline"

ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
record() { echo "# cost $1 $2"; printf '%s %s\n' "$1" "$2" >>"${RESULT}"; }
field() { awk -v k="$2" '$1 == k { print $2 }' "$1"; }

if ! wait_for 90 has_session; then
    systemctl restart gdm
    if ! wait_for 120 has_session; then
        skip "no session for ${TEST_USER} (10-account.sh)"
        finish
    fi
fi
RESULT="$(mktemp)"
# A new session, so that what is measured is a login, with the lab's Eval for the frame count (as suite 13)
EXT="/var/home/${TEST_USER}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${EXT}"
printf '{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab'"'"'s Eval", "shell-version": ["49", "50", "51"]}\n' >"${EXT}/metadata.json"
printf 'export default class { enable() { global.context.unsafe_mode = true; } disable() {} }\n' >"${EXT}/extension.js"
chown -R "${TEST_USER}:" "${EXT}"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
loginctl unlock-sessions
systemctl restart gdm
eval_ready() { [[ "$(ev "typeof global.stage")" == object ]]; }
# GDM 51 now and then shows its greeter instead of signing the account in again: once more (as live.mjs does)
if ! wait_for 90 eval_ready; then
    systemctl restart gdm
    if ! wait_for 120 eval_ready; then
        not_ok "the session came back with the lab's Eval"
        finish
    fi
fi
# GNOME 51's unit is @user, GNOME 50's @wayland
for UNIT in org.gnome.Shell@user.service org.gnome.Shell@wayland.service; do
    PID="$(user_systemctl show "${UNIT}" -p MainPID --value)"
    [[ -n "${PID}" && "${PID}" != 0 ]] && break
done
if [[ -z "${PID}" || "${PID}" == 0 ]]; then
    not_ok "GNOME Shell's process is known"
    finish
fi

started="$(user_systemctl show "${UNIT}" -p InactiveExitTimestampMonotonic --value)"
ready="$(user_systemctl show "${UNIT}" -p ActiveEnterTimestampMonotonic --value)"
[[ "${ready:-0}" -gt "${started:-0}" ]] && record shell_start_ms "$(((ready - started) / 1000))"

# Let the login's work settle before anything is measured
sleep 60
ctx() { awk '/^(non)?voluntary_ctxt_switches/ { n += $2 } END { print n }' "/proc/${PID}/status"; }
ticks() { awk '{ print $14 + $15 }' "/proc/${PID}/stat"; }

record shell_rss_mb "$(awk '/^VmRSS/ { print int($2 / 1024) }' "/proc/${PID}/status")"
ev "global.__r7cost = {frames: 0}; global.__r7cost.id = global.stage.connect('after-paint', () => { global.__r7cost.frames++; }); 0" >/dev/null
ctx0="$(ctx)" ticks0="$(ticks)"
sleep "${IDLE}"
ctx1="$(ctx)" ticks1="$(ticks)"
frames="$(ev "global.stage.disconnect(global.__r7cost.id); global.__r7cost.frames")"
record shell_wakeups_per_min "$(((ctx1 - ctx0) * 60 / IDLE))"
record shell_ticks_per_min "$(((ticks1 - ticks0) * 60 / IDLE))"
[[ "${frames}" =~ ^[0-9]+$ ]] && record shell_frames_per_min "$((frames * 60 / IDLE))"
record shell_rss_after_idle_mb "$(awk '/^VmRSS/ { print int($2 / 1024) }' "/proc/${PID}/status")"

check "the shell did not restart while it was measured" test "$(user_systemctl show "${UNIT}" -p MainPID --value)" == "${PID}"

if [[ ! -s "${BASELINE}" ]]; then
    skip "no baseline beside this suite: the numbers above are only recorded"
else
    within() {
        local name="$1" extra="$2" base now
        base="$(field "${BASELINE}" "${name}")" now="$(field "${RESULT}" "${name}")"
        if [[ -z "${base}" || -z "${now}" ]]; then
            skip "${name}: no number to compare (baseline: ${base:-none}, now: ${now:-none})"
        elif ((now <= base + extra)); then
            ok "${name}: ${now}, within ${extra} of the baseline's ${base}"
        else
            not_ok "${name}: ${now}, more than ${extra} above the baseline's ${base}"
        fi
    }
    # The plan's limits. One run decides nothing on a busy lab: the same code on a loaded host differed by 14 MB and 47 wakeups a
    # minute between two runs (2026-11-02). The baseline and the run compared with it are each the median of three on a quiet lab
    within shell_rss_mb 15
    within shell_start_ms 100
    within shell_wakeups_per_min 6
    within shell_ticks_per_min 3
    within shell_frames_per_min 1
fi
rm -f "${RESULT}"
finish
