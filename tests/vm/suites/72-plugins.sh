#!/usr/bin/bash
# The plugin system, phase 1 (plans/PLAN-plugins.md, Tests): the root helper and the store, the host and its sandbox, with a
# hostile plugin and a hostile account, as far as phase 1 reaches. Nothing of the shell is here: the pill, the card and the dialog
# read through AT-SPI are later phases, and so are the broker's calls, the agent's drafts and a finger through polkit (suite 73).
# One phase. Every line of the hostile plugin must fail as itself, and every attempt of the account must be refused by the
# kernel or the helper:
#
#   1. The account: no write to the store, grants.json or history (the kernel); commit, grant and unblock through pkexec with
#      no password (refused); an archive with a link, a device, a FIFO, .., an absolute path, setuid bits, twice, too large, too many
#      or not a tar (refused by prepare); a file swapped in the folder after it was packed (the frozen copy is what counts);
#      a commit whose digest is not the ticket's, with a space or a newline in an argument, or a permission the manifest does not
#      name; the quarantine's quota; one changed byte in the store, as root, which stops the plugin.
#   2. The hostile plugin with no permission: the files, the buses, the network and the kernel (fixtures/hostile/plugin/run.sh), a
#      fork bomb and a memory bomb.
#   3. The same plugin granted a host that /etc/hosts points at 192.168.7.7: the proxy refuses it after resolving, a port other
#      than 443, a host nobody granted, and an address.
#   4. A block cuts what is open and survives an update; off is off (no process of the host, no bus name).
# vmlab-paths: @sf/usr/libexec/amethystora-plugin* @sf/usr/libexec/amethystora-plugins @sf/usr/share/polkit-1/*/*plugins* @sf/usr/lib/systemd/user/amethystora-plugin-host.service @sf/usr/lib/tmpfiles.d/amethystora-plugins.conf
source "$(dirname "$0")/lib.sh"

HERE="$(dirname "$0")"
FIXTURES="${HERE}/../fixtures/hostile"
HOST=/usr/libexec/amethystora-plugin-host
ROOT=/usr/libexec/amethystora-plugin-root
STORE=/var/lib/amethystora/plugins
ID=dev.example.hostile
WORK=/var/lib/vmlab-tests/72-plugins
rm -rf "${WORK}"
install -d -m 0755 "${WORK}"

if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}" "$(loginctl list-sessions --no-legend)"
    finish
fi
UID_TEST="$(id -u "${TEST_USER}")"
HOME_TEST="$(getent passwd "${TEST_USER}" | cut -d: -f6)"

as_tester() { runuser -u "${TEST_USER}" -- "$@"; }
# The helper as pkexec starts it, for the account's own uid: what the kernel and the helper refuse is the same
helper() { PKEXEC_UID="${UID_TEST}" "${ROOT}" "$@"; }
field() { awk -F'\t' -v key="$1" '$1 == key { print $2; exit }'; }
refused() { # refused WHAT COMMAND...: not ok when the command succeeded
    local what="$1" out
    shift
    if out="$("$@" 2>&1)"; then not_ok "${what}" "it was accepted: ${out}"; else ok "${what}"; fi
}

# The plugin, with the test account's home written into it, packed as a person would add it
make_plugin() { # make_plugin FOLDER [MANIFEST]
    rm -rf "$1"
    install -d -m 0755 "$1"
    cp "${FIXTURES}/plugin/${2:-manifest.json}" "$1/manifest.json"
    sed "s|@HOME@|${HOME_TEST}|; s|@UID@|${UID_TEST}|" "${FIXTURES}/plugin/run.sh" >"$1/run.sh"
    chmod 0755 "$1/run.sh"
}
install_plugin() { # install_plugin FOLDER PERMISSION...: frozen, then committed with exactly these
    local folder="$1" out ticket digest
    shift
    out="$(as_tester "${HOST}" pack "${folder}" | helper prepare)" || return 1
    ticket="$(field ticket <<<"${out}")"
    digest="$(field digest <<<"${out}")"
    helper inspect "${ticket}" >/dev/null || return 1
    helper commit "${ID}" "${digest}" "$@" >/dev/null || return 1
    echo "${digest}"
}

# --- The account's own reach: ssh files it should not lose, and a folder to try to write -----------------------------------
install -d -m 0700 -o "${TEST_USER}" "${HOME_TEST}/.ssh"
echo "not a real key" >"${HOME_TEST}/.ssh/id_test"
chown "${TEST_USER}" "${HOME_TEST}/.ssh/id_test"

make_plugin "${WORK}/plugin"
check "the hostile plugin passes the check, with warnings about the tools a sandbox has no bus for" \
    sh -c "'${HOST}' check '${WORK}/plugin' | grep -q '^warning'"
DIGEST="$(install_plugin "${WORK}/plugin")" && ok "prepare, inspect and commit freeze the plugin and grant it nothing" || not_ok "the plugin could not be installed"
FOLDER="${STORE}/store/${DIGEST}"

