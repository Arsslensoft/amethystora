#!/usr/bin/bash
# The Vault (plans/PLAN-vault.md, Tests) as the image has it, for 20-tests.sh: what keeps its keys out of reach is in the
# files the image ships and is checked here, since a build never starts the service. The unit's sandbox is all there, its
# polkit policy lets nobody through and keeps no answer, the sockets are enabled and listen where the clients look, every
# recipe the manual, the window and the agent's knowledge name exists, `never.rs` refuses the agent every spelling, every
# sentence the service says to polkit is a line of vault.just (so that it is translated), no `rsa` crate is built into
# the apps, and the data it ships is what its licences say it is. What runs is the VM lab's (tests/vm/suites/74-vault.sh).
#   test-vault.sh [CTX]
set -euo pipefail

CTX="${1:-/ctx}"
UNIT=/usr/lib/systemd/system
SERVICE="${UNIT}/amethystora-vault.service"
JUST=/usr/share/amethystora/just/vault.just
SRC="${CTX}/apps"

fail() {
    echo "The Vault: $*"
    exit 1
}
has() { grep -qxF -- "$2" "$1" || fail "$1 lacks the line \"$2\""; }

# --- What is installed -----------------------------------------------------------------------------------------------------
for program in service cli work net; do
    test -x "/usr/libexec/amethystora-vault-${program}" || fail "/usr/libexec/amethystora-vault-${program} is not installed"
done
for program in git-credential-amethystora docker-credential-amethystora amethystora-vault; do
    test -x "/usr/bin/${program}" || fail "/usr/bin/${program} is not installed"
done
test -s /usr/lib64/amethystora-vault/libamethystora_vault_pkcs11.so || fail "the PKCS#11 module is not installed"
test -s /usr/lib/amethystora-vault/amethystora-vault || fail "the window's program is not installed"
test -s /usr/lib/amethystora-vault/resources/app/locale/en.json || fail "the window's messages are not listed"
# The service never loads libcrypto: the keys' process holds no parser, and its memory is eight megabytes (the network service
# does, for TLS, and holds no key)
if ldd /usr/libexec/amethystora-vault-service | grep -q 'libcrypto\|libssl'; then
    fail "amethystora-vault-service links libcrypto, which only the short-lived worker may"
fi
grep -q '^u amethystora-vault ' /usr/lib/sysusers.d/amethystora-vault.conf || fail "no sysusers line for the service's user"
grep -q '^u amethystora-vault-net ' /usr/lib/sysusers.d/amethystora-vault-net.conf || fail "no sysusers line for the network service's user"
# A TPM slot is bound to the service's uid: a build that gave it another number would leave every such slot unable to open
[[ "$(id -u amethystora-vault 2>/dev/null)" == 956 && "$(id -u amethystora-vault-net 2>/dev/null)" == 957 ]] || fail "the Vault's users are not 956 and 957"

# --- The unit: its sandbox is all there ------------------------------------------------------------------------------------
for line in User=amethystora-vault StateDirectory=amethystora/vault StateDirectoryMode=0700 PrivateNetwork=yes \
    RestrictAddressFamilies=AF_UNIX NoNewPrivileges=yes PrivateDevices=yes ProtectSystem=strict ProtectHome=yes PrivateTmp=yes \
    ProtectKernelTunables=yes ProtectKernelModules=yes ProtectKernelLogs=yes ProtectControlGroups=yes ProtectClock=yes \
    RestrictNamespaces=yes RestrictRealtime=yes RestrictSUIDSGID=yes LockPersonality=yes CapabilityBoundingSet= \
    AmbientCapabilities= MemoryDenyWriteExecute=yes SystemCallArchitectures=native SystemCallFilter=@system-service \
    UMask=0077 LimitCORE=0 LogLevelMax=warning; do
    has "${SERVICE}" "${line}"
done
grep -q '^ExecStart=/usr/libexec/amethystora-vault-service$' "${SERVICE}" || fail "the unit starts something else"
grep -q '^Sockets=amethystora-vault.socket amethystora-vault-ssh.socket$' "${SERVICE}" || fail "the service is not handed both sockets"
# Nothing weakens it: no network, no root, no wider address families, no capabilities
if grep -qE '^(PrivateNetwork=no|User=root|AmbientCapabilities=.+|CapabilityBoundingSet=.+|RestrictAddressFamilies=.*AF_(INET|NETLINK|PACKET))' "${SERVICE}"; then
    fail "the unit loosens its own sandbox"
fi
# The network service holds no key and no state, and sees the network
NET="${UNIT}/amethystora-vault-net.service"
has "${NET}" User=amethystora-vault-net
grep -qE '^(StateDirectory|ReadWritePaths)=' "${NET}" && fail "the network service keeps state"
grep -qE '^(PrivateNetwork=yes)' "${NET}" && fail "the network service has no network"

