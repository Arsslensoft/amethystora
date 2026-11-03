#!/usr/bin/bash
# Amethystora Vault (plans/PLAN-vault.md, Tests) as a person goes through it, on a booted image with SELinux enforcing, real polkit
# and the real service under systemd. Everything here is something a build cannot start: cargo holds the formats, the crypto and the
# protocol against separate programs, and build_files/shared/test-vault.sh holds what the image ships. In three phases (74 comes
# after 72 and 73, which are the plugins'):
#
#   1. the image as shipped (sockets, nothing running, nothing kept, no root CA), the cold start, setup in a terminal with polkit's
#      question put to a text agent and the recovery key typed back, the boundaries (the folder, /proc, the network, another
#      account, a Flatpak, the sealed purposes Round 7 builds on), items and their approvals, what the question says, and the
#      question in GNOME Shell's own dialog, left on the screen for the lab's screenshot
#   2. the dialog's text and picture checked and answered, the ways it opens (the account's password through polkit, the TPM, no
#      prompt, the fingerprint on a virtual reader), what closes it (the screen, a suspend, idle, the service killed, no core), what
#      is idle (the service, an open vault), SSH, protected files, the machine's root and a signed PDF, the authenticator, a hidden
#      vault, the sealed backup, forgetting and restoring, the network (and a Vaultwarden container), SELinux's denials
#   3. after a restart: closed, the files and the root still there, and everything given back
#
# Not run, and said so where each would be: a FIDO2 security key (cargo holds the slot against stand-ins), the camera, a picture's QR
# through the window's file chooser, the browser extension (it has its own harness in apps/vault-extension/tests, which runs the image's
# Firefox through Marionette and Chromium through chromedriver), a second account with a seat of its own, and Bitwarden's own server
# (the maintainer's, by hand). BackMyCodes' own app reading what this writes is done by hand when a format changes.
# vmlab-paths: apps/vault* @sf/usr/libexec/amethystora-vault* @sf/usr/lib/systemd/system/amethystora-vault* @sf/usr/share/polkit-1/actions/org.amethystora.vault.policy @sf/usr/share/amethystora/just/vault.just @sf/usr/bin/amethystora-vault @sf/usr/lib/sysusers.d/amethystora-vault* @sf/usr/share/amethystora/vault/* @sf/usr/libexec/amethystora-backup
source "$(dirname "$0")/lib.sh"

HERE="$(cd "$(dirname "$0")" && pwd)"
UID_T="$(id -u "${TEST_USER}")"
HOME_DIR="/var/home/${TEST_USER}"
WORK=/run/vmlab-vault
STATE=/var/lib/vmlab-tests/vault.env
ASK_OUT="${HOME_DIR}/.cache/vmlab-vault-ask.out"
STATEDIR=/var/lib/amethystora/vault
SERVICE=amethystora-vault.service
PASSWORD="${TEST_USER}"
VAULT_PASSWORD='Vmlab-vault-2026'
LAB_SECRET='Lab-secret-2026-Zq'
# The first test key of RFC 6238 and 4226: the ASCII digits 12345678901234567890
TOTP_KEY='GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ'
RECOVERY_RE='([A-Z0-9]{5}-){10}[A-Z0-9]{5}'
# The typographic quotes polkit's sentences put round an item's name, as bytes (ShellCheck takes the characters for a mistake)
LQ=$'\xe2\x80\x9c'
RQ=$'\xe2\x80\x9d'
AVC_SINCE="$(date '+%Y-%m-%d %H:%M:%S')"

same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
like() { if [[ "$2" == *"$3"* ]]; then ok "$1"; else not_ok "$1" "expected to hold: $3" "found: $(head -c 600 <<<"$2")"; fi; }
unlike() { if [[ "$2" != *"$3"* ]]; then ok "$1"; else not_ok "$1" "must not hold: $3" "found: $(head -c 600 <<<"$2")"; fi; }
as_tester() { runuser -u "${TEST_USER}" -- "$@"; }
tester_session() { in_session "$*"; }
sock() { as_tester python3 "${WORK}/vaultsock.py" "$@"; }
# The vault's own command line as the account, which prints values and tab-separated lines
cli() { in_session "/usr/libexec/amethystora-vault-cli $*" </dev/null; }
is_open() { [[ "$(cli status | awk -F'\t' '$1 == "vault" { print $3 }' | head -n 1)" == open ]]; }
is_closed() { ! is_open; }
service_pid() { systemctl show -p MainPID --value "${SERVICE}"; }
service_running() { [[ "$(systemctl is-active "${SERVICE}")" == active ]]; }
service_gone() { ! service_running; }
ensure_session() { has_session || wait_for 120 has_session; }
remember() { printf '%s=%q\n' "$1" "$2" >>"${STATE}"; }
# shellcheck source=/dev/null
[[ ! -s "${STATE}" ]] || source "${STATE}"
# Polkit's question at a terminal, answered with the account's own password, when it is asked at all
PW=('?Password: ' "${PASSWORD}"$'\r')

