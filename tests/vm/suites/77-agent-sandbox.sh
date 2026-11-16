#!/usr/bin/bash
# Several accounts of one agent (plans/PLAN-agent-accounts.md) and an agent held to its project (plans/PLAN-agent-sandbox.md),
# as the test account, with a stand-in agent that tries each thing and prints what happened. One phase:
#   - the sandbox: the project, a cache and its own folder writable; the home, .git's hooks and settings and its own settings
#     not; ~/.ssh empty; no bus, no runtime folder, no other process; sudo refused; TIOCSTI refused; a nested namespace
#     cannot unmount what is hidden; .git cannot be moved away; the session's file while it runs and Stop by its number
#   - with user namespaces turned off the launch is refused and nothing starts
#   - profiles: a folder's profile, two agents at once on two profiles each seeing only its own folder, a way-2 profile's
#     folder bound over the home's, the reader and the watcher's rules looking into a profile, the shells' hook
#   - SANDBOX=always: --no-sandbox refused in a project
# vmlab-paths: apps/agent-tools/* @sf/usr/bin/amethystora-agent @sf/usr/share/amethystora/agent-paths @sf/usr/share/amethystora/credential-files @sf/usr/share/amethystora/agent-hook/* @sf/usr/libexec/amethystora-security-reader @sf/usr/libexec/amethystora-audit-home-rules @sf/usr/share/amethystora/just/agent.just
source "$(dirname "$0")/lib.sh"

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
H="/var/home/${TEST_USER}"
TOOLS=/usr/libexec/amethystora-agent-tools
CONF=/etc/amethystora/agent.conf
OWN=/run/vmlab-agent-sandbox
mkdir -p "${OWN}"
chmod 0755 "${OWN}"
cp -a "${CONF}" "${OWN}/agent.conf.before" 2>/dev/null || true

# A command as the account, on its user manager's bus
as_account() { user_bus env HOME="${H}" "$@"; }

# A script run as the account, on its user manager's bus, its output kept
user_script() {
    local name="$1"
    cat >"${OWN}/${name}.sh"
    chmod 0755 "${OWN}/${name}.sh"
    user_bus env HOME="${H}" bash "${OWN}/${name}.sh" 2>&1
}

# --- The sandbox --------------------------------------------------------------------------------------------------------
out="$(user_script sandbox <<EOF
set +e
mkdir -p ~/vmlab-code/site/src ~/.ssh ~/.cargo/registry
echo KEY >~/.ssh/id_vmlab; chmod 600 ~/.ssh/id_vmlab
cd ~/vmlab-code/site && git init -q 2>/dev/null
${TOOLS} agent-run --agent claude --sandbox open --quiet -- bash -c '
try() { if eval "\$2" >/dev/null 2>&1; then echo "\$1=yes"; else echo "\$1=no"; fi; }
try project "echo x >~/vmlab-code/site/ok"
try cache "echo x >~/.cargo/registry/vmlab"
try own "echo x >~/.claude/vmlab"
try home "echo x >~/vmlab-nope"
try hooks "echo x >~/vmlab-code/site/.git/hooks/pre-commit"
try gitconfig "echo x >>~/vmlab-code/site/.git/config"
try settings "echo x >>~/.claude/settings.json"
try key "test -s ~/.ssh/id_vmlab"
try bus "test -e /run/user/\$(id -u)/bus"
try dbus "test -n \"\${DBUS_SESSION_BUS_ADDRESS:-}\""
try sudo "sudo -n true"
try procs "test \$(ls /proc | grep -c ^[0-9]) -lt 8"
try unmount "unshare -Urm umount -l ~/.ssh"
try movegit "mv ~/vmlab-code/site/.git ~/vmlab-code/site/.git2"
try tiocsti "python3 -c \"import fcntl,termios,pty,os; m,s=pty.openpty(); fcntl.ioctl(s, termios.TIOCSTI, b\\\"x\\\")\""
try readonly "test \"\${AMETHYSTORA_READ_ONLY:-}\" = 1 && test \"\${AMETHYSTORA_AGENT_SANDBOX:-}\" = open"
'
EOF
)"
expect() {
    local key="$1" want="$2" what="$3"
    if grep -qx "${key}=${want}" <<<"${out}"; then ok "${what}"; else not_ok "${what}" "${out}"; fi
}
expect project yes "sandboxed: it writes in its project"
expect cache yes "sandboxed: it writes in a build cache"
expect own yes "sandboxed: it writes in its own folder"
expect home no "sandboxed: it cannot write in the home"
expect hooks no "sandboxed: .git/hooks is read-only"
expect gitconfig no "sandboxed: .git/config is read-only"
expect settings no "sandboxed: its own settings are read-only"
expect key no "sandboxed: ~/.ssh's key is hidden"
expect bus no "sandboxed: the session bus is not there"
expect dbus no "sandboxed: nothing says where the bus is"
expect sudo no "sandboxed: sudo refuses"
expect procs yes "sandboxed: only its own processes"
expect unmount no "sandboxed: a nested namespace cannot unmount what is hidden"
expect movegit no "sandboxed: .git cannot be moved away"
expect tiocsti no "sandboxed: typing into a terminal (TIOCSTI) is refused"
expect readonly yes "sandboxed: Amethystora's tools only read, and the agent is told it is sandboxed"
check "sandboxed: the placeholder folders it made in the project are gone" \
    bash -c "[[ ! -e ${H}/vmlab-code/site/.claude && ! -e ${H}/vmlab-code/site/.cursor ]]"
