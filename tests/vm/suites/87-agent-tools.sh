#!/usr/bin/bash
# The AI agent's tools (plans/done/PLAN-agentic-os.md), as an account uses them through an agent: an MCP client
# (mcp.py) starts /usr/libexec/amethystora-agent-tools as an agent would, and polkit's password prompts are
# answered as the person at the machine would (pkagent.py, the test account's own password). One phase:
#   - off as shipped, and the update's one notice to an account that had the agent on
#   - level 1: every state topic, search, read, logs, history; level 2 not listed
#   - level 2: a setting changed and undone, a Flatpak installed and removed, a hand-off answered in a terminal
#   - root in the session mode: one password for two commands, the helper ends when the screen locks and when
#     the agent ends, its socket out of reach of another process, a notification and a journal line each time;
#     then the minutes, each and terminal modes once each
#   - the lock (know, off, one account's own), an agent started with its own checks off, the health check on a
#     unit broken on purpose, the watcher quiet about the tools' own entry, and everything off again
# vmlab-paths: apps/agent-tools/* @sf/usr/bin/amethystora-agent @sf/usr/libexec/amethystora-agent* @sf/usr/share/amethystora/agents/* @sf/usr/libexec/amethystora-doctor @sf/usr/libexec/amethystora-changes
source "$(dirname "$0")/lib.sh"

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
OWN=/run/vmlab-agent
mkdir -p "${OWN}"
cp "$(dirname "$0")/mcp.py" "$(dirname "$0")/pkagent.py" "${OWN}/"
chmod -R a+rX "${OWN}"
HOME_DIR="/var/home/${TEST_USER}"
CONFIG="${HOME_DIR}/.config/amethystora"
TOOLS=/usr/libexec/amethystora-agent-tools
CONF=/etc/amethystora/agent.conf
PASSWORD="${TEST_USER}"
cp -a "${CONF}" "${OWN}/agent.conf.before"

