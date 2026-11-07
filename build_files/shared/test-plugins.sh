#!/usr/bin/bash
# The plugin system (plans/done/PLAN-plugins.md, Tests) as the image has it, for 20-tests.sh: what keeps a plugin's code and grants out of
# the account's hands is in files the image ships, and is checked here since a build never starts a plugin. The root helper and the
# host are installed, the polkit policy gives every verb an action, asks an administrator for each that gives more and keeps no
# answer, the rule lets nobody through for those, `grant` has no --yes, the host's unit has no setting that needs a namespace and
# nobody has enabled it, the shell reads no plugin path, and `never.rs` refuses the agent every way in. What runs is the VM lab's
# (tests/vm/suites/72-plugins.sh); the host's own tests are cargo's, in the apps stage.
#   test-plugins.sh [CTX]
set -euo pipefail

CTX="${1:-/ctx}"
POLICY=/usr/share/polkit-1/actions/org.amethystora.plugins.policy
RULES=/usr/share/polkit-1/rules.d/50-amethystora-plugins.rules
UNIT=/usr/lib/systemd/user/amethystora-plugin-host.service
COMMAND=/usr/libexec/amethystora-plugins
JUST=/usr/share/amethystora/just/plugin.just
SRC="${CTX}/apps"

fail() {
    echo "Plugins: $*"
    exit 1
}

# --- What is installed -----------------------------------------------------------------------------------------------------
for program in amethystora-plugin-host amethystora-plugin-root amethystora-plugins; do
    test -x "/usr/libexec/${program}" || fail "/usr/libexec/${program} is not installed"
done
test -s /usr/share/amethystora/plugins/seccomp.bpf || fail "the sandbox's seccomp filter is not installed"
# A filter is a program of 8-byte instructions
(($(stat -c %s /usr/share/amethystora/plugins/seccomp.bpf) % 8 == 0)) || fail "seccomp.bpf is not a whole number of instructions"
# The helper that writes the store does nothing with the bus: it links no GLib
if ldd /usr/libexec/amethystora-plugin-root | grep -q 'libglib\|libgio'; then
    fail "amethystora-plugin-root links GLib, which only the host may"
fi
test -s "${JUST}" || fail "${JUST} is missing"
grep -q '^mod plugin$' /usr/share/amethystora/just/00-entry.just || fail "00-entry.just does not declare the plugin group"
for file in /usr/lib/tmpfiles.d/amethystora-plugins.conf /etc/amethystora/plugins.conf; do
    test -s "${file}" || fail "${file} is missing"
done
grep -qE '^d /var/lib/amethystora/plugins 0755 root root' /usr/lib/tmpfiles.d/amethystora-plugins.conf || fail "the store is not root's"
grep -qE '^D /var/lib/amethystora/plugins/quarantine 0700 root root' /usr/lib/tmpfiles.d/amethystora-plugins.conf || fail "the quarantine is not root's alone, or is not cleared at boot"
grep -qx 'ALLOW=granted' /etc/amethystora/plugins.conf || fail "plugins.conf does not ship ALLOW=granted"
grep -qx 'BLOCK_NETWORK=no' /etc/amethystora/plugins.conf || fail "plugins.conf does not ship BLOCK_NETWORK=no"

# --- The polkit policy: one action for each verb, each asking an administrator, none keeping an answer ----------------------------
for verb in prepare inspect commit commit-updates commit-dev commit-shipped grant revoke block unblock remove rollback; do
    grep -qF "<annotate key=\"org.freedesktop.policykit.exec.argv1\">${verb}</annotate>" "${POLICY}" || fail "the policy has no action for ${verb}"
    grep -qF "<action id=\"org.amethystora.plugins.${verb}\">" "${POLICY}" || fail "the action of ${verb} has another id"
done
[[ "$(grep -c '<action ' "${POLICY}")" == 12 ]] || fail "the policy has an action that is not one of the verbs"
# One kept answer, the plan's only one: commit-dev, which the helper holds to one plugin and no more than it has
[[ "$(grep -c '_keep' "${POLICY}")" == 1 ]] && sed -n '/<action id="org.amethystora.plugins.commit-dev">/,/<\/action>/p' "${POLICY}" | grep -q 'auth_admin_keep' ||
    fail "an answer is kept for an action other than commit-dev"