check "sandboxed: the launch's folder is gone" bash -c "[[ -z \"\$(ls -A ${H}/.cache/amethystora/agent-sandbox 2>/dev/null)\" ]]"

# The session's file while it runs, and Stop by its number ends the sandbox and everything in it
user_script long <<EOF >/dev/null &
cd ~/vmlab-code/site && exec ${TOOLS} agent-run --agent claude --sandbox open --quiet -- sleep 300
EOF
sleep 4
runner="$(cd "/run/user/$(id -u "${TEST_USER}")/amethystora/agent-sessions" 2>/dev/null && for f in [0-9]*; do
    jq -e '.sandboxed == "open" and .agent == "claude"' "${f}" >/dev/null 2>&1 && echo "${f}"; done | head -n 1)"
if [[ -n "${runner}" ]]; then
    ok "a sandbox's runner names its session: agent, profile and sandboxed"
    if as_account "${TOOLS}" profile running | grep -q 'Claude Code · sandboxed'; then
        ok "the running sessions list it"
    else
        not_ok "the running sessions list it" "$(as_account "${TOOLS}" profile running 2>&1)"
    fi
    user_bus env HOME="${H}" amethystora-agent stop "${runner}" >/dev/null 2>&1
    sleep 2
    check "Stop ends the sandbox and everything in it" bash -c "! pgrep -u ${TEST_USER} -f 'sleep 300' >/dev/null"
else
    not_ok "a sandbox's runner names its session: agent, profile and sandboxed" "$(ls -la "/run/user/$(id -u "${TEST_USER}")/amethystora/agent-sessions" 2>&1)"
fi
pkill -u "${TEST_USER}" -f 'sleep 300' 2>/dev/null || true
wait 2>/dev/null || true

# The network held to a list: an allowed host answers, another is refused by the proxy, and nothing goes round it
out="$(user_script network <<EOF
${TOOLS} sandbox use ~/vmlab-code >/dev/null; ${TOOLS} sandbox network ~/vmlab-code list >/dev/null
cd ~/vmlab-code/site && ${TOOLS} agent-run --agent claude --quiet -- bash -c '
echo "allowed=\$(curl -s -o /dev/null -m 25 -w %{http_code} https://api.github.com/zen)"
echo "denied=\$(curl -s -o /dev/null -m 25 -w %{http_code} https://example.com/)"
echo "direct=\$(curl -s --noproxy "*" -o /dev/null -m 8 -w %{http_code} https://api.github.com/zen)"
echo "plain=\$(curl -s -o /dev/null -m 10 -w %{http_code} http://example.com/)"'
${TOOLS} sandbox off ~/vmlab-code >/dev/null
EOF
)"
if curl -s -o /dev/null -m 15 https://api.github.com/zen; then
    expect allowed 200 "held to a list: a host on the list answers"
    expect denied 000 "held to a list: a host off it is refused"
    expect direct 000 "held to a list: nothing goes round the proxy"
    expect plain 403 "held to a list: plain HTTP is refused"
else
    skip "the lab has no network, so the list of hosts was not tried"
fi