# --- 1. The hostile account ---------------------------------------------------------------------------------------------------
refused "the account cannot write the store" as_tester sh -c "echo x >> '${FOLDER}/run.sh'"
refused "the account cannot add a file to a plugin's folder" as_tester touch "${FOLDER}/added"
refused "the account cannot write grants.json" as_tester sh -c "echo '{}' > '${STORE}/accounts/${UID_TEST}/grants.json'"
refused "the account cannot write the history" as_tester sh -c "echo '{}' >> '${STORE}/accounts/${UID_TEST}/history.jsonl'"
refused "the account cannot make a ticket of its own in the quarantine" as_tester mkdir "${STORE}/quarantine/mine"
refused "the account cannot even list the quarantine" as_tester ls "${STORE}/quarantine"
# commit and unblock through pkexec, as the account would from a script with no password and no agent to answer
for verb in commit grant unblock rollback; do
    refused "${verb} through pkexec needs an administrator, and a script has none" \
        as_tester setsid timeout 30 pkexec "${ROOT}" "${verb}" "${ID}" "${DIGEST}" notify </dev/null
done
refused "the helper does not run as a plain account" as_tester "${ROOT}" prepare </dev/null
refused "the helper does not run as root with no account behind it" "${ROOT}" prepare </dev/null

python3 "${FIXTURES}/archives.py" "${WORK}/archives" "${FIXTURES}/plugin/manifest.json"
for archive in "${WORK}"/archives/*.tar; do
    refused "prepare refuses $(basename "${archive}" .tar)" helper prepare <"${archive}"
done
# The helper's own lock file stays: only a ticket would be what was refused
kept="$(ls -A "${STORE}/quarantine" | grep -vx '.lock')"
if [[ -z "${kept}" ]]; then ok "nothing of the refused archives was kept"; else not_ok "the quarantine holds what was refused" "${kept}"; fi

# A file swapped after the pack is not what was frozen: the digest is the packed folder's
make_plugin "${WORK}/swap"
packed="${WORK}/swap.tar"
as_tester "${HOST}" pack "${WORK}/swap" >"${packed}"
echo "echo swapped" >>"${WORK}/swap/run.sh"
out="$(helper prepare <"${packed}")"
digest_swapped="$(field digest <<<"${out}")"
make_plugin "${WORK}/swap"
if [[ "${digest_swapped}" == "$(as_tester "${HOST}" pack "${WORK}/swap" | helper prepare | field digest)" ]]; then
    ok "what was frozen is what was packed, whatever happens to the folder after"
else
    not_ok "the digest follows the folder, not the archive"
fi
rm -rf "${STORE}/quarantine"/*

# A commit is held to what the person saw
out="$(as_tester "${HOST}" pack "${WORK}/plugin" | helper prepare)"
ticket="$(field ticket <<<"${out}")"
fresh="$(field digest <<<"${out}")"
refused "a commit whose digest is not the ticket's" helper commit "${ID}" "$(printf '0%.0s' {1..64})" notify
refused "a space in a permission" helper commit "${ID}" "${fresh}" "notify open"
refused "a newline in a permission" helper commit "${ID}" "${fresh}" $'notify\nopen'
refused "a permission the manifest does not name" helper commit "${ID}" "${fresh}" notify
refused "a permission with a shell character" helper commit "${ID}" "${fresh}" 'network=a.example.com;id'
refused "another id than the ticket's" helper commit dev.example.other "${fresh}"
other_uid=$((UID_TEST + 1))
refused "another account's ticket" env PKEXEC_UID="${other_uid}" "${ROOT}" commit "${ID}" "${fresh}"
refused "inspect of a ticket that is not the account's" env PKEXEC_UID="${other_uid}" "${ROOT}" inspect "${ticket}"
for _ in 1 2 3 4; do
    as_tester "${HOST}" pack "${WORK}/plugin" | helper prepare >/dev/null 2>&1
done
refused "the quarantine's quota for one account" sh -c "'${HOST}' pack '${WORK}/plugin' | PKEXEC_UID=${UID_TEST} '${ROOT}' prepare"
rm -rf "${STORE}/quarantine"/*

# --- 2. The hostile plugin, with no permission ----------------------------------------------------------------------------------
python3 -m http.server 8099 --bind 127.0.0.1 >/dev/null 2>&1 &
SERVER=$!
trap 'kill ${SERVER} 2>/dev/null' EXIT
# Rows of a card: the number of attempts expected, and none a LEAK
hostile() { # hostile ACTION COUNT
    local out leaks rows
    out="$(in_session "${HOST} try ${ID} $1" 2>&1)" || { not_ok "the hostile plugin's $1 run failed" "${out}"; return; }
    rows="$(jq -r '.card | length' <<<"${out}" 2>/dev/null)"
    leaks="$(jq -r '.card[] | select(.value != "blocked") | .label' <<<"${out}" 2>/dev/null)"
    [[ "${rows}" == "$2" ]] && ok "the hostile plugin made all $2 attempts of $1" || not_ok "the $1 run made ${rows:-none} of $2 attempts" "${out}"
    [[ -z "${leaks}" ]] && ok "every attempt of $1 failed as itself" || not_ok "something of $1 worked" "${leaks}"
}
hostile fs 12
hostile bus 8
hostile net 6
hostile proc 10
[[ ! -e "${HOME_TEST}/leaked" && ! -e "${HOME_TEST}/leaked2" && ! -e /usr/leaked && ! -e /etc/leaked ]] && ok "nothing was written outside the sandbox" || not_ok "a file the plugin made is on the machine"
# A fork bomb stays within its 64 tasks, and a memory bomb ends the run
out="$(in_session "${HOST} try ${ID} bomb" 2>&1)" || true
[[ "$(jq -r '.card[0].value' <<<"${out}" 2>/dev/null)" == blocked ]] && ok "a fork bomb is held to its tasks" || not_ok "the fork bomb was not held" "${out}"
out="$(in_session "${HOST} try ${ID} membomb" 2>&1)" && not_ok "a memory bomb ended its run in a result" "${out}" || ok "a memory bomb is stopped by its scope's limit"
[[ -z "$(pgrep -u "${TEST_USER}" -f 'time.sleep\(30\)')" ]] && ok "no process of the fork bomb is left" || not_ok "the fork bomb's processes outlived the run"

# --- 3. The network ----------------------------------------------------------------------------------------------------------------
grep -q 'granted.example.test' /etc/hosts || echo '192.168.7.7 granted.example.test' >>/etc/hosts
make_plugin "${WORK}/plugin-network" manifest-network.json
DIGEST_NETWORK="$(install_plugin "${WORK}/plugin-network" network=granted.example.test)" && ok "an update that adds a host is committed with exactly that host" || not_ok "the network variant could not be installed"
sleep 6
out="$(in_session "${HOST} try ${ID} netp" 2>&1)" || not_ok "the proxy run failed" "${out}"
[[ "$(jq -r '.card | length' <<<"${out}" 2>/dev/null)" == 4 && -z "$(jq -r '.card[] | select(.value != "blocked") | .label' <<<"${out}" 2>/dev/null)" ]] &&
    ok "the proxy refuses a host that resolves to the router, port 22, a host nobody granted and an address" ||
    not_ok "something got through the proxy" "${out}"
SANDBOX="$(as_tester "${HOST}" sandbox "${ID}")"
[[ "${SANDBOX}" == *'--unshare-all'* && "${SANDBOX}" == *'--disable-userns'* && "${SANDBOX}" == *'--seccomp 9'* && "${SANDBOX}" != *'--share-net'* ]] && ok "ame plugin show prints the plan's recipe" || not_ok "the printed sandbox is not the recipe" "${SANDBOX}"
as_tester "${HOST}" show "${ID}" | grep -q '^previous' && ok "an update keeps the last version for a rollback" || not_ok "show lost the previous version"

# --- 4. A block, an update, and what a changed byte does -------------------------------------------------------------------------------
helper block "${ID}" granted.example.test >/dev/null
if as_tester "${HOST}" show "${ID}" --json | jq -e '.blocked_hosts | index("granted.example.test")' >/dev/null; then
    ok "a block is in the grants"
else
    not_ok "the block is missing from the grants"
fi
out="$(as_tester "${HOST}" pack "${WORK}/plugin" | helper prepare)"
helper commit "${ID}" "$(field digest <<<"${out}")" >/dev/null
if as_tester "${HOST}" show "${ID}" --json | jq -e '.blocked_hosts | index("granted.example.test")' >/dev/null; then
    ok "an update does not clear a block"
else
    not_ok "an update cleared the block"
fi
helper unblock "${ID}" granted.example.test >/dev/null && ok "unblock lifts it" || not_ok "unblock failed"
refused "unblock through pkexec needs an administrator" as_tester setsid timeout 30 pkexec "${ROOT}" unblock "${ID}" granted.example.test </dev/null
# Root changes one byte of what the person approved: the plugin stops until they decide
current="$(as_tester "${HOST}" show "${ID}" | field digest)"
echo "# one byte" >>"${STORE}/store/${current}/run.sh"
out="$(in_session "${HOST} try ${ID}" 2>&1)" && not_ok "a plugin that changed ran" "${out}" || ok "a plugin whose files changed does not run"
as_tester "${HOST}" verify "${ID}" | grep -q '^changed' && ok "verify names the change" || not_ok "verify did not see the change"
refused "the account cannot put the byte back" as_tester sh -c "echo x > '${STORE}/store/${current}/run.sh'"

# Off is off: nothing of the host is running, and the bus name is gone
as_tester "${HOST}" disable "${ID}" 2>/dev/null
if wait_for 30 sh -c "! pgrep -u ${TEST_USER} -f ^/usr/libexec/amethystora-plugin-host >/dev/null"; then ok "with no plugin on, no process of the host is left"; else not_ok "the host runs with no plugin on"; fi
user_bus busctl --user list 2>/dev/null | grep -q org.amethystora.Plugins && not_ok "the host's bus name is still owned" || ok "the host's bus name is gone"
finish