grep -q 'Some("commit-dev") if rest.len() == 1 => self.commit_updates' "${SRC}/plugin-host/src/rootcmd.rs" || fail "commit-dev is not the checks of an update, for one plugin"
if sed '/<action id="org.amethystora.plugins.commit-dev">/,/<\/action>/d' "${POLICY}" | grep -qE '_keep|<allow_active>(yes|no)</allow_active>|<allow_any>(auth|yes)|<allow_inactive>(auth|yes)'; then
    fail "the policy keeps an answer, or lets someone through, or asks someone who is not at the machine"
fi
[[ "$(grep -c '<allow_active>auth_admin</allow_active>' "${POLICY}")" == 11 ]] || fail "an action does not ask an administrator"
grep -q 'exec.path">/usr/libexec/amethystora-plugin-root<' "${POLICY}" || fail "the policy names another program"
# What gives more shows what is being given, in polkit's own dialog: the command line, which the helper holds to a short set of characters
for verb in commit commit-updates commit-dev commit-shipped grant unblock rollback; do
    sed -n "/<action id=\"org.amethystora.plugins.${verb}\">/,/<\/action>/p" "${POLICY}" | grep -q '\$(command_line)' || fail "the dialog of ${verb} does not show the command line"
done
# The rule lets the person at the machine through for what only takes away or reads, and names nothing that gives more
if grep -qE 'plugins\.(commit|commit-updates|commit-dev|grant|unblock|rollback)"' "${RULES}"; then
    fail "the rule lets someone through for what gives a plugin more"
fi
for verb in prepare inspect revoke block remove commit-shipped; do
    grep -qF "\"org.amethystora.plugins.${verb}\"" "${RULES}" || fail "the rule leaves out ${verb}"
done
grep -q 'subject.local && subject.active' "${RULES}" || fail "the rule does not ask for the active local session"

# --- Approval is a person's: grant and add have no --yes, and nothing answers for them -------------------------------------------
if grep -nE -e '--yes|--assume-yes|--force' "${COMMAND}" "${JUST}"; then
    fail "a plugin command takes --yes, or an answer given in advance"
fi
grep -q 'need_terminal' "${COMMAND}" || fail "${COMMAND} does not insist on a terminal"
# Every change of the store goes through pkexec and the helper, never a write of the command's own
if grep -nE '(>|tee|cp|mv|rm|install|ln|chmod|chown) .*/var/lib/amethystora/plugins' "${COMMAND}"; then
    fail "${COMMAND} writes the store itself"
fi

# plugin-approval off (amethystora-hardening) is the one rule that lets anything through, and only a commit, only for wheel at the
# machine: it is shipped in the hardening folder, never in rules.d, the switch is on as the image ships, and no image can turn it off
APPROVAL=/usr/share/amethystora/hardening/49-amethystora-plugin-approval.rules
test -s "${APPROVAL}" || fail "${APPROVAL} is missing"
[[ "$(grep -o 'org\.amethystora\.plugins\.[a-z-]*' "${APPROVAL}" | sort -u)" == org.amethystora.plugins.commit ]] ||
    fail "the plugin-approval rule names an action other than commit"
grep -q 'subject.local && subject.active' "${APPROVAL}" && grep -q 'isInGroup("wheel")' "${APPROVAL}" ||
    fail "the plugin-approval rule lets through more than an administrator at the machine"
test ! -e /usr/share/polkit-1/rules.d/49-amethystora-plugin-approval.rules -a ! -e /etc/polkit-1/rules.d/49-amethystora-plugin-approval.rules ||
    fail "the image ships plugin-approval off"
/usr/libexec/amethystora-hardening is-on plugin-approval || fail "plugin-approval is off as the image ships"
grep -q 'APPROVAL_OFF_RULE: &str = "/etc/polkit-1/rules.d/49-amethystora-plugin-approval.rules"' "${SRC}/plugin-host/src/rootcmd.rs" ||
    fail "the root helper does not look for the rule plugin-approval off writes"
if grep -A3 '^BUILD_SAFE_OFF=' /usr/libexec/amethystora-image-policy | grep -q plugin-approval; then
    fail "an image may turn plugin-approval off"
fi