# The client as the account, as an agent started from a terminal is: a process of the account's user manager,
# which polkit counts as part of its desktop session. runuser's own session has no seat, and polkit refuses
# such a process outright.
mcp() {
    user_bus systemd-run --user --pipe --wait --quiet --collect --setenv=WAYLAND_DISPLAY=wayland-0 \
        -- /usr/bin/python3 "${OWN}/mcp.py" "$@" </dev/null 2>/dev/null
}
# One answer of mcp's: the text of the call named, the Nth of them (default the first)
answer() { jq -rs --arg c "$2" --argjson n "${3:-0}" '[.[] | select(.call? == $c)][$n].text // ""' <<<"$1"; }
failed() { jq -es --arg c "$2" --argjson n "${3:-0}" '[.[] | select(.call? == $c)][$n].error' <<<"$1" >/dev/null; }
listed() { jq -rs '[.[] | select(.list?)][0].list | join(" ")' <<<"$1"; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
as_tester() { runuser -u "${TEST_USER}" -- "$@"; }
tester_ame() { user_bus bash -lc "$*" </dev/null; }

# --- Off as shipped, and the update's notice -----------------------------------------------------------------

# An account made now has them off; the test account starts this suite off too, whatever an earlier suite
# (80-control's agent card) left
useradd -m vmlabfresh 2>/dev/null
if runuser -u vmlabfresh -- amethystora-agent status >/dev/null 2>&1; then
    not_ok "the agentic features are off for an account as shipped"
else
    ok "the agentic features are off for an account as shipped"
fi
same "with them off, the tools list nothing" \
    "$(runuser -u vmlabfresh -- /usr/libexec/amethystora-agent-tools mcp <<<'{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq -r '[.result.tools[].name] | join(" ")')" ""
userdel -r vmlabfresh 2>/dev/null
as_tester rm -f "${CONFIG}/agentic" "${CONFIG}/agent-tools"
tester_ame "amethystora-agent sync" >/dev/null
same "and for the test account, set back off" "$(listed "$(mcp list)")" ""

# An account that had the agent on under the old image: its skills linked, no marker. The hook that runs before
# 13-agentic.sh says so once; one that had it off (no-agentic) is only cleaned up.
as_tester mkdir -p "${HOME_DIR}/.claude/skills"
as_tester ln -sfn /usr/share/amethystora/agents/skills/amethystora "${HOME_DIR}/.claude/skills/amethystora"
CHECKER="${HOME_DIR}/.local/share/amethystora/setup_versioning.json"
[[ -f "${CHECKER}" ]] && as_tester sh -c "jq 'del(.version.user.\"agentic-off\")' '${CHECKER}' >'${CHECKER}.new' && mv '${CHECKER}.new' '${CHECKER}'"
before="$(user_systemctl list-units --all --plain --no-legend 'run-*' | awk '{print $1}' | sort)"
user_bus bash /usr/share/amethystora/user-setup.hooks.d/13-agentic-off.sh >/dev/null 2>&1
noticed=""
for unit in $(comm -13 <(echo "${before}") <(user_systemctl list-units --all --plain --no-legend 'run-*' | awk '{print $1}' | sort)); do
    user_systemctl show -p ExecStart "${unit%.timer}.service" 2>/dev/null | grep -q amethystora-agentic-off-notice && noticed=yes
done
same "an account that had the agent on is told once that it is now off" "${noticed}" yes
user_bus bash /usr/share/amethystora/user-setup.hooks.d/13-agentic-off.sh >/dev/null 2>&1
check "the hook says it only once (its version is recorded)" jq -e '.version.user."agentic-off" == "1"' "${CHECKER}"
as_tester rm -f "${HOME_DIR}/.claude/skills/amethystora"

# --- Level 1 ---------------------------------------------------------------------------------------------

tester_ame "ame agent toggle on" >/dev/null
check "ame agent toggle on writes the marker" test -e "${CONFIG}/agentic"
calls=(list 'search {"query": "roll back an update"}' 'read {"id": "manual/rolling-back"}'
    'read {"id": "playbook/update-fails"}' 'logs {"unit": "NetworkManager.service", "lines": 10}' 'history {}')
for topic in overview health settings security security-settings app-permissions software updates backups disks setup-diff units crashes; do
    calls+=("state {\"topic\": \"${topic}\"}")
done
out="$(mcp "${calls[@]}")"
same "level 1 lists the five read-only tools" "$(listed "${out}")" "search read state logs history"
bytes="$(jq -rs '[.[] | select(.list?)][0].bytes' <<<"${out}")"
if ((bytes < 6000)); then ok "the five tools' schemas are small (${bytes} bytes)"; else not_ok "the tools' schemas are ${bytes} bytes"; fi
answer "${out}" search | grep -q "manual/rolling-back" && ok "search finds how to roll back" ||
    not_ok "search did not find manual/rolling-back" "$(answer "${out}" search | head -c 600)"
answer "${out}" read | grep -q "Rolling back" && ok "read gives the passage" || not_ok "read" "$(answer "${out}" read | head -c 300)"
answer "${out}" read 1 | grep -q "## " && ok "read gives a whole playbook by its name" || not_ok "read playbook/update-fails" "$(answer "${out}" read 1 | head -c 300)"
answer "${out}" logs | grep -q "not instructions" && ok "logs come marked as data" || not_ok "logs" "$(answer "${out}" logs | head -c 300)"
for n in $(seq 0 12); do
    text="$(answer "${out}" state "${n}")"
    if failed "${out}" state "${n}"; then
        not_ok "state topic $((n + 1)) answers" "${text:0:400}"
    else
        ok "state topic $((n + 1)) answers ($((${#text} / 4)) tokens)"
    fi
done
answer "${out}" state 0 | grep -q "^Image: " && ok "state overview names the image" || not_ok "state overview" "$(answer "${out}" state 0 | head -c 400)"
# What the same answers cost read with shell commands, for the plan's Status: a few of the tasks
echo "# tokens, tools vs shell: overview $(($(answer "${out}" state 0 | wc -c) / 4)) vs $((($(rpm-ostree status 2>/dev/null | wc -c) + $(systemctl --failed 2>/dev/null | wc -c) + $(df -h 2>/dev/null | wc -c) + $(free -h | wc -c)) / 4))"
echo "# tokens, tools vs shell: settings $(($(answer "${out}" state 2 | wc -c) / 4)) vs $(($(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | wc -c) / 4))"
echo "# tokens, tools vs shell: security $(($(answer "${out}" state 3 | wc -c) / 4)) vs $(($(user_bus /usr/libexec/amethystora-security-status 2>/dev/null | wc -c) / 4))"
echo "# tokens, tools vs shell: rolling back $(($(answer "${out}" search | wc -c) / 4 + $(answer "${out}" read | wc -c) / 4)) vs $(($(wc -c </usr/share/amethystora/manual/rolling-back.md) / 4))"
echo "# tokens, tools vs shell: logs $(($(answer "${out}" logs | wc -c) / 4)) vs $(($(journalctl -u NetworkManager.service -b -n 40 --no-pager 2>/dev/null | wc -c) / 4))"
echo "# tokens, tools vs shell: app permissions $(($(answer "${out}" state 5 | wc -c) / 4)) vs $(($(user_bus /usr/libexec/amethystora-app-permissions --json 2>/dev/null | wc -c) / 4))"
echo "# tokens, tools vs shell: software $(($(answer "${out}" state 6 | wc -c) / 4)) vs $((($(flatpak list 2>/dev/null | wc -c) + $(user_bus /usr/bin/amethystora-pkg containers list --json 2>/dev/null | wc -c)) / 4))"
echo "# tokens, tools vs shell: updates $(($(answer "${out}" state 7 | wc -c) / 4)) vs $((($(rpm-ostree status 2>/dev/null | wc -c) + $(journalctl -u uupd.service -n 50 --no-pager 2>/dev/null | wc -c) + $(fwupdmgr get-updates 2>/dev/null | wc -c)) / 4))"
echo "# tokens, tools vs shell: backups $(($(answer "${out}" state 8 | wc -c) / 4)) vs $(($(user_bus /usr/libexec/amethystora-restore status --json 2>/dev/null | wc -c) / 4))"
echo "# tokens, tools vs shell: disks $(($(answer "${out}" state 9 | wc -c) / 4)) vs $((($(/usr/libexec/amethystora-raid status --json 2>/dev/null | wc -c) + $(df -h 2>/dev/null | wc -c)) / 4))"

# --- Level 2 ---------------------------------------------------------------------------------------------

out="$(mcp 'change {"id": "transition", "value": "fade"}')"
answer "${out}" change | grep -q "Changes are off" && ok "level 2 refuses while changes are off" || not_ok "change before level 2" "$(answer "${out}" change)"
tester_ame "ame agent tools on" >/dev/null
out="$(mcp list)"
same "level 2 adds the five tools that change things" "$(listed "${out}")" "search read state logs history change software run undo plugin"

# A setting changed as Control changes it, and undone
style="$(user_bus gsettings get org.gnome.shell.extensions.amethystora-transitions style 2>/dev/null | tr -d "'")"
[[ "${style}" == wipe ]] && target=fade || target=wipe
out="$(mcp "change {\"id\": \"transition\", \"value\": \"${target}\"}")"
now="$(user_bus gsettings get org.gnome.shell.extensions.amethystora-transitions style 2>/dev/null | tr -d "'")"
same "change sets the transition with Control's own command" "${now}" "${target}"
record="$(answer "${out}" change | grep -o 'Record [0-9a-f]\{8\}' | awk '{print $2}')"
if [[ -n "${record}" ]]; then
    out="$(mcp "undo {\"id\": \"${record}\"}" "undo {\"id\": \"${record}\"}")"
    now="$(user_bus gsettings get org.gnome.shell.extensions.amethystora-transitions style 2>/dev/null | tr -d "'")"
    same "undo puts it back" "${now}" "${style}"
    failed "${out}" undo 1 && ok "a change is undone only once" || not_ok "undo twice" "$(answer "${out}" undo 1)"
else
    not_ok "change gave a record id" "$(answer "${out}" change)"
fi
out="$(mcp 'change {"id": "agent-root", "value": "each"}' 'change {"id": "aur", "value": "on"}')"
failed "${out}" change 0 && failed "${out}" change 1 && ok "change refuses the agent's own root mode and the AUR" ||
    not_ok "change of agent-root or aur" "$(answer "${out}" change 0)" "$(answer "${out}" change 1)"

# A small Flatpak, installed and taken away again (its download can take longer than a call)
APP=org.gnome.Mahjongg
# In the machine's installation, or the account's own where it has a Flathub of its own
installed() { flatpak info --system "${APP}" >/dev/null 2>&1 || user_bus flatpak info --user "${APP}" >/dev/null 2>&1; }
out="$(mcp "software {\"action\": \"install\", \"source\": \"flathub\", \"id\": \"${APP}\"}")"
job="$(answer "${out}" software | grep -o '[0-9a-f]\{8\}' | head -1)"
for _ in 1 2 3 4 5 6 7 8 9 10; do
    installed && break
    mcp "history {\"id\": \"${job}\", \"wait\": 60}" >/dev/null
done
if installed; then
    ok "software installs a Flatpak"
    out="$(mcp "undo {\"id\": \"${job}\"}" 'pause 30')"
    for _ in 1 2 3 4 5 6; do installed || break; sleep 10; done
    installed && not_ok "undo removed the Flatpak" "$(answer "${out}" undo)" || ok "undo removes it again"
else
    not_ok "software installs a Flatpak" "$(answer "${out}" software)"
fi

# The hand-off, on a terminal of its own: the command and the agent's reason shown, run only after a yes, its
# status and output written back
REPORT="$(as_tester mktemp -d)"
in_terminal 60 'Run it\? \[y/N\]' $'y\r' 'Press enter' $'\r' -- runuser -u "${TEST_USER}" -- \
    /usr/libexec/amethystora-in-terminal --agent $'testing \e[2J the hand-off' --report "${REPORT}" -- sh -c 'echo from-the-command; exit 4' \
    >"${OWN}/handoff.out" 2>&1
same "the hand-off reports the exit status" "$(cat "${REPORT}/status" 2>/dev/null)" 4
grep -q from-the-command "${REPORT}/output" 2>/dev/null && ok "the hand-off reports what it printed" || not_ok "the hand-off's output"
grep -q 'sh -c echo\\ from-the-command' "${OWN}/handoff.out" && ok "the hand-off shows the command as words" ||
    not_ok "the hand-off shows the command" "$(tail -c 600 "${OWN}/handoff.out")"
REPORT2="$(as_tester mktemp -d)"
in_terminal 60 'Run it\? \[y/N\]' $'n\r' -- runuser -u "${TEST_USER}" -- \
    /usr/libexec/amethystora-in-terminal --agent "no" --report "${REPORT2}" -- touch "${OWN}/should-not-exist" >/dev/null 2>&1
same "a declined hand-off runs nothing" "$(cat "${REPORT2}/status" 2>/dev/null)$([[ -e ${OWN}/should-not-exist ]] && echo ran)" declined
out="$(mcp 'run {"argv": ["bash", "-c", "id"], "reason": "x", "root": true}' 'run {"argv": ["ame", "security", "firewall", "off"], "reason": "x", "root": true}')"
failed "${out}" run 0 && failed "${out}" run 1 && ok "run refuses shells and loosening security" || not_ok "run's never list" "$(answer "${out}" run 0)"

# --- Root, in the session mode: one password for two commands ----------------------------------------------

started="$(date '+%F %T')"
# The notifications the account's desktop is sent, as its session bus carries them: amethystora-notify-users
# prints nothing to the journal when its caller keeps what it prints
user_bus timeout 90 busctl --user monitor org.freedesktop.Notifications >"${OWN}/notify.log" 2>&1 &
monitor=$!
sleep 1
mcp 'pause 6' 'run {"argv": ["id", "-u"], "reason": "the first root command", "root": true}' \
    'run {"argv": ["ls", "/root"], "reason": "the second, same session", "root": true}' 'pause 25' >"${OWN}/session.out" &
client=$!
server=""
for _ in $(seq 20); do
    server="$(jq -r 'select(.server_pid?) | .server_pid' "${OWN}/session.out" 2>/dev/null | head -1)"
    [[ -n "${server}" ]] && break
    sleep 0.5
done
as_tester python3 "${OWN}/pkagent.py" "${server:-1}" "${PASSWORD}" 50 >"${OWN}/session.prompts" 2>&1 &
sleep 12
helper="$(pgrep -f '^/usr/libexec/amethystora-agent-root session' | head -1)"
if [[ -n "${helper}" ]]; then
    ok "the session's helper runs, as root"
    # Another process of the account can open neither end of the socket: not through /proc/PID/fd (a socket cannot
    # be opened that way), not with pidfd_getfd (Yama keeps it from anyone but the parent), not the helper's at all
    reach="$(as_tester python3 - "${server}" "${helper}" <<'PY'
import ctypes, os, sys
server, helper = int(sys.argv[1]), int(sys.argv[2])
libc = ctypes.CDLL(None, use_errno=True)
reached = []
for pid in (server, helper):
    try:
        names = os.listdir(f"/proc/{pid}/fd")
    except OSError:
        continue
    for name in names:
        path = f"/proc/{pid}/fd/{name}"
        try:
            if not os.readlink(path).startswith("socket:"):
                continue
            os.close(os.open(path, os.O_RDWR))
            reached.append(f"open {path}")
        except OSError:
            pass
        pidfd = libc.syscall(434, pid, 0)
        if pidfd >= 0:
            fd = libc.syscall(438, pidfd, int(name), 0)
            if fd >= 0:
                reached.append(f"pidfd_getfd {pid}/{name}")
print(" ".join(reached))
PY
)"
    same "no other process of the account reaches the helper's socket" "${reach}" ""
