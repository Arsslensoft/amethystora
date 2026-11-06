#!/usr/bin/bash
# The plugin system (plans/PLAN-plugins.md, Tests) as the image has it, for 20-tests.sh: what keeps a plugin's code and grants out of
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
for verb in prepare inspect commit commit-shipped grant revoke block unblock remove rollback; do
    grep -qF "<annotate key=\"org.freedesktop.policykit.exec.argv1\">${verb}</annotate>" "${POLICY}" || fail "the policy has no action for ${verb}"
    grep -qF "<action id=\"org.amethystora.plugins.${verb}\">" "${POLICY}" || fail "the action of ${verb} has another id"
done
[[ "$(grep -c '<action ' "${POLICY}")" == 10 ]] || fail "the policy has an action that is not one of the verbs"
if grep -qE '_keep|<allow_active>(yes|no)</allow_active>|<allow_any>(auth|yes)|<allow_inactive>(auth|yes)' "${POLICY}"; then
    fail "the policy keeps an answer, or lets someone through, or asks someone who is not at the machine"
fi
[[ "$(grep -c '<allow_active>auth_admin</allow_active>' "${POLICY}")" == 10 ]] || fail "an action does not ask an administrator"
grep -q 'exec.path">/usr/libexec/amethystora-plugin-root<' "${POLICY}" || fail "the policy names another program"
# What gives more shows what is being given, in polkit's own dialog: the command line, which the helper holds to a short set of characters
for verb in commit commit-shipped grant unblock rollback; do
    sed -n "/<action id=\"org.amethystora.plugins.${verb}\">/,/<\/action>/p" "${POLICY}" | grep -q '\$(command_line)' || fail "the dialog of ${verb} does not show the command line"
done
# The rule lets the person at the machine through for what only takes away or reads, and names nothing that gives more
if grep -qE 'plugins\.(commit|grant|unblock|rollback)"' "${RULES}"; then
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

echo "Plugins: the helper, the host, the policy, the sandbox and the agent's list are as the plan has them"