# No namespaces: the launch is refused, and the command never runs plain
before="$(sysctl -n user.max_user_namespaces)"
sysctl -q -w user.max_user_namespaces=0
out="$(user_script nonamespace <<EOF
cd ~/vmlab-code/site && ${TOOLS} agent-run --agent claude --sandbox open --quiet -- touch ~/vmlab-ran-plain; echo "exit=\$?"
EOF
)"
sysctl -q -w user.max_user_namespaces="${before}"
check "with no namespaces the sandboxed launch fails" bash -c "grep -q 'exit=[1-9]' <<<'${out//\'/}'"
check "...and the command never ran plain" bash -c "[[ ! -e ${H}/vmlab-ran-plain ]]"

# --- Profiles -----------------------------------------------------------------------------------------------------------
out="$(user_script profiles <<EOF
set +e
ame agent profile add vmlab-work >/dev/null
ame agent profile add vmlab-home >/dev/null
mkdir -p ~/vmlab-work/api ~/vmlab-personal
ame agent profile use vmlab-work ~/vmlab-work >/dev/null
ame agent profile use vmlab-home ~/vmlab-personal >/dev/null
echo "which=\$(ame agent profile which ~/vmlab-work/api)"
# Two stand-in agents at once, each on its own profile, each seeing its own folder: a way-1 agent by its variable
cd ~/vmlab-work/api && ${TOOLS} agent-run --agent claude --quiet -- bash -c 'echo "work=\$CLAUDE_CONFIG_DIR"; sleep 2' &
cd ~/vmlab-personal && ${TOOLS} agent-run --agent claude --quiet -- bash -c 'echo "home=\$CLAUDE_CONFIG_DIR"; sleep 2' &
wait
# ...and a way-2 agent, its profile's folder bound over the home's
mkdir -p ~/.cursor && touch ~/.cursor/vmlab-home-file
cd ~/vmlab-work/api && ${TOOLS} agent-run --agent cursor-agent --quiet -- bash -c 'touch ~/.cursor/vmlab-made; ls ~/.cursor | tr "\n" " "; echo'
echo "homecursor=\$(ls ~/.cursor | tr '\n' ' ')"
# A profile in a sandbox: its own folder writable, the home's own folder of the agent hidden
cd ~/vmlab-work/api && ${TOOLS} agent-run --agent claude --sandbox open --quiet -- bash -c 'test -e ~/.claude/vmlab && echo "sees-home-claude=yes" || echo "sees-home-claude=no"; echo x >"\$CLAUDE_CONFIG_DIR/vmlab" && echo "writes-profile=yes"'
EOF
)"
grep -q "which=vmlab-work (the folder's)" <<<"${out}" && ok "a folder picks its profile" || not_ok "a folder picks its profile" "${out}"
grep -q "work=${H}/.local/share/amethystora/agent-profiles/vmlab-work/.claude" <<<"${out}" &&
    grep -q "home=${H}/.local/share/amethystora/agent-profiles/vmlab-home/.claude" <<<"${out}" &&
    ok "two agents at once, each on its own profile" || not_ok "two agents at once, each on its own profile" "${out}"
check "a way-2 profile's folder is its own" bash -c "[[ -e ${H}/.local/share/amethystora/agent-profiles/vmlab-work/.cursor/vmlab-made && ! -e ${H}/.cursor/vmlab-made ]]"
grep -q "sees-home-claude=no" <<<"${out}" && ok "sandboxed on a profile, the home's own Claude folder is hidden" ||
    not_ok "sandboxed on a profile, the home's own Claude folder is hidden" "${out}"
grep -q "writes-profile=yes" <<<"${out}" && ok "sandboxed on a profile, its own profile is writable" ||
    not_ok "sandboxed on a profile, its own profile is writable" "${out}"
if ! as_account "${TOOLS}" profile in-use vmlab-home >/dev/null; then
    ok "a profile no session runs on is free to remove"
else
    not_ok "a profile no session runs on is free to remove"
fi
# The reader and the watcher's rules look into a profile
mkdir -p "${H}/.local/share/amethystora/agent-profiles/vmlab-work/.claude"
echo '{"permissions": {"defaultMode": "bypassPermissions"}}' >"${H}/.local/share/amethystora/agent-profiles/vmlab-work/.claude/settings.json"
echo '{}' >"${H}/.local/share/amethystora/agent-profiles/vmlab-work/.claude/.credentials.json"
chown -R "${TEST_USER}:" "${H}/.local/share/amethystora/agent-profiles"
check "the security reader reads a profile's settings and names it" bash -c "
    runuser -u ${TEST_USER} -- env HOME=${H} /usr/libexec/amethystora-security-reader agent-config | grep -q 'agent-profiles/vmlab-work/.claude/settings.json turns'"