else
    not_ok "the session's helper runs"
fi
wait "${client}"
sleep 2
same "the session asked for the password once" "$(cat "${OWN}/session.prompts" 2>/dev/null)" 1
out="$(cat "${OWN}/session.out")"
answer "${out}" run 0 | grep -q "It worked" && answer "${out}" run 0 | grep -q "│ 0" && ok "a root command runs as root after one password" ||
    not_ok "the first root command" "$(answer "${out}" run 0 | head -c 500)"
answer "${out}" run 1 | grep -q "It worked" && ok "a second runs in the same session" || not_ok "the second root command" "$(answer "${out}" run 1 | head -c 500)"
sleep 3
pgrep -f '^/usr/libexec/amethystora-agent-root' >/dev/null && not_ok "the helper ended with the agent" || ok "the helper ended with the agent"
journalctl -t amethystora-agent-root --since "${started}" --no-pager -o cat | grep -q "ran as root: id -u" &&
    ok "each root command has its journal line" || not_ok "the journal line" "$(journalctl -t amethystora-agent-root --since "${started}" --no-pager -o cat | tail -5)"
kill "${monitor}" 2>/dev/null
grep -q "The AI agent ran a command as root" "${OWN}/notify.log" &&
    ok "and its notification" || not_ok "the notification" "$(tail -c 600 "${OWN}/notify.log")"