# --- The host's unit: nothing that needs a namespace, and nobody has it enabled ----------------------------------------------------
if grep -qE '^(ProtectSystem|ProtectHome|ReadWritePaths|ReadOnlyPaths|PrivateTmp|PrivateDevices|PrivateNetwork|PrivateUsers|ProtectKernel[A-Za-z]*|ProtectControlGroups|ProtectClock|ProtectHostname|ProtectProc|CapabilityBoundingSet|RestrictNamespaces|MemoryDenyWriteExecute|SystemCallFilter)=' "${UNIT}"; then
    fail "the host's unit has a setting that needs a namespace, and the host could no longer tell the shell from a stranger"
fi
grep -q '^BusName=org.amethystora.Plugins$' "${UNIT}" || fail "the unit does not own the host's bus name"
if compgen -G '/usr/lib/systemd/user/*.wants/amethystora-plugin-host.service' >/dev/null || compgen -G '/etc/systemd/user/*.wants/amethystora-plugin-host.service' >/dev/null; then
    fail "the host's unit is enabled for everyone"
fi
grep -q 'org.amethystora.Plugins' "${SRC}/plugin-host/dbus/org.amethystora.Plugins.xml" || fail "the bus contract is missing"

# --- The sandbox is one function, and it is the plan's ----------------------------------------------------------------------------
for flag in --unshare-all --unshare-user --disable-userns --new-session --die-with-parent '"ALL"' --clearenv --seccomp --slice=amethystora-plugins.slice MemoryMax=256M MemorySwapMax=0 OOMPolicy=continue CPUQuota=25% TasksMax=64; do
    grep -qF -- "${flag}" "${SRC}/plugin-host/src/sandbox.rs" || fail "the sandbox recipe lacks ${flag}"
done
# Only one place builds the command line a plugin runs under
if grep -rn --include=*.rs '"--unshare-all"' "${SRC}" | grep -v 'plugin-host/src/sandbox.rs' | grep -q .; then
    fail "a second place builds a sandbox command line"
fi

# --- Shipped plugins reach nothing outside the machine's own state ------------------------------------------------------------------
for manifest in /usr/share/amethystora/plugins/*/manifest.json; do
    [[ -e "${manifest}" ]] || continue
    if grep -qE '"(network|network-local|files|window|change)"' "${manifest}"; then
        fail "${manifest}: a plugin of the image asks for network, files, window or change"
    fi
done

# --- The shell reads no plugin path, and the agent is refused every way in ---------------------------------------------------------------
if grep -rln 'var/lib/amethystora/plugins' /usr/share/gnome-shell/extensions 2>/dev/null | grep -q .; then
    fail "a shell module reads the store, which only the host does"
fi
for pattern in '"amethystora-plugin-root"' '"org.amethystora.plugins"' '"plugin", "grant" | "unblock" | "add" | "update" | "rollback" | "dev"'; do
    grep -qF -- "${pattern}" "${SRC}/agent-tools/src/never.rs" || fail "never.rs lacks ${pattern}"
done

# --- What the shell draws (R7-01, 03, 07, 09): nothing a plugin says can pass for the system -----------------------------------------
UI=/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io
MODULES="${UI}/modules"
# `extra` comes after everything the system shows and before quiet and a focus profile's mark; the plugins module right after the pill
ORDER_LINE="$(grep -E "^export const ORDER" -A1 "${MODULES}/pill/activities.js" | tr -d '\n')"
[[ "${ORDER_LINE}" == *"'saving', 'extra', 'quiet', 'focus'];"* ]] || fail "ORDER does not put extra between saving and quiet"
for system in call recording flat present meeting link code battery agent; do
    before="${ORDER_LINE%%\'extra\'*}"
    [[ "${before}" == *"'${system}'"* ]] || fail "${system} does not come before extra"
done
grep -qE "^    \['pill', Pill\],$" "${UI}/extension.js" || fail "the pill module is not a line of its own"
grep -A1 -E "^    \['pill', Pill\],$" "${UI}/extension.js" | grep -qE "^    \['plugins', Plugins\],$" || fail "the plugins module does not start right after the pill"
# A plugin's activity has no plate, and nothing the plugins draw names a system look or builds one from what a plugin printed
grep -qE "this\.plate = '';" "${MODULES}/plugins/pill.js" || fail "a plugin's pill activity has a plate"
if grep -rnE "plate: *[^'\" ]|\.plate *= *[^' ]|plateTint|'call'|'recording'|'agent-ask'|'battery'" "${MODULES}/plugins" "${MODULES}/plugins.js"; then
    fail "the plugins module names a system plate or look"
fi
# Text only: never markup, never a path as an icon, never a command typed by a plugin
if grep -rnE "use_markup = true|set_use_markup\(true\)|markup" "${MODULES}/plugins" "${MODULES}/plugins.js" "${MODULES}/search/plugins.js" | grep -v '^[^:]*:[0-9]*: *//' | grep -v 'never markup\|no markup\|Markup\|use_markup = false'; then
    fail "the plugins' drawing reads markup"