# terminal SECONDS [EXPECT SEND]... -- SCRIPT: SCRIPT in a terminal of the account's session, as Ptyxis opens one: in the account's user
# manager, which polkit takes for the seat's session. @recovery as a SEND is the key the Vault showed (vaultterm.py)
terminal() {
    local seconds="$1" steps=()
    shift
    while [[ $# -gt 0 && "$1" != -- ]]; do steps+=("$1"); shift; done
    shift
    ensure_session || true
    python3 "${HERE}/vaultterm.py" "${seconds}" "${steps[@]}" -- runuser -u "${TEST_USER}" -- env XDG_RUNTIME_DIR="/run/user/${UID_T}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${UID_T}/bus" LC_ALL=en_US.UTF-8 LANGUAGE=en \
        systemd-run --user --pty --wait --quiet --collect -- /usr/bin/bash -lc "$*"
}
# av SECONDS [EXPECT SEND]... -- WORDS: `ame vault WORDS` in that terminal, with polkit's own text agent registered for its shell, which
# prints what polkit asks (the sentence, as polkit made it from the service's, and what is asked) and takes the account's password: the
# questions of GNOME's dialog, put where a suite can read them. `$$$$` is systemd-run's escape for `$$`, `$$?` for `$?`
av() {
    local seconds="$1" steps=()
    shift
    while [[ $# -gt 0 && "$1" != -- ]]; do steps+=("$1"); shift; done
    shift
    terminal "${seconds}" "${steps[@]}" -- 'pkttyagent --process $$$$ & sleep 1; cd; ame vault '"$*"'; s=$$?; kill %1; exit $$s'
}
asked() { grep -c 'AUTHENTICATING FOR' <<<"$1"; }
# The vault opened by its password, typed in a terminal (no polkit question: it is the password that opens it)
open_by_password() {
    av 90 'Vault password or recovery key: ' "${VAULT_PASSWORD}"$'\r' -- open password >/dev/null
    is_open
}
lock_it() { cli lock >/dev/null; wait_for 20 is_closed; }

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
mkdir -p "${WORK}" /var/lib/vmlab-tests
cp "${HERE}/atspi.py" "${HERE}/keys.py" "${HERE}/vaultsock.py" "${WORK}/"
chmod -R a+rX "${WORK}"
as_tester mkdir -p "${HOME_DIR}/.cache"
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"

# ====================================================================================================================================
if ((SUITE_PHASE == 1)); then
# ====================================================================================================================================
: >"${STATE}"
chmod 0600 "${STATE}"

# --- 1. The image as shipped ------------------------------------------------------------------------------------------------------
# A machine that was never set up costs only two sockets. Whatever an earlier suite left of a vault of the account is forgotten first,
# as a person would (the typed word FORGET), so that what follows starts from nothing
if [[ "$(cli status | awk -F'\t' '$1 == "set-up" { print $2 }')" == yes ]]; then
    av 90 'type: FORGET' $'FORGET\r' "${PW[@]}" -- forget >/dev/null
    echo "# an earlier suite left a vault of the account: forgotten first"
fi
check "the Vault's socket is listening" systemctl is-active amethystora-vault.socket
check "the SSH agent's socket is listening" systemctl is-active amethystora-vault-ssh.socket
same "both are enabled, and the service itself is not" \
    "$(systemctl is-enabled amethystora-vault.socket amethystora-vault-ssh.socket "${SERVICE}" 2>&1 | paste -sd ' ')" "enabled enabled disabled"
if wait_for 90 service_gone; then
    ok "nothing runs while no vault is open: the service has gone"
else
    not_ok "the service still runs with nothing open" "$(systemctl status "${SERVICE}" --no-pager 2>&1 | head -n 8)"
fi
same "no process of the Vault's is running" "$(pgrep -fa 'amethystora-vault-(service|work|net)' | grep -v vmlab || true)" ""
same "no timer of the Vault's, for the system or the account" \
    "$( { systemctl list-timers --all --no-legend --no-pager; user_systemctl list-timers --all --no-legend --no-pager; } 2>/dev/null | grep -ci vault)" 0
if [[ ! -e "${STATEDIR}/machine" ]]; then
    ok "no root certificate exists before anyone needs one"
else
    skip "a root certificate exists already (an earlier suite made one): the machine's root is checked from here on"
fi
same "the network service is not running, and its socket waits" "$(systemctl is-active amethystora-vault-net.service amethystora-vault-net.socket 2>&1 | paste -sd ' ')" "inactive active"

# --- 2. The cold start -----------------------------------------------------------------------------------------------------------
systemctl stop "${SERVICE}"
IFS=$'\t' read -r ms answer <<<"$(sock --time '{"v":1,"op":"status"}' | head -n 1)"
if jq -e '.ok == true' <<<"${answer:-null}" >/dev/null 2>&1; then
    echo "# the service, cold start to its first answer: ${ms} ms (the budget is 150 ms on real hardware; this VM computes in software)"
    if ((${ms%.*} < 1500)); then ok "the socket starts the service and gets an answer"; else not_ok "the first answer took ${ms} ms"; fi
else
    not_ok "the socket did not start the service and get an answer" "${answer:-}" "$(journalctl -u "${SERVICE}" -b --no-pager -o cat | tail -n 8)"
fi
like "an account with no vault is told so, by the command" "$(tester_session 'ame vault status')" "no vault"
like "a request that is not one is answered, and the service answers the next" \
    "$(sock '{"v":1,"op":"nonesuch"}' 'not json' '{"v":2}' '{"v":1,"op":"status"}' | tail -n 1)" '"ok":true'

# --- 3. Setup, in a terminal ---------------------------------------------------------------------------------------------------
# The password twice, polkit's question put to the text agent (the account's own password), the recovery key shown and typed back
out="$(av 150 'New vault password: ' "${VAULT_PASSWORD}"$'\r' 'Once more: ' "${VAULT_PASSWORD}"$'\r' 'Password: ' "${PASSWORD}"$'\r' \
    'Type the recovery key back: ' '@recovery'$'\r' -- setup password)"
rc=$?
RECOVERY="$(grep -oE "${RECOVERY_RE}" <<<"${out}" | tail -n 1)"
if ((rc == 0)) && [[ -n "${RECOVERY}" ]]; then
    ok "ame vault setup password: the recovery key is shown and typed back, and the vault is made"
    like "...and it says so" "${out}" "The vault is set up and open"
    remember RECOVERY "${RECOVERY}"
else
    not_ok "ame vault setup password (${rc})" "$(tail -n 14 <<<"${out}")"
fi
like "polkit's question named the action" "${out}" "AUTHENTICATING FOR org.amethystora.vault.open"
like "...and the program that asks, from the sentence the service passed" "${out}" "A program calling itself"
unlike "...with no placeholder left in it" "${out}" '$('
check "the vault is open after setup" is_open
like "its way of opening is the password" "$(cli status | awk -F'\t' '$1 == "vault" { print $5 }' | head -n 1)" password
if wait_for 20 service_running; then ok "the service runs now"; else not_ok "the service is not running after setup"; fi
SERVICE_PID="$(service_pid)"
same "...as its own user, amethystora-vault" "$(ps -o user= -p "${SERVICE_PID}" | tr -d ' ')" amethystora-vault

# --- 4. The boundaries, as the same account and unapproved ------------------------------------------------------------------------
same "the folder is the service's, closed to everyone else" "$(stat -c '%U %a' "${STATEDIR}")" "amethystora-vault 700"
same "...and the account's folder in it" "$(stat -c '%U %a' "${STATEDIR}/${UID_T}")" "amethystora-vault 700"
if as_tester ls "${STATEDIR}" >/dev/null 2>&1; then not_ok "the account can list the vault's folder"; else ok "the account cannot list the vault's folder"; fi
if as_tester sh -c "cat ${STATEDIR}/${UID_T}/* ${STATEDIR}/${UID_T}/*/* 2>/dev/null | head -c 1" 2>/dev/null | grep -q .; then
    not_ok "the account read a file of the vault"
else
    ok "the account cannot read a file of the vault"
fi
if as_tester sh -c "head -c 16 /proc/${SERVICE_PID}/environ" >/dev/null 2>&1; then not_ok "the account read the service's environment"; else ok "the account cannot read the service's environment"; fi
if as_tester sh -c "head -c 16 /proc/${SERVICE_PID}/mem" >/dev/null 2>&1; then not_ok "the account read the service's memory"; else ok "the account cannot read the service's memory"; fi
# PTRACE_SEIZE attaches without stopping: refused, or the service is left running as it was
if as_tester python3 -c '
import ctypes, sys
libc = ctypes.CDLL(None, use_errno=True)
sys.exit(0 if libc.ptrace(0x4206, int(sys.argv[1]), 0, 0) == 0 else 1)' "${SERVICE_PID}" 2>/dev/null; then
    not_ok "the account attached to the service"
else
    ok "the account cannot attach to the service"
fi
same "the service is not dumpable (its /proc entries are root's)" "$(stat -c %U "/proc/${SERVICE_PID}")" root
same "no core file is ever written (the limit is 0 and 0)" "$(awk '/Max core file size/ { print $5, $6 }' "/proc/${SERVICE_PID}/limits")" "0 0"
same "no capability is held" "$(awk '/^CapEff/ { print $2 }' "/proc/${SERVICE_PID}/status")" 0000000000000000
same "the service has no network: its own, with only a loopback" "$(nsenter -t "${SERVICE_PID}" -n ip -o link | wc -l)" 1
same "...and the unit says so" "$(systemctl show -p PrivateNetwork --value "${SERVICE}")" yes
# Another account: no session of its own, and no way to the account's vault or its folder
useradd -m vmlabother 2>/dev/null
OTHER="$(runuser -u vmlabother -- python3 "${WORK}/vaultsock.py" '{"v":1,"op":"status"}' 2>&1)"
if jq -e '.set_up == true or ((.vaults // []) | length > 0)' <<<"${OTHER}" >/dev/null 2>&1; then
    not_ok "another account sees a vault through the socket" "${OTHER}"
else
    ok "another account gets no vault of this one's from the socket (${OTHER:0:60})"
fi
if runuser -u vmlabother -- ls "${STATEDIR}/${UID_T}" >/dev/null 2>&1; then not_ok "another account lists the account's folder"; else ok "another account cannot list the account's folder"; fi
# A Flatpak with the whole filesystem granted
APP="$(user_bus flatpak list --app --columns=application 2>/dev/null | head -n 1)"
if [[ -n "${APP}" ]]; then
    reach="$(user_bus flatpak run --filesystem=host --filesystem=home --command=sh "${APP}" -c 'test -S /run/amethystora/vault.sock && echo reachable || echo unreachable' 2>&1 | tail -n 1)"
    same "a Flatpak with host and home granted cannot reach the socket (${APP})" "${reach}" unreachable
else
    skip "no Flatpak app installed in the lab to try the socket from (the first boot's installs need the network)"
fi
# The sealed purposes Round 7's features use, while the vault is open
RECORD="vmlab-$$"
DATA="$(printf 'vault seal test' | base64)"
SEALED="$(sock "{\"v\":1,\"op\":\"seal\",\"purpose\":\"vmlab\",\"record\":\"${RECORD}\",\"data\":\"${DATA}\"}" | jq -r '.data // empty')"
if [[ -n "${SEALED}" ]]; then
    same "seal and open keep a purpose's data" "$(sock "{\"v\":1,\"op\":\"open\",\"purpose\":\"vmlab\",\"record\":\"${RECORD}\",\"data\":\"${SEALED}\"}" | jq -r '.data // empty' | base64 -d)" "vault seal test"
    unlike "...and a record of another name does not open it" "$(sock "{\"v\":1,\"op\":\"open\",\"purpose\":\"vmlab\",\"record\":\"other\",\"data\":\"${SEALED}\"}")" '"ok":true'
else
    not_ok "an open vault sealed nothing" "$(sock "{\"v\":1,\"op\":\"seal\",\"purpose\":\"vmlab\",\"record\":\"${RECORD}\",\"data\":\"${DATA}\"}")"
fi

# --- 5. Items and what each asks -----------------------------------------------------------------------------------------------
printf '%s\n' "${LAB_SECRET}" | in_session "ame vault password add lab-login lab https://lab.example" >/dev/null 2>&1
LISTED="$(tester_session 'ame vault password list')"
like "a password is kept, with its username and address" "${LISTED}" $'lab-login\tlab\thttps://lab.example'
ITEM="$(awk -F'\t' '$2 == "lab-login" { print $1 }' <<<"${LISTED}")"
remember ITEM "${ITEM}"
# A password is While open as it comes: asked once, then not again while the vault stays open
out="$(av 120 "${PW[@]}" -- 'password get lab-login >/dev/null; ame vault password get lab-login')"
same "a password asks once while the vault is open" "$(asked "${out}")" 1
like "...the second use gave it" "${out}" "${LAB_SECRET}"
like "the question names the item between its quotes" "${out}" "asks to use ${LQ}lab-login${RQ} from Amethystora Vault"
like "...and the program that asks" "${out}" "A program calling itself"
unlike "...and leaves no placeholder in the sentence" "${out}" '$('
# Ask every time: a question each time, and a no is a refusal
av 90 "${PW[@]}" -- item approval password "${ITEM}" ask >/dev/null
out="$(av 120 "${PW[@]}" '?Password: ' "${PASSWORD}"$'\r' -- 'password get lab-login >/dev/null; ame vault password get lab-login')"
same "Ask every time asks at each use" "$(asked "${out}")" 2
# Ctrl-D at the password prompt is the person saying no: polkit's agent ends, which is no failed login to count against the account
out="$(av 120 'Password: ' $'\x04' -- password get lab-login)"
rc=$?
faillock --user "${TEST_USER}" --reset 2>/dev/null
if ((rc != 0)) && [[ "${out}" != *"${LAB_SECRET}"* ]]; then
    ok "a program whose question is answered no gets a refusal and nothing of the secret"
else
    not_ok "a refused question still gave something (${rc})" "$(tail -n 8 <<<"${out}")"
fi
like "a question answered no leaves the service answering" "$(sock '{"v":1,"op":"status"}')" '"ok":true'

# --- 6. The question in GNOME Shell's own dialog, with its picture left for the lab -------------------------------------------
# No text agent here: the question goes to the shell's agent, which draws it over the desktop. The next phase reads what it says and
# looks at the screen the lab took, then answers it
rm -f "${ASK_OUT}"
user_bus systemd-run --user --unit vmlab-vault-ask --collect --quiet --setenv=LC_ALL=en_US.UTF-8 --setenv=LANGUAGE=en \
    -p "StandardOutput=file:${ASK_OUT}" -p "StandardError=file:${ASK_OUT}.err" -- /usr/bin/bash -lc 'ame vault password get lab-login'
dialog_up() { user_bus python3 "${WORK}/atspi.py" gnome-shell has "asks to use" >/dev/null 2>&1; }
if wait_for 60 dialog_up; then
    ok "GNOME Shell shows the Vault's question in its own dialog"
    remember DIALOG yes
else
    skip "GNOME Shell's dialog is not in the accessibility tree (atspi.py gnome-shell), or no dialog came: the text agent above read the same sentence"
    remember DIALOG no
fi
screen_next
continue_phase
fi

# ====================================================================================================================================
if ((SUITE_PHASE == 2)); then
# ====================================================================================================================================
SCREEN_PNG=/var/lib/vmlab-tests/screen.png

# --- 6 again. The dialog's words and picture, and the answer -------------------------------------------------------------------
if [[ "${DIALOG:-no}" == yes ]]; then
    TEXT="$(user_bus python3 "${WORK}/atspi.py" gnome-shell text 2>/dev/null)"
    like "the dialog says who asks, from the sentence polkit made" "${TEXT}" "A program calling itself"
    like "...and which item, expanded from the service's details" "${TEXT}" "${LQ}lab-login${RQ}"
    unlike "...with no placeholder left in it" "${TEXT}" '$('
    if [[ -s "${SCREEN_PNG}" ]]; then
        # The lab's own picture of the screen, as a person sees it: something is drawn in its middle, which a blank screen would not show
        colours="$(python3 -c '
import sys
import gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1])
w, h, n, r, d = p.get_width(), p.get_height(), p.get_n_channels(), p.get_rowstride(), p.get_pixels()
seen = set()
for y in range(h // 3, 2 * h // 3, 3):
    for x in range(w // 4, 3 * w // 4, 3):
        o = y * r + x * n
        seen.add((d[o] >> 3, d[o + 1] >> 3, d[o + 2] >> 3))
print(len(seen))' "${SCREEN_PNG}" 2>/dev/null)"
        if [[ "${colours:-0}" -gt 40 ]]; then ok "the screen the lab took shows the dialog's drawing in its middle (${colours} colours)"; else not_ok "the lab's screen has ${colours:-no} colours in its middle"; fi
    else
        not_ok "the lab handed no screen over (run.mjs screen_next)"
    fi
    python3 "${WORK}/keys.py" "text:${PASSWORD}" return >/dev/null
    if wait_for 30 grep -qF "${LAB_SECRET}" "${ASK_OUT}"; then
        ok "the password typed into the dialog answers the question, and the secret comes out"
    else
        not_ok "the dialog's answer did not release the secret" "$(cat "${ASK_OUT}" "${ASK_OUT}.err" 2>&1 | tail -n 6)"
    fi
else
    skip "the dialog was not seen in phase 1: its answer is the text agent's, tested above"
fi
user_systemctl stop vmlab-vault-ask.service >/dev/null 2>&1
rm -f "${ASK_OUT}" "${ASK_OUT}.err"

# --- 7. The ways it opens ------------------------------------------------------------------------------------------------------
# The TPM, and the account's password through polkit
TPM="$(cli status | awk -F'\t' '$1 == "tpm" { print $2 }')"
if [[ "${TPM}" == yes ]]; then
    out="$(av 90 "${PW[@]}" -- methods add approval)"
    like "the TPM slot is added, after polkit's question" "${out}" "AUTHENTICATING FOR org.amethystora.vault.open"
    like "...and listed among the ways in" "$(cli methods)" approval
    if lock_it; then ok "lock closes the vault"; else not_ok "lock did not close the vault"; fi
    began="$(date +%s.%N)"
    out="$(av 90 "${PW[@]}" -- open approval)"
    took="$(awk -v a="${began}" -v b="$(date +%s.%N)" 'BEGIN { printf "%.1f", b - a }')"
    if is_open && [[ "$(asked "${out}")" == 1 ]]; then
        ok "the vault opens with the account's password through polkit, with nothing of the vault's own typed"
        echo "# opening by the TPM and polkit, through a terminal: ${took} s (systemd-creds takes about half of it)"
    else
        not_ok "the vault did not open by the TPM and polkit" "$(tail -n 8 <<<"${out}")"
    fi
    # No prompt: nothing asked at all, and a program of the account that asks gets everything: the manual says so, and here it is shown
    av 60 'type: no prompt' $'no prompt\r' "${PW[@]}" -- methods add no-prompt >/dev/null
    lock_it
    out="$(av 60 -- open no-prompt)"
    if is_open && [[ "$(asked "${out}")" == 0 ]]; then
        ok "no prompt opens the vault with nothing asked"
    else
        not_ok "no prompt did not open the vault without asking" "$(tail -n 8 <<<"${out}")"
    fi
    lock_it
    like "...and a plain program of the account opens it as well, which is what the manual says it gives up" \
        "$(sock '{"v":1,"op":"unlock","method":"no-prompt"}')" '"ok":true'
    SLOT="$(cli methods | jq -r '[.. | objects | select(.kind? == "no-prompt") | .id][0] // empty')"
    if [[ -n "${SLOT}" ]]; then
        av 60 "${PW[@]}" -- methods remove "${SLOT}" >/dev/null
        lock_it
        unlike "with the slot removed, no prompt is gone" "$(sock '{"v":1,"op":"unlock","method":"no-prompt"}')" '"ok":true'
    else
        skip "the no prompt slot's id was not found in the methods' answer, so it is left for the end"
    fi
else
    skip "this machine has no TPM: the approval and no prompt methods are not offered"
fi
open_by_password && ok "the vault opens with its password, typed" || not_ok "the vault did not open with its password"
lock_it
av 60 'Vault password or recovery key: ' $'wrong-password\r' -- open password >/dev/null
if (($? != 0)) && is_closed; then ok "a wrong vault password opens nothing"; else not_ok "a wrong vault password did something"; fi
open_by_password
skip "the security key as a way in: no FIDO2 device in the lab (cargo holds the slot against stand-ins, as Notes' passkeys)"

# The fingerprint, on libfprint's virtual reader, as suite 84 sets it up: a lab-only SELinux rule for its socket and a finger enrolled
READER=/var/lib/fprint/vmlab-reader
reader_send() {
    python3 - "$1" "${READER}" <<'PY'
import socket, sys
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[2])
s.sendall(sys.argv[1].encode())
s.close()
PY
}
feed_scans() {
    local id="$1" deadline=$((SECONDS + $2))
    shift 2
    until "$@" >/dev/null 2>&1; do
        ((SECONDS < deadline)) || return 1
        reader_send "SCAN ${id}" 2>/dev/null || true
        sleep 1.5
    done
}
fingerprint_off() {
    fprintd-delete "${TEST_USER}" >/dev/null 2>&1
    rm -f /run/systemd/system/fprintd.service.d/vmlab-vault.conf "${WORK}/vmlab-fprintd-reader.cil"
    systemctl daemon-reload
    systemctl stop fprintd.service 2>/dev/null
    if semodule -l 2>/dev/null | grep -qx vmlab-fprintd-reader; then semodule -r vmlab-fprintd-reader; fi
    rm -f "${READER}"
}
if [[ "${TPM}" == yes ]] && command -v fprintd-enroll >/dev/null && command -v semodule >/dev/null; then
    printf '(allow fprintd_t fprintd_var_lib_t (sock_file (create write unlink getattr setattr)))\n' >"${WORK}/vmlab-fprintd-reader.cil"
    semodule -i "${WORK}/vmlab-fprintd-reader.cil"
    mkdir -p /run/systemd/system/fprintd.service.d
    printf '[Service]\nEnvironment=FP_VIRTUAL_DEVICE_STORAGE=%s\nExecStart=\nExecStart=/usr/libexec/fprintd --no-timeout\n' "${READER}" \
        >/run/systemd/system/fprintd.service.d/vmlab-vault.conf
    systemctl daemon-reload
    systemctl restart fprintd.service
    timeout 120 fprintd-enroll -f right-index-finger "${TEST_USER}" >"${WORK}/enroll.log" 2>&1 &
    enrolling=$!
    if feed_scans vmlab-vault 100 grep -q enroll-completed "${WORK}/enroll.log"; then
        wait "${enrolling}"
        ok "a finger is enrolled on the virtual reader"
        # The vault by fingerprint: the same polkit question (the approval slot is there already), put to GNOME's dialog and answered by
        # a touch of the reader, nothing typed: polkit's PAM stack asks fprintd, which identifies the finger
        lock_it
        user_bus systemd-run --user --unit vmlab-vault-finger --collect --quiet -p "StandardOutput=file:${ASK_OUT}" \
            -- /usr/libexec/amethystora-vault-cli open approval
        if feed_scans vmlab-vault 60 is_open; then
            ok "the vault opens with a touch of the fingerprint reader, nothing typed"
        else
            not_ok "the fingerprint did not open the vault" "$(tail -n 4 "${ASK_OUT}" 2>&1)" "$(journalctl -b -u fprintd --no-pager -o cat | tail -n 6)"
        fi
        # Another finger is not the finger: the question stays, and nothing opens
        lock_it
        user_systemctl stop vmlab-vault-finger.service >/dev/null 2>&1
        user_bus systemd-run --user --unit vmlab-vault-badfinger --collect --quiet -- /usr/libexec/amethystora-vault-cli open approval
        sleep 3
        reader_send "SCAN vmlab-someone-else" 2>/dev/null
        sleep 4
        if is_closed; then ok "another finger opens nothing"; else not_ok "another finger opened the vault"; fi
        user_systemctl stop vmlab-vault-badfinger.service >/dev/null 2>&1
    else
        kill "${enrolling}" 2>/dev/null
        skip "the virtual reader could not be driven in the lab (enrolling timed out); the fingerprint's own PAM stack is suite 84's"
    fi
    fingerprint_off
    rm -f "${ASK_OUT}"
else
    skip "no TPM, fprintd or SELinux tools to set up the virtual reader with"
fi
is_open || open_by_password

# --- 8. What closes it ---------------------------------------------------------------------------------------------------------
loginctl lock-session "${SESSION}"
if wait_for 30 is_closed; then ok "locking the screen closes the vault"; else not_ok "the vault stayed open with the screen locked"; fi
unlike "...and a program that asks gets no secret and no question is put" "$(sock '{"v":1,"op":"password-copy","id":"x"}')" '"ok":true'
loginctl unlock-session "${SESSION}"
sleep 2
if is_closed; then ok "unlocking does not open it again by itself"; else not_ok "the vault was open after the unlock"; fi
open_by_password
# A suspend, by the kernel's own test of one, which wakes it after a few seconds (the lab's VM has no sleep a timer could end)
if grep -qw devices /sys/power/pm_test 2>/dev/null; then
    user_bus gsettings set org.gnome.desktop.screensaver lock-enabled false
    like "while it is open the service holds a delay inhibitor, so that sleep waits for the wipe" "$(systemd-inhibit --list --no-pager 2>/dev/null | grep -i vault || true)" "delay"
    before="$(date +%s)"
    echo devices >/sys/power/pm_test
    systemctl suspend
    wait_for 180 sh -c "journalctl -b --since @${before} --no-pager -o cat | grep -q 'PM: suspend exit'"
    echo none >/sys/power/pm_test
    if wait_for 30 is_closed; then ok "a suspend without the screen locking closes the vault"; else not_ok "the vault stayed open through a suspend"; fi
    user_bus gsettings reset org.gnome.desktop.screensaver lock-enabled
else
    skip "a suspend: this kernel has no test of its suspend (pm_test)"
fi
open_by_password
# Idle: after the setting's minutes without use. One minute is the shortest
av 90 "${PW[@]}" -- methods idle 1 >/dev/null
sleep 80
if is_closed; then ok "an open vault that nobody uses closes by itself after its minute"; else not_ok "the vault was still open eighty seconds after its last use"; fi
open_by_password
av 90 "${PW[@]}" -- methods idle 15 >/dev/null
# The service killed, however it dies: nothing of the key is left to open, and no core file is written
KILLED_AT="$(date '+%Y-%m-%d %H:%M:%S')"
systemctl kill -s ABRT "${SERVICE}"
sleep 4
if is_closed; then ok "a service killed by a signal leaves the vault closed"; else not_ok "the vault was open after its service was killed"; fi
same "its abort left no core dump (the limit is 0, and a key is not written anywhere)" "$(coredumpctl list --since "${KILLED_AT}" --no-legend --no-pager 2>/dev/null | grep -c amethystora-vault)" 0
open_by_password

# --- 9. Idle costs nothing -----------------------------------------------------------------------------------------------------
PID="$(service_pid)"
switches() { cat /proc/"${PID}"/task/*/status | awk '/ctxt_switches/ { n += $2 } END { print n }'; }
before="$(switches)"
sleep 30
after="$(switches)"
echo "# an open vault, idle for 30 s: $((after - before)) context switches (one deadline is its timer)"
if ((after - before <= 4)); then ok "an open, idle vault wakes at most for its deadline"; else not_ok "an open, idle vault woke $((after - before)) times in 30 s"; fi
rss="$(awk '/^VmRSS/ { print $2 }' "/proc/${PID}/status")"
echo "# the service's memory with the vault open: ${rss} kB"
if ((rss < 14000)); then ok "the service is small: ${rss} kB resident (its budget is 8 MB on a quiet machine, plus one scrypt while opening)"; else not_ok "the service holds ${rss} kB"; fi
lock_it
if wait_for 90 service_gone; then ok "closed and idle, the service exits within its minute"; else not_ok "the service was still there ninety seconds after the last vault closed"; fi
open_by_password

# --- 10. The authenticator -----------------------------------------------------------------------------------------------------
out="$(av 60 'The otpauth:// link: ' "otpauth://totp/Lab:lab@example.com?secret=${TOTP_KEY}&issuer=Lab"$'\r' -- auth add)"
AUTH="$(tester_session 'ame vault auth list' | awk -F'\t' '$2 ~ /Lab/ { print $1 }' | head -n 1)"
if [[ -n "${AUTH}" ]]; then
    ok "an authenticator account is added from its link"
    code_of() { av 90 "${PW[@]}" -- auth code Lab | grep -oE '^[0-9]{6}' | tail -n 1; }
    mine="$(code_of)"
    theirs="$(oathtool --totp -b "${TOTP_KEY}" 2>/dev/null)"
    if [[ -n "${mine}" && ( "${mine}" == "${theirs}" || "${mine}" == "$(oathtool --totp -b "${TOTP_KEY}" -N "$(date -d '-30 seconds' '+%F %T')" 2>/dev/null)" ) ]]; then
        ok "its code is oathtool's, for the same key and the same time"
    else
        not_ok "the code (${mine:-none}) is not oathtool's (${theirs:-no oathtool})"
    fi
    out="$(av 90 "${PW[@]}" -- auth qr Lab)"
    like "the QR code to scan into another app asks first, saying who can make codes" "${out}" "Anyone who sees it can make your codes"
    like "...and shows the key beside it, in groups, for an app that cannot scan" "$(tr -d ' \n\r' <<<"${out}")" "${TOTP_KEY}"
else
    not_ok "the account was not added" "$(tail -n 6 <<<"${out}")"
fi
skip "a QR code read from a picture, from the screen and from a camera: the window's file chooser, the screenshot portal and a virtual webcam are not in the lab (rqrr against qrencode is cargo's)"

# --- 11. SSH ---------------------------------------------------------------------------------------------------------------------
out="$(av 60 -- key new lab-ssh --kind=ssh)"
PUBLIC="$(grep -E '^ssh-ed25519 ' <<<"${out}" | head -n 1 | tr -d '\r')"
if [[ -n "${PUBLIC}" ]]; then
    ok "an SSH key is made inside the vault, and its public half is printed"
    systemctl start sshd.service
    install -d -m 0700 -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.ssh"
    printf '%s\n' "${PUBLIC}" >"${HOME_DIR}/.ssh/authorized_keys"
    chown "${TEST_USER}:${TEST_USER}" "${HOME_DIR}/.ssh/authorized_keys"
    chmod 0600 "${HOME_DIR}/.ssh/authorized_keys"
    restorecon -R "${HOME_DIR}/.ssh" 2>/dev/null
    AGENT=/run/amethystora/vault-ssh.sock
    LISTING="$(as_tester env SSH_AUTH_SOCK="${AGENT}" ssh-add -L 2>&1)"
    like "the agent lists the public key" "${LISTING}" "$(cut -d' ' -f2 <<<"${PUBLIC}")"
    unlike "...and never a private one" "${LISTING}" "PRIVATE"
    SSH=(ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o PasswordAuthentication=no -o LogLevel=ERROR -o ConnectTimeout=20 "${TEST_USER}@127.0.0.1" true)
    printf '#!/usr/bin/bash\nexport SSH_AUTH_SOCK=%s\nexec %s\n' "${AGENT}" "${SSH[*]}" >"${WORK}/ssh-login.sh"
    chmod a+rx "${WORK}/ssh-login.sh"
    # ssh asks for a signature, and the prompt names it. The text agent is registered for the terminal's shell, which runs ssh
    out="$(terminal 120 "${PW[@]}" -- 'pkttyagent --process $$$$ & sleep 1; bash '"${WORK}"'/ssh-login.sh; s=$$?; kill %1; exit $$s')"
    rc=$?
    if ((rc == 0)); then ok "ssh logs in to a real sshd with the key that only the vault holds"; else not_ok "ssh did not log in with the vault's key (${rc})" "$(tail -n 8 <<<"${out}")"; fi
    like "...and the prompt names ssh" "${out}" "A program calling itself ssh"
    lock_it
    like "a closed vault gives an empty list" "$(as_tester env SSH_AUTH_SOCK="${AGENT}" ssh-add -L 2>&1)" "The agent has no identities"
    if as_tester env SSH_AUTH_SOCK="${AGENT}" "${SSH[@]}" 2>/dev/null; then not_ok "ssh logged in with the vault closed"; else ok "a closed vault signs nothing: ssh is refused"; fi
    open_by_password
    systemctl stop sshd.service
    rm -f "${HOME_DIR}/.ssh/authorized_keys"
else
    not_ok "no SSH key was made" "$(tail -n 6 <<<"${out}")"
fi
# The agent's environment in a login: a fresh account's manager, which reads environment.d as it starts
useradd -m vmlabssh 2>/dev/null
if runuser -l vmlabssh -c 'ame vault ssh on' >/dev/null 2>&1; then
    loginctl enable-linger vmlabssh
    if systemctl start "user@$(id -u vmlabssh).service" 2>/dev/null; then
        sleep 3
        SSH_ENV="$(runuser -u vmlabssh -- env XDG_RUNTIME_DIR="/run/user/$(id -u vmlabssh)" systemctl --user show-environment 2>/dev/null | grep '^SSH_AUTH_SOCK=')"
        same "a new login's manager points SSH_AUTH_SOCK at the vault's agent, not gcr's or the keyring's" "${SSH_ENV}" "SSH_AUTH_SOCK=/run/amethystora/vault-ssh.sock"
        runuser -l vmlabssh -c 'ame vault ssh off' >/dev/null 2>&1
    else
        skip "a user manager for a second account could not be started to read its environment"
    fi
    loginctl disable-linger vmlabssh
else
    skip "ame vault ssh on needs a vault on the account (a second one has none)"
fi

# --- 12. Protected files and a command that uses one ---------------------------------------------------------------------------------
as_tester sh -c "mkdir -p ~/.aws && printf '[default]\naws_access_key_id = AKIAVMLAB\naws_secret_access_key = %s\n' '${LAB_SECRET}' >~/.aws/credentials"
CREDENTIALS="${HOME_DIR}/.aws/credentials"
out="$(av 90 'type: yes' $'yes\r' "${PW[@]}" -- protect "${CREDENTIALS}")"
if [[ ! -e "${CREDENTIALS}" ]]; then
    ok "a protected file is out of the home"
    like "...and says first what it cannot do" "${out}" "Old copies survive this"
    # A tool reaches it through ame vault run: the file is there for that command alone, and sealed again after it
    out="$(av 120 "${PW[@]}" -- run "${CREDENTIALS}" -- sh -c "'cat ~/.aws/credentials; echo more >>~/.aws/credentials'")"
    like "a command run through the vault finds the file at its path" "${out}" "AKIAVMLAB"
    like "...and the vault says what its other programs can reach while it runs" "${out}" "other programs of this account can read"
    if [[ ! -e "${CREDENTIALS}" ]]; then ok "...and afterwards it is out of the home again"; else not_ok "the file stayed in the home after the command"; fi
    out="$(av 120 "${PW[@]}" -- run "${CREDENTIALS}" -- cat "${CREDENTIALS}")"
    like "what the command wrote is sealed, and the next command reads it" "${out}" "more"
    # While the command runs, other programs of the account can reach what it holds: the manual says so, and here it is looked at
    terminal 120 "${PW[@]}" -- 'pkttyagent --process $$$$ & sleep 1; ame vault run '"${CREDENTIALS}"' -- sleep 25; kill %1' >/dev/null 2>&1 &
    runner=$!
    sleep 14
    holder="$(pgrep -u "${TEST_USER}" -n -x sleep)"
    if [[ -n "${holder}" ]]; then
        seen="$(as_tester cat "/proc/${holder}/root${CREDENTIALS}" 2>&1)"
        like "while it runs, another program of the account reads the file through /proc/PID/root, as the manual says" "${seen}" "AKIAVMLAB"
    else
        skip "the command inside the namespace was not found, so /proc/PID/root was not tried"
    fi
    wait "${runner}" 2>/dev/null
    av 90 "${PW[@]}" -- run "${CREDENTIALS}" -- sudo -n true >/dev/null
    if (($? != 0)); then ok "there is no sudo inside a run"; else not_ok "sudo worked inside a run"; fi
    av 90 "${PW[@]}" -- unprotect "${CREDENTIALS}" >/dev/null
    as_tester rm -rf "${HOME_DIR}/.aws"
else
    not_ok "the file stayed in the home" "$(tail -n 8 <<<"${out}")"
fi

# --- 13. The machine's root, certificates and a signed PDF -----------------------------------------------------------------------------
# A session over SSH is let in by nothing: a password of an open vault and a certificate both say no, however the vault is open
systemctl start sshd.service
install -d -m 0700 -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.ssh"
rm -f "${WORK}/login" "${WORK}/login.pub"
ssh-keygen -q -t ed25519 -N '' -f "${WORK}/login"
cat "${WORK}/login.pub" >"${HOME_DIR}/.ssh/authorized_keys"
chown "${TEST_USER}:${TEST_USER}" "${HOME_DIR}/.ssh/authorized_keys"
chmod 0600 "${HOME_DIR}/.ssh/authorized_keys"
restorecon -R "${HOME_DIR}/.ssh" 2>/dev/null
over_ssh() { ssh -i "${WORK}/login" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o IdentitiesOnly=yes -o LogLevel=ERROR -o ConnectTimeout=20 "${TEST_USER}@127.0.0.1" "$@" </dev/null; }
if out="$(over_ssh "ame vault password get lab-login" 2>&1)"; then
    not_ok "a session over SSH was given a password of an open vault" "${out}"
else
    ok "a session over SSH is refused a password, though the vault is open: polkit never asks it"
fi
unlike "...nor is it given a certificate" "$(over_ssh "ame vault cert new sshed pdf" 2>&1)" "CN="
rm -f "${HOME_DIR}/.ssh/authorized_keys"
systemctl stop sshd.service
out="$(av 150 "${PW[@]}" '?Password: ' "${PASSWORD}"$'\r' -- cert new lab-pdf pdf "--name='Lab Tester'")"
rc=$?
if ((rc == 0)) && [[ "${out}" == *"OU=Amethystora Vault"* ]]; then
    ok "a PDF certificate is issued, with Amethystora Vault in its subject"
else
    not_ok "no PDF certificate was issued (${rc})" "$(tail -n 8 <<<"${out}")"
fi
like "...after a question that names the certificate" "${out}" "issue the certificate ${LQ}lab-pdf${RQ}"
if [[ -d "${STATEDIR}/machine" ]]; then
    ok "the machine's root exists now, made for the first certificate that needed one"
    if as_tester ls "${STATEDIR}/machine" >/dev/null 2>&1; then not_ok "the account can list the machine's folder"; else ok "the account cannot read the machine's root folder"; fi
else
    not_ok "no root folder was made for the first certificate"
fi
ROOT_PEM="${WORK}/root.pem"
tester_session "ame vault root" >"${ROOT_PEM}" 2>"${WORK}/root.err"
like "the root names its machine by a number from its key" "$(openssl x509 -in "${ROOT_PEM}" -noout -subject 2>&1)" "CN=Amethystora OS Root CA"
HOST="$(hostname -s)"
if [[ -n "${HOST}" && "${HOST}" != localhost ]]; then
    unlike "...and never by the host's name" "$(openssl x509 -in "${ROOT_PEM}" -noout -subject 2>&1)" "${HOST}"
fi
remember ROOT_FP "$(openssl x509 -in "${ROOT_PEM}" -noout -fingerprint -sha256 2>/dev/null)"
if trust list 2>/dev/null | grep -qi 'Amethystora OS'; then
    not_ok "the machine's root is in the system's trust store"
else
    ok "the machine's root is not in the system's trust store"
fi
tester_session "ame vault cert ca" >"${WORK}/ca.pem" 2>/dev/null
same "the account's CA is under the machine's root" "$(openssl verify -CAfile "${ROOT_PEM}" -partial_chain "${WORK}/ca.pem" 2>&1 | tail -n 1)" "${WORK}/ca.pem: OK"
like "...and its subject is the account's" "$(openssl x509 -in "${WORK}/ca.pem" -noout -subject 2>&1)" "Code Signing CA"
skip "two accounts get different CAs under the same root: a second account with a seat of its own is not in the lab"
# A request for a public name by the local TLS profile is refused by the service before it gets as far as a chain
out="$(av 120 "${PW[@]}" -- cert new site tls www.example.com)"
if (($? != 0)) && [[ "${out}" != *"BEGIN CERTIFICATE"* ]]; then ok "a certificate for a public website's name is refused"; else not_ok "a TLS certificate for www.example.com was issued" "$(tail -n 4 <<<"${out}")"; fi
out="$(av 150 "${PW[@]}" '?Password: ' "${PASSWORD}"$'\r' -- cert new site tls printer.local 192.168.1.5)"
like "a certificate for a local name is issued" "${out}" "Amethystora OS"
# A real PDF, signed by pdfsig through the module with the key staying in the service
python3 - "${WORK}/lab.pdf" <<'PY'
import sys
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    None,
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
stream = b"BT /F1 18 Tf 20 50 Td (The Vault signs this) Tj ET"
objects[3] = b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream"
out, offsets = b"%PDF-1.4\n", []
for number, body in enumerate(objects, 1):
    offsets.append(len(out))
    out += b"%d 0 obj\n" % number + body + b"\nendobj\n"
xref = len(out)
out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1) + b"".join(b"%010d 00000 n \n" % o for o in offsets)
out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objects) + 1, xref)
open(sys.argv[1], "wb").write(out)
PY
chmod a+r "${WORK}/lab.pdf"
as_tester install -m 0644 "${WORK}/lab.pdf" "${HOME_DIR}/lab.pdf"
out="$(av 150 "${PW[@]}" -- pdf sign "${HOME_DIR}/lab.pdf")"
if [[ -s "${HOME_DIR}/lab (signed).pdf" ]]; then
    ok "ame vault pdf sign signs a real PDF, after a question, and says what it needs trusted"
    like "...the question is about signing with the certificate" "${out}" "asks to sign with ${LQ}lab-pdf${RQ}"
    like "...and the warning came before it" "${out}" "Only computers that trust this machine's root, or your CA, accept this"
    NSS="sql:${HOME_DIR}/.pki/nssdb"
    as_tester mkdir -p "${HOME_DIR}/.pki/nssdb"
    as_tester certutil -N --empty-password -d "${NSS}" >/dev/null 2>&1
    untrusted="$(as_tester pdfsig -nssdir "${NSS}" "${HOME_DIR}/lab (signed).pdf" 2>&1)"
    like "an account that has not trusted the root finds the signature valid and the signer unknown" "${untrusted}" "Signature is Valid"
    unlike "...and does not call the certificate trusted" "${untrusted}" "Certificate is Trusted"
    as_tester certutil -A -d "${NSS}" -n "Amethystora lab root" -t ",C," -i "${ROOT_PEM}" >/dev/null 2>&1
    trusted="$(as_tester pdfsig -nssdir "${NSS}" "${HOME_DIR}/lab (signed).pdf" 2>&1)"
    like "with the root trusted for signatures, pdfsig says the certificate is trusted" "${trusted}" "Certificate is Trusted"
    like "...and names the signer" "${trusted}" "Lab Tester"
    runuser -u vmlabother -- sh -c 'mkdir -p ~/.pki/nssdb && certutil -N --empty-password -d sql:$HOME/.pki/nssdb' >/dev/null 2>&1
    cp "${HOME_DIR}/lab (signed).pdf" /home/vmlabother/signed.pdf 2>/dev/null && chown vmlabother /home/vmlabother/signed.pdf
    unlike "another account that has not trusted it does not call it trusted either" \
        "$(runuser -u vmlabother -- pdfsig -nssdir sql:/home/vmlabother/.pki/nssdb /home/vmlabother/signed.pdf 2>&1)" "Certificate is Trusted"
    same "the private key was never in a file the account can see" \
        "$(as_tester grep -rlE 'BEGIN (RSA |EC |ENCRYPTED )?PRIVATE KEY' "${HOME_DIR}" /tmp "/run/user/${UID_T}" 2>/dev/null | grep -v vmlab | head -n 3)" ""
else
    not_ok "no signed copy was made" "$(tail -n 10 <<<"${out}")"
fi

# --- 14. A hidden vault ----------------------------------------------------------------------------------------------------------
HIDDEN='Hidden-vault-2026'
out="$(av 150 'Name for the vault: ' $'Quiet\r' 'To make it hidden, type: hidden ' $'hidden\r' 'New vault password: ' "${HIDDEN}"$'\r' 'Once more: ' "${HIDDEN}"$'\r' \
    "${PW[@]}" 'Type the recovery key back: ' '@recovery'$'\r' -- new)"
if (($? == 0)); then ok "a hidden vault is made, in a terminal, with its own recovery key"; else not_ok "the hidden vault was not made" "$(tail -n 10 <<<"${out}")"; fi
lock_it
STATUS_NOW="$(cli status)"
unlike "a hidden vault is in no list" "${STATUS_NOW}" "Quiet"
same "the list names only the vault that is listed" "$(awk -F'\t' '$1 == "vault"' <<<"${STATUS_NOW}" | wc -l)" 1
unlike "the journal says nothing of it" "$(journalctl -b --since "${SUITE_BEGAN}" --no-pager -o cat | grep -i 'quiet' | grep -i vault || true)" "Quiet"
av 90 'Vault password or recovery key: ' "${HIDDEN}"$'\r' -- open password >/dev/null
like "its password, typed on the closed screen, opens it" "$(cli status | awk -F'\t' '$1 == "vault" { print $3 }' | paste -sd ' ')" "open"
lock_it
# How long a right password and a wrong one take: the same, to within what the machine's noise allows (one scrypt either way)
timed() {
    local began
    began="$(date +%s%N)"
    sock "{\"v\":1,\"op\":\"unlock\",\"method\":\"password\",\"secret\":\"$(printf '%s' "$1" | base64)\"}" >/dev/null
    echo $((($(date +%s%N) - began) / 1000000))
}
right=0
wrong=0
for _ in 1 2 3; do
    right=$((right + $(timed "${HIDDEN}")))
    lock_it
    wrong=$((wrong + $(timed "not-the-password")))
    lock_it
done
echo "# opening by password, three tries each: the hidden vault's ${right} ms, a wrong one's ${wrong} ms"
if awk -v r="${right}" -v w="${wrong}" 'BEGIN { d = r > w ? r - w : w - r; exit !(d <= (r > w ? r : w) * 0.5) }'; then
    ok "a right password for a hidden vault takes about as long as a wrong one"
else
    not_ok "the hidden vault's password and a wrong one differ in time (${right} ms and ${wrong} ms over three tries)"
fi
same "the report names no hidden vault" "$(tester_session 'ame security status' 2>&1 | grep -ci quiet || true)" 0
same "...and what the machine sends names nothing of one" "$(tester_session 'ame security sends' 2>&1 | grep -ci quiet || true)" 0
av 90 'Vault password or recovery key: ' "${HIDDEN}"$'\r' -- open password >/dev/null
av 90 "${PW[@]}" -- remove-this >/dev/null
lock_it
open_by_password

# --- 15. The sealed backup, forgetting, and putting it back ---------------------------------------------------------------------------
if ame --show vault::backup >/dev/null 2>&1; then
    out="$(av 90 -- backup)"
    BACKUP="$(find "${HOME_DIR}/.local/share/amethystora/vault-backup" -name 'vault-*.avb' 2>/dev/null | sort | tail -n 1)"
    if [[ -n "${BACKUP}" ]]; then
        ok "ame vault backup writes a sealed file in the account's folder"
        same "...which holds nothing of what the vault keeps" "$(grep -c "${LAB_SECRET}" "${BACKUP}")" 0
        lock_it
        out="$(av 150 'type: FORGET' $'FORGET\r' "${PW[@]}" -- forget)"
        like "forget says what it removes" "${out}" "including any you cannot see"
        like "...and nothing is set up afterwards" "$(tester_session 'ame vault status')" "no vault"
        out="$(av 180 '?Vault password or recovery key: ' "${RECOVERY}"$'\r' '?New vault password: ' "${VAULT_PASSWORD}"$'\r' '?Once more: ' "${VAULT_PASSWORD}"$'\r' \
            "${PW[@]}" -- restore "'${BACKUP}'")"
        if is_open || open_by_password; then
            like "restore with the recovery key brings everything back" "$(tester_session 'ame vault password list')" "lab-login"
        else
            not_ok "the backup did not restore" "$(tail -n 8 <<<"${out}")"
        fi
    else
        not_ok "ame vault backup wrote no file" "$(tail -n 6 <<<"${out}")"
    fi
else
    not_ok "ame vault backup is no command: the sealed backup is not built"
fi

# --- 16. The network -------------------------------------------------------------------------------------------------------------
same "the network service is stopped until sync is connected" "$(systemctl is-active amethystora-vault-net.service)" inactive
if command -v podman >/dev/null && wait_for 60 sh -c 'getent hosts docker.io >/dev/null'; then
    IMAGE=docker.io/vaultwarden/server@sha256:4fe17f415f90087377d37c755e0ea3cf189dcc6794e49cdd2a9adbb4937d9e1f
    if podman pull -q "${IMAGE}" >/dev/null 2>&1; then
        podman rm -f vmlab-vaultwarden >/dev/null 2>&1
        podman run -d --name vmlab-vaultwarden -p 127.0.0.1:8080:8080 --tmpfs /data -e I_REALLY_WANT_VOLATILE_STORAGE=true \
            -e DOMAIN=http://127.0.0.1:8080 -e ROCKET_PORT=8080 -e SIGNUPS_ALLOWED=true -e SIGNUPS_VERIFY=false -e DISABLE_ICON_DOWNLOAD=true "${IMAGE}" >/dev/null 2>&1
        if wait_for 90 sh -c 'curl -sf http://127.0.0.1:8080/alive >/dev/null'; then ok "Vaultwarden, pinned by digest, answers on this machine"; else not_ok "Vaultwarden did not start"; fi
        # A registered account needs a web vault's key derivation in the browser, which the program never has; vault-net's own
        # Vaultwarden test registers one (cargo, apps/vault-net/tests/vaultwarden.rs). Here the real binaries meet the real server
        # with an account that does not exist: the network service starts only now, reaches only that host, and nothing is kept
        out="$(av 120 "${PW[@]}" 'Email address of the account: ' $'nobody@example.org\r' 'Type key or password: ' $'password\r' 'Master password' $'not-a-real-password\r' -- sync connect http://127.0.0.1:8080)"
        rc=$?
        if ((rc != 0)); then ok "connecting to a server with no such account fails, and says so"; else not_ok "sync connected to an account that does not exist"; fi
        echo "# the network service after a connection attempt: $(systemctl is-active amethystora-vault-net.service)"
        same "sync's address is not a row of what the machine sends: nothing is connected" "$(tester_session 'ame security sends' 2>&1 | grep -c vault-sync || true)" 0
        skip "every kind both ways, a conflict keeping both, a sealed item unreadable on the server, a stopped server and nothing failing: they need an account the server knows (cargo's vaultwarden.rs does them against this same image)"
        podman rm -f vmlab-vaultwarden >/dev/null 2>&1
    else
        skip "Vaultwarden could not be pulled here (no registry reachable from the lab): sync against it is cargo's, apps/vault-net/tests/vaultwarden.rs"
    fi
else
    skip "no podman, or no network, to start Vaultwarden in: sync against it is cargo's, apps/vault-net/tests/vaultwarden.rs"
fi
skip "Bitwarden's own server: by hand, with the maintainer's test account"

# --- 17. SELinux stayed out of the way ---------------------------------------------------------------------------------------------
same "SELinux is enforcing" "$(getenforce 2>/dev/null)" Enforcing
denials="$(journalctl --since "${AVC_SINCE}" _TRANSPORT=audit -o cat --no-pager 2>/dev/null | grep -E 'avc: +denied' | grep -v vmlab |
    grep -E 'amethystora_vault|amethystora-vault|vault-service|vault-cli|vault-work|vault-net|vault-ssh|ame vault' || true)"
same "no denial names the Vault's programs, sockets or files" "${denials}" ""
echo "# denials of any kind since the suite began: $(journalctl --since "${AVC_SINCE}" _TRANSPORT=audit -o cat --no-pager 2>/dev/null | grep -c 'avc: *denied')"

reboot_and_continue
fi

# ====================================================================================================================================
if ((SUITE_PHASE == 3)); then
# ====================================================================================================================================
# --- 18. After a restart ---------------------------------------------------------------------------------------------------------------
if wait_for 300 has_session; then ok "the account signs in again"; fi
same "the vault is closed after a restart" "$(cli status | awk -F'\t' '$1 == "vault" { print $3 }' | head -n 1)" closed
same "no service is running with nothing open" "$(systemctl is-active "${SERVICE}")" inactive
same "the sockets listen again" "$(systemctl is-active amethystora-vault.socket amethystora-vault-ssh.socket | paste -sd ' ')" "active active"
if open_by_password; then ok "the password still opens it"; else not_ok "the password no longer opens the vault"; fi
like "its items are where they were" "$(tester_session 'ame vault password list')" "lab-login"
like "...and so is the account's certificate" "$(tester_session 'ame vault cert list')" "lab-pdf"
same "the machine's root is the same one" "$(tester_session 'ame vault root' 2>/dev/null | openssl x509 -noout -fingerprint -sha256 2>/dev/null)" "${ROOT_FP:-unknown}"
lock_it
av 90 'Vault password or recovery key: ' "${RECOVERY}"$'\r' -- open password >/dev/null
if is_open; then ok "the recovery key still opens it"; else not_ok "the recovery key did not open the vault"; fi

# Everything the suite made is taken away
av 120 'type: FORGET' $'FORGET\r' "${PW[@]}" -- forget >/dev/null
like "the suite's own vault is forgotten" "$(tester_session 'ame vault status')" "no vault"
as_tester rm -rf "${HOME_DIR}/lab.pdf" "${HOME_DIR}/lab (signed).pdf" "${HOME_DIR}/.pki/nssdb" "${HOME_DIR}/.local/share/amethystora/vault-backup"
userdel -r vmlabother 2>/dev/null
userdel -r vmlabssh 2>/dev/null
rm -rf "${WORK}" "${STATE}"
fi

finish