# The screen locked ends the session's helper
mcp 'pause 4' 'run {"argv": ["true"], "reason": "before the lock", "root": true}' 'pause 40' >"${OWN}/lock.out" &
client=$!
sleep 2
server="$(jq -r 'select(.server_pid?) | .server_pid' "${OWN}/lock.out" 2>/dev/null | head -1)"
as_tester python3 "${OWN}/pkagent.py" "${server:-1}" "${PASSWORD}" 20 >/dev/null 2>&1 &
sleep 15
# The account's desktop session, not one of the sessions runuser opens
session="$(loginctl show-user "${TEST_USER}" -p Display --value)"
helper_gone() { ! pgrep -f '^/usr/libexec/amethystora-agent-root session' >/dev/null; }
if ! helper_gone; then
    locking="$(date '+%F %T')"
    loginctl lock-session "${session}"
    if wait_for 30 helper_gone; then
        ok "the session's helper ends when the screen locks"
    else
        not_ok "the helper outlived the lock" "session ${session}: $(loginctl show-session "${session}" -p Type -p LockedHint 2>&1 | paste -sd ' ')" \
            "$(journalctl -t amethystora-agent-root --since "${locking}" --no-pager -o cat | tail -3)"
    fi
    loginctl unlock-session "${session}"
else
    not_ok "a helper to lock out" "$(cat "${OWN}/lock.out" | tail -c 400)"
