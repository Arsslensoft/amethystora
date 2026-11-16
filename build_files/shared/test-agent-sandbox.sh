#!/usr/bin/bash
# Profiles and the sandbox of the AI agents (plans/PLAN-agent-accounts.md, plans/PLAN-agent-sandbox.md), against a stand-in
# home, from 20-tests.sh. Nothing here runs bubblewrap, which the image's build may not give namespaces: the recipe is held to
# what it says (`sandbox show --json`, the same function `agent-run` runs), and tests/vm/suites/77-agent-sandbox.sh runs it.
set -euo pipefail

TOOLS=/usr/libexec/amethystora-agent-tools
TABLE=/usr/share/amethystora/agent-paths
CREDENTIALS=/usr/share/amethystora/credential-files
fail() { echo "test-agent-sandbox: $*" >&2; exit 1; }

# The table names every agent the launcher offers, each with its way and its folder; every login is a credential the watcher
# watches and the sandbox hides; the watcher's rules read both files
mapfile -t agents < <(sed -n 's/^AGENTS=(\(.*\))$/\1/p' /usr/bin/amethystora-agent | tr ' ' '\n')
[[ ${#agents[@]} -eq 6 ]] || fail "the launcher's AGENTS"
for agent in "${agents[@]}"; do
    awk -v a="${agent}" '$1 == "way" && $2 == a { found = 1 } END { exit !found }' "${TABLE}" || fail "${agent} has no way line"
    awk -v a="${agent}" '$1 == "dir" && $2 == a { found = 1 } END { exit !found }' "${TABLE}" || fail "${agent} has no dir line"
done
while read -r login; do
    grep -qxF -- "${login}" "${CREDENTIALS}" || fail "${login} is in the table but not in credential-files"
done < <(awk '$1 == "login" { print $3 }' "${TABLE}")
grep -q '/usr/share/amethystora/credential-files' /usr/libexec/amethystora-audit-home-rules || fail "the watcher's rules do not read credential-files"
grep -q 'agent-profiles/\*/' /usr/libexec/amethystora-audit-home-rules || fail "the watcher's rules do not read each profile's logins"
test -s /usr/share/amethystora/agent-sandbox.bpf || fail "the sandbox's seccomp filter is missing"
test -x /usr/bin/bwrap || fail "bubblewrap is missing"

# A stand-in home, outside /tmp (which a sandbox has its own of, and never takes a project from)
H="$(mktemp -d -p /var/lib agent-sandbox-test.XXXXXX)"
trap 'rm -rf "${H}"' EXIT
export HOME="${H}" XDG_CONFIG_HOME="${H}/.config" XDG_STATE_HOME="${H}/.local/state" XDG_CACHE_HOME="${H}/.cache" XDG_RUNTIME_DIR="${H}/run"
unset AMETHYSTORA_AGENT_PROFILE AMETHYSTORA_AGENT_PROFILE_PIN CLAUDE_CONFIG_DIR CODEX_HOME
mkdir -p "${H}/run" "${H}/work/api" "${H}/workshop" "${H}/code/site/.git/hooks" "${H}/code/site/src" "${H}/.ssh" "${H}/.cursor" "${H}/.config/x"
echo key >"${H}/.ssh/id_ed25519"
echo "[core]" >"${H}/code/site/.git/config"

# Profiles: made, bound to a folder, picked by the longest real path, never by a name that only begins the same
"${TOOLS}" profile add work >/dev/null
! "${TOOLS}" profile add default 2>/dev/null || fail "default became a profile"
! "${TOOLS}" profile add 'Bad/Name' 2>/dev/null || fail "a bad name became a profile"
"${TOOLS}" profile use work "${H}/work" >/dev/null
[[ "$("${TOOLS}" profile which "${H}/work/api")" == "work (the folder's)" ]] || fail "the folder's profile"
[[ "$("${TOOLS}" profile which "${H}/workshop")" == "default (the home's)" ]] || fail "a folder that only begins the same"
ln -s "${H}/work" "${H}/link"
[[ "$("${TOOLS}" profile which "${H}/link/api")" == "work (the folder's)" ]] || fail "a link into the folder"
[[ "$(AMETHYSTORA_AGENT_PROFILE_PIN=default "${TOOLS}" profile which "${H}/work/api")" == "default (the terminal's)" ]] || fail "the pin"
! AMETHYSTORA_AGENT_PROFILE_PIN=nope "${TOOLS}" profile which "${H}/work/api" 2>/dev/null || fail "a pin that is not a profile"
plan="$("${TOOLS}" agent-plan --agent claude --dir "${H}/work/api")"
jq -e --arg dir "${H}/.local/share/amethystora/agent-profiles/work/.claude" \
    '.profile == "work" and .env.CLAUDE_CONFIG_DIR == $dir and .login == ".local/share/amethystora/agent-profiles/work/.claude/.credentials.json" and .wrapped == false' \
    <<<"${plan}" >/dev/null || fail "the plan of a way-1 profile: ${plan}"
jq -e '.wrapped == true' <<<"$("${TOOLS}" agent-plan --agent cursor-agent --dir "${H}/work/api")" >/dev/null || fail "a way-2 profile is not wrapped"
! "${TOOLS}" agent-plan --agent copilot --dir "${H}/work/api" 2>/dev/null || fail "Copilot CLI got a profile"
# The hook's lines for a folder, quoted
"${TOOLS}" profile env --shell bash --dir "${H}/work/api" | grep -qx "export CLAUDE_CONFIG_DIR='${H}/.local/share/amethystora/agent-profiles/work/.claude'" ||
    fail "the hook's variable"
"${TOOLS}" profile env --shell fish --dir "${H}/work/api" | grep -qx "set -gx AMETHYSTORA_AGENT_PROMPT 'work'" || fail "the hook's prompt, for fish"
# ...and the shells' functions only while the hook is on
bash --norc --noprofile -ic "source /usr/share/amethystora/agent-hook/hook.sh; ! type -t claude >/dev/null" 2>/dev/null || fail "the hook ran while off"
touch "${H}/.config/amethystora/agent-hook"
[[ "$(cd "${H}/work/api" && bash --norc --noprofile -ic 'source /usr/share/amethystora/agent-hook/hook.sh; echo "$(type -t claude) ${CLAUDE_CONFIG_DIR}"' 2>/dev/null)" == "function ${H}/.local/share/amethystora/agent-profiles/work/.claude" ]] ||
    fail "the hook in bash"
# A running session on a profile keeps it from being removed; the session's own file says its agent and profile
mkdir -p "${H}/run/amethystora/agent-sessions"
echo "{\"pid\": $$, \"agent\": \"claude\", \"profile\": \"work\", \"started\": 1}" >"${H}/run/amethystora/agent-sessions/$$"
"${TOOLS}" profile in-use work >/dev/null || fail "a running session is not seen"
[[ "$("${TOOLS}" profile running)" == "$$: Claude Code · work, since"* ]] || fail "the running sessions"
rm "${H}/run/amethystora/agent-sessions/$$"
! "${TOOLS}" profile in-use work >/dev/null || fail "an ended session is still seen"
# The security reader reads a profile's settings as the home's, and names the profile
mkdir -p "${H}/.local/share/amethystora/agent-profiles/work/.claude"
echo '{"permissions": {"defaultMode": "bypassPermissions"}}' >"${H}/.local/share/amethystora/agent-profiles/work/.claude/settings.json"
/usr/libexec/amethystora-security-reader agent-config | jq -e 'select(.tactic == "Defense Evasion") | select(.text | startswith("~/.local/share/amethystora/agent-profiles/work/.claude/settings.json"))' \
    >/dev/null || fail "the reader did not look into the profile"

# The sandbox: the project is the repository, never the home, a dot-folder of it or a hidden place; its keys hidden, the other
# agents' folders hidden, the repository's hooks and settings read-only after the project is bound, the filter given
"${TOOLS}" sandbox use "${H}/code" >/dev/null
! "${TOOLS}" sandbox use "${H}" 2>/dev/null || fail "the home got a rule"
shown="$("${TOOLS}" sandbox show --agent claude --dir "${H}/code/site/src" --json)"
jq -e --arg h "${H}" '.sandbox == "open" and .project == "\($h)/code/site" and (.hidden | index("\($h)/.ssh")) and (.hidden | index("\($h)/.cursor"))
    and (.writable | index("\($h)/code/site")) and (.writable | index("\($h)/.claude"))' <<<"${shown}" >/dev/null || fail "the recipe's places: ${shown}"
command="$(jq -r .command <<<"${shown}")"
[[ ${command} == *"--bind ${H}/code/site ${H}/code/site "*"--ro-bind ${H}/code/site/.git/hooks ${H}/code/site/.git/hooks "* ]] || fail "hooks read-only after the project"
[[ ${command} == *"--ro-bind ${H}/code/site/.git/config ${H}/code/site/.git/config "* ]] || fail "the repository's settings"
[[ ${command} == *"--tmpfs ${H}/.ssh "*"--remount-ro ${H}/.ssh "* ]] || fail ".ssh hidden and read-only"
[[ ${command} == *" --seccomp 9 "* && ${command} == *" --unshare-pid "* && ${command} != *"--share-net"* ]] || fail "the filter and the namespaces"
[[ ${command} == *"--tmpfs /run "* && ${command} != *"/run/user/"*"/bus"* ]] || fail "the bus"
# A closed sandbox replaces the home
[[ "$("${TOOLS}" sandbox show --agent claude --dir "${H}/code/site" --sandbox closed --json | jq -r .command)" == *"--tmpfs ${H} "* ]] || fail "a closed sandbox"
# Never a project that is the home, a dot-folder of it, or above it
! "${TOOLS}" agent-plan --agent claude --sandbox open --dir "${H}" 2>/dev/null || fail "the home as a project"
! "${TOOLS}" agent-plan --agent claude --sandbox open --dir "${H}/.config/x" 2>/dev/null || fail "a dot-folder as a project"
! "${TOOLS}" agent-plan --agent claude --sandbox open --dir / 2>/dev/null || fail "/ as a project"
# Permissions say what they give away, and an unknown one is refused
"${TOOLS}" sandbox cost ssh-agent | grep -q "every key" || fail "ssh-agent's cost"
! "${TOOLS}" sandbox allow "${H}/code" network=any 2>/dev/null || fail "an unknown permission"
"${TOOLS}" sandbox allow "${H}/code" gpu >/dev/null
[[ "$("${TOOLS}" sandbox show --agent claude --dir "${H}/code/site" --json | jq -r .command)" == *"--dev-bind-try /dev/dri /dev/dri "* ]] || fail "gpu"
"${TOOLS}" sandbox off "${H}/code" >/dev/null
jq -e '.wrapped == false' <<<"$("${TOOLS}" agent-plan --agent claude --dir "${H}/code/site")" >/dev/null || fail "a rule taken off"
echo "test-agent-sandbox: ok"
