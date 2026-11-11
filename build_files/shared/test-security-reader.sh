#!/usr/bin/bash
# What the security watcher reads as each account (amethystora-security-reader), for 20-tests.sh: each mode
# against a planted home, run as that account as the watcher runs it. What a mode finds has a stable id, a
# level and the entry `ame security allow` takes, and what the account wrote stays words: nothing it finds
# is run. Nothing here writes outside a temporary folder.

set -euo pipefail

READER=/usr/libexec/amethystora-security-reader
T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
chmod 0755 "${T}"
fail() { echo "test-security-reader: $*" >&2; exit 1; }
H="${T}/home"
as_account() {
    setpriv --reuid=4600 --regid=4600 --clear-groups -- /usr/bin/env -i HOME="${H}" USER=bob PATH=/usr/bin:/usr/sbin \
        LANG=C.UTF-8 "${READER}" "$@"
}

# --- dev-hooks --------------------------------------------------------------------------------------------

mkdir -p "${H}/.local/lib/python3.14/site-packages" "${H}/.ssh" "${H}/.config/git" \
    "${H}/.vscode-oss/extensions/evil.ext-1.0.0"
printf 'import os; os.system("touch %s/ran")\n# a comment\n/some/path\n' "${T}" \
    >"${H}/.local/lib/python3.14/site-packages/x.pth"
printf '[include]\n\tpath = %s/.config/git/more\n[core]\n\tfsmonitor = touch %s/ran-git\n' "${H}" "${T}" >"${H}/.gitconfig"
printf '[core]\n\thooksPath = /tmp/hooks\n' >"${H}/.config/git/more"
printf 'Host x\n  ProxyCommand nc %%h %%p\n  User bob\nMatch exec "true"\n' >"${H}/.ssh/config"
printf 'registry=https://example.org\nnode-options=--require /tmp/x.js\n' >"${H}/.npmrc"
printf '{"publisher":"evil","name":"ext\\u001b[2J","version":"1.0.0"}' >"${H}/.vscode-oss/extensions/evil.ext-1.0.0/package.json"
chown -R 4600:4600 "${H}"

OUT="$(as_account dev-hooks)" || fail "dev-hooks failed"
[[ ! -e "${T}/ran" && ! -e "${T}/ran-git" ]] || fail "reading the hooks ran one"
jq -se 'length == 8 and (map(.id) | unique | length) == 8 and all(.[]; .text | test("[\u0000-\u001f]") | not)' <<<"${OUT}" >/dev/null ||
    fail "dev-hooks found: ${OUT}"
for want in 'runs code at every start of Python: import os' 'sets core.fsmonitor to touch' 'sets core.hookspath to /tmp/hooks' \
    'ProxyCommand nc' 'Match exec' 'node-options=--require' 'An editor extension was added: evil.ext??2J 1.0.0'; do
    grep -qF -- "${want}" <<<"${OUT}" || fail "dev-hooks did not find \"${want}\": ${OUT}"
done
jq -se '[.[] | select(.id | startswith("extension:"))] | .[1].level == "quiet" and .[1].within == .[0].id' <<<"${OUT}" >/dev/null ||
    fail "an extension's version is not within the extension"
jq -se 'all(.[]; .allow | startswith("'"${H}"'/"))' <<<"${OUT}" >/dev/null || fail "an entry to allow is not the file"
# The same home, read again, gives the same ids
[[ "$(jq -r .id <<<"${OUT}")" == "$(as_account dev-hooks | jq -r .id)" ]] || fail "the ids are not stable"

# --- agent-config -------------------------------------------------------------------------------------------

mkdir -p "${H}/.claude" "${H}/.codex" "${H}/.gemini/extensions/x" "${H}/.config/opencode/plugin" "${H}/.cursor" \
    "${H}/.copilot/hooks" "${H}/.gemini/antigravity-cli/plugins/pl" "${H}/.gemini/config"