fi
wait "${client}"

# The other modes, once each
for mode in minutes each; do
    "${TOOLS}" conf root "${mode}" >/dev/null
    mcp 'pause 6' 'run {"argv": ["true"], "reason": "one", "root": true}' 'run {"argv": ["true"], "reason": "two", "root": true}' \
        >"${OWN}/${mode}.out" &
    client=$!
    sleep 2
    server="$(jq -r 'select(.server_pid?) | .server_pid' "${OWN}/${mode}.out" 2>/dev/null | head -1)"
    asked="$(as_tester python3 "${OWN}/pkagent.py" "${server:-1}" "${PASSWORD}" 60 2>/dev/null)"
    wait "${client}"
    [[ "${mode}" == minutes ]] && want=1 || want=2
    worked="$(jq -rs '[.[] | select(.call? == "run") | select(.text | test("It worked"))] | length' "${OWN}/${mode}.out")"
    same "the ${mode} mode: two root commands, ${want} password(s)" "${asked}/${worked}" "${want}/2"
done
"${TOOLS}" conf root terminal >/dev/null
out="$(mcp 'run {"argv": ["true"], "reason": "in a terminal", "root": true}')"
# Put to the person (the pill's card or a terminal, round 7), who also types their password for sudo
answer "${out}" run | grep -q "Put to the user, who reads it and says yes .* and types their password" &&
    ok "the terminal mode puts the command to the person, with sudo's password" || not_ok "the terminal mode" "$(answer "${out}" run)"