# Each table login, under every profile, as the watcher's rules glob it, finds the one planted there
found=""
while read -r login; do
    for file in "${H}"/.local/share/amethystora/agent-profiles/*/${login}; do [[ -f ${file} ]] && found+="${file} "; done
done < <(awk '$1 == "login" { print $3 }' /usr/share/amethystora/agent-paths)
if [[ ${found} == *vmlab-work/.claude/.credentials.json* ]] && grep -q 'agent-profiles/\*/' /usr/libexec/amethystora-audit-home-rules; then
    ok "the watcher's credential rules take a profile's login"
else
    not_ok "the watcher's credential rules take a profile's login" "${found}"
fi
# The shells' hook
out="$(user_script hook <<'EOF'
ame agent hook on >/dev/null
cd ~/vmlab-work/api && bash -ic 'echo "bash=$CLAUDE_CONFIG_DIR $(type -t claude)"' 2>/dev/null | tail -n 1
cd ~/vmlab-work/api && zsh -ic 'echo "zsh=$CLAUDE_CONFIG_DIR"' 2>/dev/null | tail -n 1
cd ~/vmlab-work/api && fish -ic 'echo "fish=$CLAUDE_CONFIG_DIR"' 2>/dev/null | tail -n 1
ame agent hook off >/dev/null
cd ~/vmlab-work/api && bash -ic 'echo "off=${CLAUDE_CONFIG_DIR:-none}"' 2>/dev/null | tail -n 1
EOF
)"
for shell in bash zsh fish; do
    grep -q "${shell}=${H}/.local/share/amethystora/agent-profiles/vmlab-work/.claude" <<<"${out}" &&
        ok "the hook in ${shell} follows the folder" || not_ok "the hook in ${shell} follows the folder" "${out}"
done
grep -q "bash=.* function" <<<"${out}" && ok "the hook wraps an agent's command" || not_ok "the hook wraps an agent's command" "${out}"
grep -q "off=none" <<<"${out}" && ok "off, the hook does nothing" || not_ok "off, the hook does nothing" "${out}"

# --- SANDBOX=always -----------------------------------------------------------------------------------------------------
echo "SANDBOX=always" >>"${CONF}"
out="$(user_script always <<EOF
cd ~/vmlab-code/site && ${TOOLS} agent-plan --agent claude --dir . | jq -r .sandbox
cd ~/vmlab-code/site && ${TOOLS} agent-plan --agent claude --sandbox no --dir . >/dev/null 2>&1; echo "no=\$?"
cd ~ && ${TOOLS} agent-plan --agent claude --dir . | jq -r '.sandbox // "plain"'
EOF
)"
cp -a "${OWN}/agent.conf.before" "${CONF}" 2>/dev/null || sed -i '/^SANDBOX=always$/d' "${CONF}"
[[ "$(sed -n 1p <<<"${out}")" == open ]] && ok "SANDBOX=always sandboxes a launch in a project" || not_ok "SANDBOX=always sandboxes a launch in a project" "${out}"
grep -q "no=2" <<<"${out}" && ok "SANDBOX=always refuses --no-sandbox in a project" || not_ok "SANDBOX=always refuses --no-sandbox in a project" "${out}"
[[ "$(sed -n 3p <<<"${out}")" == plain ]] && ok "SANDBOX=always leaves a launch from the home plain" || not_ok "SANDBOX=always leaves a launch from the home plain" "${out}"

# Everything put back
user_script cleanup <<'EOF' >/dev/null
rm -rf ~/vmlab-code ~/vmlab-work ~/vmlab-personal ~/.ssh/id_vmlab ~/.cursor/vmlab-home-file ~/.claude/vmlab
rm -rf ~/.local/share/amethystora/agent-profiles/vmlab-work ~/.local/share/amethystora/agent-profiles/vmlab-home
rm -f ~/.config/amethystora/agent-profiles.json ~/.config/amethystora/agent-sandbox.json ~/.config/amethystora/agent-hook
EOF
finish