cat >"${H}/.claude/settings.json" <<EOF
{"hooks": {"PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "touch ${T}/ran-hook"}]}]},
 "apiKeyHelper": "sh -c 'cat key'", "permissions": {"defaultMode": "bypassPermissions"}}
EOF
echo '{"mcpServers": {"evil": {"command": "npx", "args": ["-y", "evil-mcp"]}}, "projects": {"/p": {"allowedTools": ["Bash"]}}}' \
    >"${H}/.claude.json"
printf 'notify = ["notify-send", "done"]\napproval_policy = "never"\n[mcp_servers.far]\nurl = "https://example.org/mcp"\n' \
    >"${H}/.codex/config.toml"
# "Always allow" in each agent's own settings: the tools that change things lose their question, the read ones are the image's own
echo '{"permissions": {"allow": ["mcp__amethystora__search", "mcp__amethystora__run", "mcp__amethystora", "Bash(ls)"]}}' \
    >"${H}/.claude/settings.local.json"
printf '[mcp_servers.amethystora]\ncommand = "/usr/libexec/amethystora-agent-tools"\nargs = ["mcp"]\ndefault_tools_approval_mode = "approve"\n[mcp_servers.amethystora.tools.state]\napproval_mode = "approve"\n[mcp_servers.amethystora.tools.software]\napproval_mode = "approve"\n' \
    >>"${H}/.codex/config.toml"
echo '{"permission": {"amethystora_search": "allow", "amethystora_undo": "allow", "amethystora_c*": "allow", "bash": "ask"}}' \
    >"${H}/.config/opencode/opencode.json"
echo '{"locations": {"/home/x": {"tool_approvals": [{"kind": "mcp", "serverName": "amethystora", "toolName": "search"}, {"kind": "mcp", "serverName": "amethystora", "toolName": "plugin"}, {"kind": "mcp", "serverName": "amethystora", "toolName": null}]}}}' \
    >"${H}/.copilot/permissions-config.json"
echo '{"mcpServers": {"g": {"command": "node", "args": ["g.js"], "trust": true}}, "tools": {"discoveryCommand": "find-tools"}}' \
    >"${H}/.gemini/settings.json"
echo '{"name": "ext"}' >"${H}/.gemini/extensions/x/gemini-extension.json"
echo '{"toolPermission": "always-proceed", "statusLine": {"type": "command", "command": "agy-status"}}' \
    >"${H}/.gemini/antigravity-cli/settings.json"
echo '{"guard": {"enabled": true, "PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "from-agy"}]}]}}' \
    >"${H}/.gemini/config/hooks.json"
echo '{"mcpServers": {"remote": {"serverUrl": "https://example.net/mcp"}}}' >"${H}/.gemini/antigravity-cli/plugins/pl/mcp_config.json"
printf '// comment\n{\n  "mcp": {"local": {"type": "local", "command": ["bun", "x.ts"]},},\n  "plugin": ["opencode-evil"],\n  "permission": "allow",\n}\n' \
    >"${H}/.config/opencode/opencode.jsonc"
echo 'export default {}' >"${H}/.config/opencode/plugin/p.ts"
echo '{"approvalMode": "unrestricted", "permissions": {"allow": ["Mcp(amethystora:search)", "Mcp(amethystora:run)"]}}' \
    >"${H}/.cursor/cli-config.json"
# The entry amethystora-agent writes for the agent's tools is the image's own; the same name with anything more
# in it is somebody else's
echo '{"mcpServers": {"amethystora": {"command": "/usr/libexec/amethystora-agent-tools", "args": ["mcp"], "env": {"AMETHYSTORA_AGENT_LAUNCHED": "1"}}}}' \
    >"${H}/.cursor/mcp.json"
echo '{"mcpServers": {"amethystora": {"command": "/usr/libexec/amethystora-agent-tools", "args": ["mcp"], "env": {"X": "1"}}}}' \
    >"${H}/.gemini/config/mcp_config.json"
echo '{"version": 1, "hooks": {"preToolUse": [{"type": "command", "bash": "echo from copilot"}]}}' >"${H}/.copilot/hooks/h.json"
chown -R 4600:4600 "${H}"

OUT="$(as_account agent-config)" || fail "agent-config failed"
[[ ! -e "${T}/ran-hook" ]] || fail "reading a hook ran it"
for want in 'runs a command on PreToolUse: touch' 'runs a command as apiKeyHelper' "turns Claude Code's checks off" \
    'starts the MCP server evil: npx -y evil-mcp' 'runs a command when a turn ends: notify-send done' 'has Codex never ask' \
    'sends to the MCP server far: https://example.org/mcp' 'lets the MCP server g run tools without asking' \
    'runs a command as tools.discoveryCommand' 'is an extension Gemini CLI loads: ext' 'starts the MCP server local: bun x.ts' \
    'loads the plugin: opencode-evil' 'has Opencode never ask' 'is code Opencode loads' 'has Cursor run everything without asking' \
    'runs a command on preToolUse: echo from copilot' 'has Antigravity CLI never ask' 'runs a command as statusLine: agy-status' \
    'runs a command on PreToolUse: from-agy' 'sends to the MCP server remote: https://example.net/mcp' \
    'is a plugin Antigravity CLI loads: pl' \
    'mcp_config.json starts the MCP server amethystora: /usr/libexec/amethystora-agent-tools mcp' \
    'lets the MCP server amethystora run a tool without asking: Mcp(amethystora:run)'; do
    grep -qF -- "${want}" <<<"${OUT}" || fail "agent-config did not find \"${want}\": ${OUT}"
done
for unwanted in '.cursor/mcp.json starts the MCP server amethystora' 'Mcp(amethystora:search)'; do
    if grep -qF -- "${unwanted}" <<<"${OUT}"; then fail "agent-config said the image's own entry: ${unwanted}"; fi
done
# Every agent's allow rule on a tool that changes something is Defense Evasion (the tools then refuse); a read tool's is not
EVASION="$(jq -r 'select(.tactic == "Defense Evasion") | .text' <<<"${OUT}")"
for want in 'settings.local.json lets the MCP server amethystora run a tool without asking: mcp__amethystora__run' \
    'config.toml lets the MCP server amethystora run a tool without asking: default_tools_approval_mode approve' \
    'config.toml lets the MCP server amethystora run a tool without asking: tools.software.approval_mode approve' \
    'opencode.json lets the MCP server amethystora run a tool without asking: permission amethystora_undo allow' \
    'opencode.json lets the MCP server amethystora run a tool without asking: permission amethystora_c* allow' \
    'permissions-config.json lets the MCP server amethystora run a tool without asking: mcp amethystora plugin' \
    'permissions-config.json lets the MCP server amethystora run a tool without asking: mcp amethystora every tool'; do
    grep -qF -- "${want}" <<<"${EVASION}" || fail "agent-config did not report an allow rule as Defense Evasion \"${want}\": ${EVASION}"
done
grep -qE 'settings\.local\.json lets the MCP server amethystora run a tool without asking: mcp__amethystora$' <<<"${EVASION}" ||
    fail "agent-config did not report a rule for the whole server: ${EVASION}"
for unwanted in 'mcp__amethystora__search' 'tools.state.approval_mode' 'amethystora_search' 'mcp amethystora search' 'Bash(ls)'; do
    if grep -qF -- "${unwanted}" <<<"${EVASION}"; then fail "agent-config reported a read tool's allow rule: ${unwanted}"; fi
done
jq -se '(map(.id) | unique | length) == length
    and any(.[]; (.text | test("lets Claude Code use tools without asking in /p")) and .level == "quiet")
    and all(.[]; .allow | startswith("'"${H}"'/"))' <<<"${OUT}" >/dev/null || fail "agent-config's ids, levels or entries: ${OUT}"
[[ "$(jq -r .id <<<"${OUT}")" == "$(as_account agent-config | jq -r .id)" ]] || fail "agent-config's ids are not stable"

# --- app-permissions ----------------------------------------------------------------------------------------

# No Flatpak app in a build: nothing, and no error
[[ -z "$(as_account app-permissions)" ]] || fail "app-permissions found something with no app installed"

echo "test-security-reader: what each account's reader finds is only ever read"