pkill -u "${TEST_USER}" -f 'amethystora-in-terminal' 2>/dev/null
"${TOOLS}" conf root session >/dev/null

# --- The lock --------------------------------------------------------------------------------------------

"${TOOLS}" conf allow know >/dev/null
same "the lock at know hides level 2" "$(listed "$(mcp list)")" "search read state logs history"
"${TOOLS}" conf allow off >/dev/null
same "the lock at off hides everything" "$(listed "$(mcp list)")" ""
as_tester amethystora-agent >/dev/null 2>&1 && not_ok "the launcher refuses when locked off" || ok "the launcher refuses when locked off"
"${TOOLS}" conf allow act >/dev/null
useradd -m vmlabother 2>/dev/null
"${TOOLS}" conf allow off "${TEST_USER}" >/dev/null
same "one account's own lock applies to it" "$(listed "$(mcp list)")" ""
same "and not to another" "$(runuser -u vmlabother -- "${TOOLS}" policy --ceiling)" act
"${TOOLS}" conf allow default "${TEST_USER}" >/dev/null
userdel -r vmlabother 2>/dev/null

# --- An agent with its own checks off --------------------------------------------------------------------

out="$(user_bus env WAYLAND_DISPLAY=wayland-0 python3 "${OWN}/mcp.py" --as-agent claude --dangerously-skip-permissions -- \
    'change {"id": "transition", "value": "fade"}' 'state {"topic": "units"}' 2>/dev/null)"