# --- The sockets: enabled, and where the clients look ----------------------------------------------------------------------
has "${UNIT}/amethystora-vault.socket" ListenStream=/run/amethystora/vault.sock
has "${UNIT}/amethystora-vault-ssh.socket" ListenStream=/run/amethystora/vault-ssh.sock
has "${UNIT}/amethystora-vault-ssh.socket" Service=amethystora-vault.service
for socket in amethystora-vault.socket amethystora-vault-ssh.socket amethystora-vault-net.socket; do
    [[ "$(systemctl is-enabled "${socket}" 2>/dev/null)" == enabled ]] || fail "${socket} is not enabled"
done
# Every account may connect to the Vault's: the kernel says which one is asking. Only the service may connect to the network one's
has "${UNIT}/amethystora-vault.socket" SocketMode=0666
has "${UNIT}/amethystora-vault-ssh.socket" SocketMode=0666
has "${UNIT}/amethystora-vault-net.socket" SocketMode=0600
has "${UNIT}/amethystora-vault-net.socket" SocketUser=amethystora-vault
# Nothing but the sockets is enabled: the service starts when asked, and a machine nobody set up costs nothing
[[ "$(systemctl is-enabled amethystora-vault.service 2>/dev/null || true)" != enabled ]] || fail "the service is enabled by itself"

# --- The policy: nobody is let through, and no answer is kept -------------------------------------------------------------
POLICY=/usr/share/polkit-1/actions/org.amethystora.vault.policy
python3 - "${POLICY}" "${SRC}/vault-service/src" <<'PY' || fail "the polkit policy is not what the Vault's rules say"
import re, sys, pathlib
import xml.etree.ElementTree as ET

policy, source = sys.argv[1], pathlib.Path(sys.argv[2])
actions = {a.get("id"): a for a in ET.parse(policy).getroot().findall("action")}
named = set(re.findall(r'"(org\.amethystora\.vault\.[a-z]+)"', " ".join(p.read_text() for p in source.rglob("*.rs"))))
if named != set(actions):
    sys.exit(f"the service names {sorted(named)} and the policy defines {sorted(actions)}")
for ident, action in actions.items():
    defaults = {d.tag: (d.text or "").strip() for d in action.find("defaults")}
    admin = ident.endswith(".admin")
    if defaults != {"allow_any": "no", "allow_inactive": "no", "allow_active": "auth_admin" if admin else "auth_self"}:
        sys.exit(f"{ident}: {defaults}")
    owner = [a.text for a in action.findall("annotate") if a.get("key") == "org.freedesktop.policykit.owner"]
    if owner != ["unix-user:amethystora-vault"]:
        sys.exit(f"{ident}: its owner is {owner}")
text = open(policy).read()
if re.search(r">\s*(yes|auth_[a-z_]*keep)\s*<", text):
    sys.exit("a default is yes, or keeps an answer")
PY
# No rule lets anyone through, whatever they are
if grep -rlE 'org\.amethystora\.vault' /etc/polkit-1/rules.d /usr/share/polkit-1/rules.d 2>/dev/null | grep -q .; then
    fail "a polkit rule names the Vault's actions"
fi

# --- The recipes: every one the window, the manual and the knowledge name exists ------------------------------------------
for recipe in setup status vaults open lock methods change-password new rename remove-this forget item password codes auth key \
    cert pdf root ssh get run protect protected unprotect helpers sync backup restore logos tls; do
    ame --show "vault::${recipe}" >/dev/null || fail "ame vault ${recipe} is no command"
done
# A secret is never an argument, and the recipes that keep the person's place do not change folder
grep -q '^\[no-cd\]$' "${JUST}"
# every ame command of the manual page and the knowledge page is one
grep -ohE '\bame vault [a-z][a-z0-9-]*' /usr/share/amethystora/manual/vault.md /usr/share/amethystora/agents/knowledge/vault.md |
    sort -u | while read -r _ _ recipe; do
        ame --show "vault::${recipe}" >/dev/null || fail "the manual or the knowledge names ame vault ${recipe}, which is no command"
    done

# --- The agent never gets in ----------------------------------------------------------------------------------------------
# never.rs's own tests hold every spelling (the apps stage runs them); here, that what they hold is still its list
for name in '"amethystora-vault"' '"org.amethystora.vault"' '"/run/amethystora/vault"' '"/var/lib/amethystora/vault"' '"vault"'; do
    grep -qF "${name}" "${SRC}/agent-tools/src/never.rs" || fail "never.rs no longer names ${name}"
done
for spelling in '"ame vault open"' '"ame -q vault::open"' '"/usr/libexec/amethystora-vault-cli item list"' \
    '"socat - UNIX-CONNECT:/run/amethystora/vault.sock"'; do
    grep -qF "${spelling}" "${SRC}/agent-tools/src/never.rs" || fail "never.rs no longer tests ${spelling}"