fi
if grep -rnE "FileIcon|new_for_path\(.*(output|plugin\.)|Gio\.File\.new_for_path\(path" "${MODULES}/plugins/bar.js" "${MODULES}/plugins/tile.js" "${MODULES}/plugins/card.js" "${MODULES}/plugins/pill.js"; then
    fail "a plugin's icon is drawn from a path"
fi
# Nothing blocks the compositor in what the plugins add, and nothing is spawned but a fixed program
if grep -rnE "[a-z]_sync\(|file_get_contents|load_contents\(|spawn_command_line|spawn_sync|Subprocess" "${MODULES}/plugins" "${MODULES}/plugins.js" "${MODULES}/search/plugins.js"; then
    fail "the plugins module blocks the shell, or spawns through a shell"
fi
if grep -nE "spawn\(\[[^]]*\\$\{" "${MODULES}/plugins.js"; then
    fail "the plugins module spawns a program it built from a plugin's words"
fi
# The shell reads neither a draft nor the shipped folder: the host tells it everything
if grep -rnE "plugin-drafts|amethystora/plugins|\.local/state/amethystora/plugins" "${MODULES}/plugins" "${MODULES}/plugins.js" "${MODULES}/deskwidgets.js" "${MODULES}/search/plugins.js"; then
    fail "a shell module reads a plugin's folder"
fi
# The host's owner is checked by the file it runs, and only its signals are believed
grep -q "HOST_EXE = '/usr/libexec/amethystora-plugin-host'" "${MODULES}/plugins/host.js" || fail "the shell does not check who owns the host's name"
grep -q "sender !== this._owner" "${MODULES}/plugins/host.js" || fail "the shell believes a signal from anyone"
# Every key off costs nothing: nothing but the settings is connected, and the dispose of the settings is there
grep -q "run_dispose" "${MODULES}/plugins.js" || fail "the plugins module does not dispose its settings"
# The script pills: the shell reads only plain files of a few bytes, never a path it was given
grep -q "FileType.REGULAR" "${MODULES}/plugins/script.js" || fail "a script's pill file is read without being a plain file"
grep -q "NOFOLLOW_SYMLINKS" "${MODULES}/plugins/script.js" || fail "a script's pill file may be a link"
# Secrets: never an argument, never the environment
if grep -nE 'setting-set .*"\$\{?(value|3|4)' "${COMMAND}"; then
    fail "a setting's value is given as an argument"
fi
grep -q "SETTINGS_FD" "${SRC}/plugin-host/src/sandbox.rs" || fail "settings have no pipe"
if grep -nE 'setenv.*(token|secret|password)' "${SRC}/plugin-host/src/sandbox.rs" "${SRC}/plugin-host/src/host.rs"; then
    fail "a secret reaches the environment"
fi
# The image ships plugins and examples, and no widget extension
[[ ! -e /usr/share/gnome-shell/extensions/amethystora-widgets@arsslensoft.github.io ]] || fail "the widgets extension is still in the image"
test -s /usr/libexec/amethystora-pill || fail "ame pill's helper is missing"
test -x /usr/libexec/amethystora-pill || fail "ame pill's helper is not executable"
grep -q '^mod pill$' /usr/share/amethystora/just/00-entry.just || fail "00-entry.just does not declare the pill group"
grep -q '^name: amethystora-plugins$' /usr/share/amethystora/agents/skills/amethystora-plugins/SKILL.md || fail "the plugins skill is missing"
# The agent reads and writes drafts only through the commands: keep and open are the person's
for pattern in '"keep" | "open"'; do
    grep -qF -- "${pattern}" "${SRC}/agent-tools/src/never.rs" || fail "never.rs lacks ${pattern}"
done
node_test="${CTX}/build_files/shared/test-plugins-clamp.mjs"
if [[ -x /apps-stage/usr/bin/node ]]; then
    env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node "${node_test}" "${CTX}" || fail "the shell's clamp of what a plugin printed fails its test"
fi

echo "Plugins: the helper, the host, the policy, the sandbox and the agent's list are as the plan has them"