answer "${out}" change | grep -q "dangerously-skip-permissions" && ok "an agent with its checks off gets no change" ||
    not_ok "the checks-off refusal" "$(answer "${out}" change)"
failed "${out}" state && not_ok "level 1 still works for it" || ok "level 1 still works for it"

# A tool server that no agent of the launcher stands above (a program started it itself) gets no change and no root: its own
# environment without AMETHYSTORA_AGENT_LAUNCHED (the entry the launcher gives each agent carries it), or no agent above it
for how in --unmarked --bare; do
    out="$(user_bus env WAYLAND_DISPLAY=wayland-0 python3 "${OWN}/mcp.py" "${how}" \
        'change {"id": "transition", "value": "fade"}' 'run {"argv": ["true"], "reason": "x", "root": true}' 'state {"topic": "units"}' 2>/dev/null)"
    answer "${out}" change | grep -q "Changes are refused" && answer "${out}" run | grep -q "Changes are refused" &&
        ok "${how}: a tool server not started by an agent of the launcher gets no change and no root" ||
        not_ok "${how}: the launcher refusal" "$(answer "${out}" change) / $(answer "${out}" run)"
    failed "${out}" state && not_ok "${how}: level 1 still works" || ok "${how}: level 1 still works"
done

# --- The health check --------------------------------------------------------------------------------------

user_bus systemd-run --user --unit=vmlab-broken --quiet false >/dev/null 2>&1
sleep 2
user_bus /usr/libexec/amethystora-doctor --json | jq -e '.findings | any(.id == "failed-units" and (.evidence | test("vmlab-broken")))' >/dev/null &&
    ok "the doctor finds a unit broken on purpose" || not_ok "the doctor and a failed unit" "$(user_bus /usr/libexec/amethystora-doctor --json | head -c 600)"
mcp 'state {"topic": "health"}' | grep -q vmlab-broken && ok "state health says it" || not_ok "state health"
user_systemctl reset-failed vmlab-broken.service 2>/dev/null
# The lab's machine has failed units of its own (an akmods module refused under Secure Boot): the one broken here
# is what has to be gone
found="$(user_bus /usr/libexec/amethystora-doctor --json --only failed-units | jq -r '[.findings[].evidence] | join(" ")')"
[[ "${found}" != *vmlab-broken* ]] && ok "and verifies the fix" || not_ok "doctor --only after the fix" "${found}"

# --- The watcher, and everything off -------------------------------------------------------------------------

as_tester mkdir -p "${HOME_DIR}/.cursor"
tester_ame "amethystora-agent sync" >/dev/null
check "Cursor CLI's settings hold the tools' entry while on" jq -e '.mcpServers.amethystora.command == "/usr/libexec/amethystora-agent-tools"' "${HOME_DIR}/.cursor/mcp.json"
found="$(user_bus /usr/libexec/amethystora-security-reader agent-config 2>/dev/null | grep amethystora)"
same "the watcher is quiet about the tools' own entry" "${found}" ""
tester_ame "ame agent toggle off" >/dev/null
[[ ! -e "${CONFIG}/agentic" && ! -e "${CONFIG}/agent-tools" ]] && ok "turned off, no marker is left" || not_ok "the markers after off"
jq -e '.mcpServers.amethystora == null' "${HOME_DIR}/.cursor/mcp.json" >/dev/null && ok "and Cursor's entry is gone" || not_ok "Cursor's entry after off"
pgrep -u "${TEST_USER}" -f 'amethystora-agent-tools' >/dev/null || pgrep -f 'amethystora-agent-root' >/dev/null &&
    not_ok "a tool process outlived the agent" || ok "no tool process is left"
same "with everything off, the tools list nothing" "$(listed "$(mcp list)")" ""

cp -a "${OWN}/agent.conf.before" "${CONF}"
as_tester rm -rf "${HOME_DIR}/.cursor" "${REPORT}" "${REPORT2}"
finish