done
# ...and nothing the agent is told, or may read, hands it a secret of the Vault's
grep -qi duress /usr/share/amethystora/agents/knowledge/vault.md && fail "the Vault's knowledge page names a feature it must not"
grep -q 'ame vault' /usr/share/amethystora/agents/skills/amethystora/SKILL.md || fail "the skill does not say the Vault is the user's alone"

# --- Every sentence the service says to polkit is a line of vault.just ---------------------------------------------------
SAID="$(sed -n '/^pub mod say {/,/^}/p' "${SRC}/vault-service/src/outside.rs" | sed -n 's/^ *pub const [A-Z_]*: &str = "\(.*\)";$/\1/p' | sort)"
LISTED="$(sed -n "s/^# N_ '\(.*\)'\$/\1/p" "${JUST}" | sort)"
[[ -n "${SAID}" ]] || fail "no sentences read from outside.rs"
[[ "${SAID}" == "${LISTED}" ]] || { diff <(echo "${SAID}") <(echo "${LISTED}") || true; fail "the service's sentences are not the # N_ lines of vault.just"; }

# --- Every recipe's shared shell parses: a blank line inside `words` once broke every `ame vault` ---------------------------
just --justfile "${JUST}" --evaluate words | bash -n || fail "the shell every recipe of vault.just runs (words) does not parse"

# --- No rsa crate in what is built ----------------------------------------------------------------------------------------
CRATES=/usr/share/licenses/amethystora-apps/crates/CRATES
awk -F'\t' '$1 == "rsa" { found = 1 } END { exit found }' "${CRATES}" || fail "the rsa crate is built into the apps (RUSTSEC-2023-0071)"
awk -F'\t' '$1 == "openssl" { found = 1 } END { exit !found }' "${CRATES}" || fail "the openssl crate has no licence listed"
awk -F'\t' '$1 == "argon2" { found = 1 } END { exit !found }' "${CRATES}" || fail "the argon2 crate has no licence listed"

# --- The data and its licences -------------------------------------------------------------------------------------------
DIRECTORY=/usr/share/amethystora/vault/2fa-directory.json
(($(stat -c %s "${DIRECTORY}") < 4 * 1024 * 1024)) || fail "2fa-directory.json is past what the reader takes"
jq -e '.source == "https://2fa.directory" and (.license | startswith("The MIT License")) and (.entries | length > 1000 and length <= 20000)
    and all(.entries[]; (length == 4 or length == 5) and (.[0] | type) == "string" and (.[1] | type) == "string"
    and ((.[2] | type) == "string" or .[2] == 0) and ((.[3] | type) == "string" or .[3] == 0))' "${DIRECTORY}" >/dev/null ||
    fail "2fa-directory.json is not the shape the Vault reads"
grep -q "$(sha256sum "${DIRECTORY}" | cut -d' ' -f1)" /usr/share/licenses/2fa-directory/SOURCE || fail "2fa-directory.json is not the file its SOURCE describes"
grep -q '^Copyright' /usr/share/licenses/2fa-directory/LICENSE
grep -q "$(sha256sum "${SRC}/vault-core/assets/eff_large_wordlist.txt" | cut -d' ' -f1)" /usr/share/licenses/eff-wordlists/SOURCE ||
    fail "the word list is not the one its SOURCE describes"
[[ "$(wc -l <"${SRC}/vault-core/assets/eff_large_wordlist.txt")" == 7776 ]]
grep -q 'Creative Commons Attribution 3.0 United States' /usr/share/licenses/eff-wordlists/LICENSE
grep -q 'EFF large word list' /usr/share/licenses/amethystora/NOTICE
grep -q '2fa.directory' /usr/share/licenses/amethystora/NOTICE

# --- The look of it ------------------------------------------------------------------------------------------------------
test -s /usr/share/icons/hicolor/scalable/places/amethystora-vault.svg
# The eight icon packs (the pointer sets beside them are no icon packs)
for pack in glass glass-light neon neon-light duotone line clay muted; do
    test -s "/usr/share/icons/amethystora-${pack}/apps/scalable/amethystora-vault.svg" || fail "amethystora-${pack} has no icon for the Vault"
done
grep -qx 'Icon=amethystora-vault' /usr/share/applications/amethystora-vault.desktop
grep -qx 'StartupWMClass=org.amethystora.Vault' /usr/share/applications/amethystora-vault.desktop
grep -q "'amethystora-vault.desktop'" /usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/schemas/org.gnome.shell.extensions.amethystora-ui.privacy.gschema.xml ||
    fail "the share guard does not cover the Vault's window"
jq -e '[.[].pages[]] | index("vault.md")' /usr/share/amethystora/manual/pages.json >/dev/null

echo "The Vault: its unit, policy, sockets, recipes, sentences, crates and data are as they should be"
