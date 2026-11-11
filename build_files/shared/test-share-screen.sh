#!/usr/bin/bash
# Another screen, and help from someone you trust (R7-26, R7-50, amethystora-share-screen) against stand-ins for GNOME's remote
# desktop tools, Security's switch and the settings: that it refuses without the Security switch or with the firewall off and never
# touches the firewall itself, that the code goes in on standard input and is new each time and never written, what each mode sets,
# that every setting is put back (the person's own credentials are asked about first, since they cannot be kept), that a failure on
# the way undoes it, that a lock ends a help within a second, and that the login hook ends what a restart left.
#
#   test-share-screen.sh   HELPER names another copy
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-share-screen}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
fail() { echo "test-share-screen: $*" >&2; exit 1; }

export HOME="${T}/home" PATH="${T}/bin:${PATH}" LOG="${T}/log" SETTINGS="${T}/settings" XDG_STATE_HOME="${T}/home/.local/state" XDG_DATA_HOME="${T}/home/.local/share"
export AMETHYSTORA_HARDENING="${T}/bin/hardening" AMETHYSTORA_FIREWALLD_CONF="${T}/firewalld.conf" SWITCH="${T}/switch" CREDENTIALS="${T}/credentials"
mkdir -p "${T}/bin" "${HOME}"
: >"${LOG}"
echo on >"${SWITCH}"
echo DefaultZone=amethystora >"${AMETHYSTORA_FIREWALLD_CONF}"
reset_settings() {
    printf "screen-share-mode='mirror-primary'\nview-only=true\nenable=false\nnegotiate-port=true\nport=uint16 3389\ntls-cert=''\ntls-key=''\nauth-methods=['credentials']\n" >"${SETTINGS}"
}
reset_settings
SETTINGS_BEFORE="$(sort "${SETTINGS}" | paste -sd' ')"
cat >"${T}/bin/hardening" <<'EOS'
#!/usr/bin/bash
[[ "$1 $2" == "is-on remote-screen" && "$(cat "${SWITCH}")" == on ]]
EOS
cat >"${T}/bin/gsettings" <<'EOS'
#!/usr/bin/bash
case "$1" in
    get) grep "^$3=" "${SETTINGS}" | cut -d= -f2- ;;
    set) grep -v "^$3=" "${SETTINGS}" >"${SETTINGS}.new" || true; echo "$3=$4" >>"${SETTINGS}.new"; mv "${SETTINGS}.new" "${SETTINGS}"; echo "gsettings set $3 $4" >>"${LOG}" ;;
esac
EOS
# grdctl: set-credentials reads the code from standard input (and logs only its own arguments); status says whether a username is set
cat >"${T}/bin/grdctl" <<'EOS'
#!/usr/bin/bash
echo "grdctl $*" >>"${LOG}"
case "$2" in
    set-credentials) read -r code; echo "${code}" >"${CREDENTIALS}" ;;
    clear-credentials) rm -f "${CREDENTIALS}" ;;
    enable) [[ -z "${GRDCTL_ENABLE_FAILS:-}" ]] || exit 1 ;;
esac
if [[ "$1" == status ]]; then
    if [[ -e "${CREDENTIALS}" ]]; then echo "	Username: (hidden)"; else echo "	Username: (empty)"; fi
fi
EOS
cat >"${T}/bin/systemctl" <<'EOS'
#!/usr/bin/bash
echo "systemctl $*" >>"${LOG}"
[[ "$2" != is-active ]] || { echo "${UNIT_WAS:-inactive}"; [[ "${UNIT_WAS:-inactive}" == active ]]; }
EOS
for name in systemd-run notify-send; do printf '#!/usr/bin/bash\necho "%s $*" >>"${LOG}"\n' "${name}" >"${T}/bin/${name}"; done
printf '#!/usr/bin/bash\necho 192.168.1.20\n' >"${T}/bin/hostname"
cat >"${T}/bin/openssl" <<'EOS'
#!/usr/bin/bash
[[ "$1" != x509 ]] || { echo "sha256 Fingerprint=AB:CD:EF:01:23"; exit 0; }
while [[ $# -gt 0 ]]; do case "$1" in -keyout) echo key >"$2" ;; -out) echo cert >"$2" ;; esac; shift; done
EOS
# A bus that says the screen locked, and then stays quiet as the real one does until the unlock: the watcher must not wait for more
cat >"${T}/bin/gdbus" <<'EOS'
#!/usr/bin/bash
echo "/org/gnome/ScreenSaver: org.gnome.ScreenSaver.ActiveChanged (false,)"
echo "/org/gnome/ScreenSaver: org.gnome.ScreenSaver.ActiveChanged (true,)"
sleep 20
EOS
cat >"${T}/bin/timeout" <<'EOS'
#!/usr/bin/bash
echo "timeout $1" >>"${LOG}"
exec /usr/bin/timeout "$@"
EOS
chmod +x "${T}"/bin/*

[[ "$("${HELPER}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "no arguments is a usage error"
[[ "$("${HELPER}" second-screen maybe >/dev/null 2>&1; echo $?)" == 64 ]] || fail "a word that is not on or off"
jq -e '.mode == "off" and .port == null and .own_credentials == false' <<<"$("${HELPER}" status)" >/dev/null || fail "status when off"

# Security's switch and the firewall come first: nothing is changed, nothing is asked of the firewall, and it says where to turn it on
echo off >"${SWITCH}"
: >"${LOG}"
set +e
output="$("${HELPER}" help on 2>&1)"
code=$?
set -e
[[ "${code}" == 3 ]] || fail "it went on with the Security switch off (${code})"
grep -q 'ame security remote-screen on' <<<"${output}" && grep -q 'amethystora-security settings/network' <<<"${output}" || fail "it does not say where to turn it on: ${output}"
[[ ! -s "${LOG}" ]] || fail "something was changed while the switch was off: $(cat "${LOG}")"
echo on >"${SWITCH}"
echo DefaultZone=FedoraWorkstation >"${AMETHYSTORA_FIREWALLD_CONF}"
set +e
"${HELPER}" second-screen on >/dev/null 2>&1
code=$?
set -e
[[ "${code}" == 3 && ! -s "${LOG}" ]] || fail "it went on with the firewall off (${code})"
echo DefaultZone=amethystora-away >"${AMETHYSTORA_FIREWALLD_CONF}"
"${HELPER}" check || fail "the away zone is an Amethystora one"
echo DefaultZone=amethystora >"${AMETHYSTORA_FIREWALLD_CONF}"

# A second screen: extend, input allowed, the code on standard input and shown, the service started for this session only
: >"${LOG}"
output="$("${HELPER}" second-screen on)"
grep -qx "gsettings set screen-share-mode extend" "${LOG}" || fail "not extended: $(cat "${LOG}")"
grep -qx "gsettings set view-only false" "${LOG}" || fail "a second screen takes input from the other side"
code="$(sed -n 's/^Code: //p' <<<"${output}")"
[[ "${code}" =~ ^[0-9a-hjkmnp-tv-z]{12}$ ]] || fail "the code is not twelve characters of the alphabet: ${output}"
grep -q '^Certificate (SHA-256).*: AB:CD:EF:01:23$' <<<"${output}" || fail "the certificate's fingerprint is not shown beside the code: ${output}"
[[ "$(cat "${CREDENTIALS}")" == "${code}" ]] || fail "the code shown is not the one set"
grep -qx "grdctl rdp set-credentials amethystora" "${LOG}" || fail "the credentials were not set as the user alone: $(grep credentials "${LOG}")"
grep -q "${code}" "${LOG}" && fail "the code is on a command line"
grep -q 'set-auth-methods credentials' "${LOG}" || fail "only a code may let anyone in"
grep -q 'firewall-cmd' "${LOG}" && fail "the firewall was touched from here"
grep -qE "systemctl --user (enable|reenable)" "${LOG}" && fail "the remote desktop service was enabled for good"
grep -qx "systemctl --user restart gnome-remote-desktop.service" "${LOG}" || fail "the service was not started for this session"
grep -q "^systemd-run --user --quiet --collect --unit=amethystora-share-screen-watch .* watch second-screen" "${LOG}" || fail "nothing ends a second screen when the screen locks"
[[ "$(stat -c %a "${XDG_DATA_HOME}/amethystora/share-screen/rdp.key")" == 600 ]] || fail "the key is readable by others"
jq -e '.mode == "second-screen" and .port == 3389' <<<"$("${HELPER}" status)" >/dev/null || fail "status when on"
grep -rl "${code}" "${HOME}" 2>/dev/null && fail "the code was written to a file"

# Off: the credentials gone, the service stopped, every setting as it was (the port, the certificate, the methods too)
: >"${LOG}"
"${HELPER}" second-screen off >/dev/null
grep -q "^grdctl rdp clear-credentials" "${LOG}" && grep -q "^systemctl --user stop gnome-remote-desktop.service" "${LOG}" || fail "off did not clear and stop"
[[ "$(sort "${SETTINGS}" | paste -sd' ')" == "${SETTINGS_BEFORE}" ]] || fail "settings were not put back: $(cat "${SETTINGS}")"
jq -e '.mode == "off"' <<<"$("${HELPER}" status)" >/dev/null || fail "status after off"

# Help: view only, a watcher, a new code each time, control only while helping
reset_settings
first="$("${HELPER}" help on | sed -n 's/^Code: //p')"
grep -qx "view-only=true" "${SETTINGS}" && grep -qx "screen-share-mode=mirror-primary" "${SETTINGS}" || fail "help is not the screen, view only"
"${HELPER}" help control on >/dev/null
grep -qx "view-only=false" "${SETTINGS}" || fail "control on"
"${HELPER}" help control off >/dev/null
grep -qx "view-only=true" "${SETTINGS}" || fail "control off"
"${HELPER}" help off >/dev/null
second="$("${HELPER}" help on | sed -n 's/^Code: //p')"
[[ "${first}" != "${second}" ]] || fail "the code was the same twice"
"${HELPER}" help off >/dev/null
"${HELPER}" help control on 2>/dev/null && fail "control was given to nobody"

# The person's own credentials (GNOME Settings' Remote Desktop) cannot be kept: refused and unchanged until they agree
echo "their code" >"${CREDENTIALS}"
: >"${LOG}"
jq -e '.own_credentials == true' <<<"$("${HELPER}" status)" >/dev/null || fail "own credentials not seen"
set +e
"${HELPER}" help on >/dev/null 2>&1
code=$?
set -e
[[ "${code}" == 2 ]] || fail "their credentials were overwritten without asking (${code})"
grep -q 'gsettings set\|set-credentials' "${LOG}" && fail "something changed before they agreed"
"${HELPER}" help on overwrite >/dev/null || fail "it did not go on once they agreed"
"${HELPER}" help off >/dev/null
rm -f "${CREDENTIALS}"

# A service that was running before (their own Remote Desktop) runs again when this ends
: >"${LOG}"
UNIT_WAS=active "${HELPER}" help on >/dev/null
: >"${LOG}"
"${HELPER}" help off >/dev/null
grep -qx "systemctl --user start gnome-remote-desktop.service" "${LOG}" || fail "the service that was running before was not started again"

# A failure on the way undoes it: nothing stays on, the settings are back, and the code is gone
reset_settings
: >"${LOG}"
GRDCTL_ENABLE_FAILS=1 "${HELPER}" help on >/dev/null 2>&1 && fail "it went on with a failing grdctl"
[[ ! -e "${CREDENTIALS}" ]] || fail "the code stayed after a failure"
[[ "$(sort "${SETTINGS}" | paste -sd' ')" == "${SETTINGS_BEFORE}" ]] || fail "settings stayed changed after a failure: $(cat "${SETTINGS}")"
jq -e '.mode == "off"' <<<"$("${HELPER}" status)" >/dev/null || fail "mode after a failure"

# A lock ends a help within a second, though the bus has nothing more to say until the unlock; the hour (two for a second screen) is the limit
: >"${LOG}"
"${HELPER}" help on >/dev/null
: >"${LOG}"
start="$(date +%s%N)"
"${HELPER}" watch help
took=$((($(date +%s%N) - start) / 1000000))
((took < 1500)) || fail "the lock took ${took} ms to end the share"
grep -q "^grdctl rdp clear-credentials" "${LOG}" && grep -q "^notify-send" "${LOG}" || fail "the watcher did not end the share and say so"
grep -qx "timeout 3600" "${LOG}" || fail "a help is limited to an hour"
jq -e '.mode == "off"' <<<"$("${HELPER}" status)" >/dev/null || fail "mode after a lock"
"${HELPER}" second-screen on >/dev/null
: >"${LOG}"
"${HELPER}" watch second-screen
grep -qx "timeout 7200" "${LOG}" || fail "a second screen is limited to two hours"

# What a restart left (the login hook runs `end`): the share is over, everything put back
"${HELPER}" help on >/dev/null
reset_settings_marker="$(cat "${XDG_STATE_HOME}/amethystora/share-screen/before" | wc -l)"
((reset_settings_marker > 0)) || fail "nothing was remembered to put back"
"${HELPER}" end
jq -e '.mode == "off"' <<<"$("${HELPER}" status)" >/dev/null || fail "end did not end it"
"${HELPER}" end

# What the code may not do: leave the machine, change the firewall, or put the code on a command line
grep -qE '\b(curl|wget|socat|ncat|firewall-cmd)\b' <(grep -v '^ *#' "${HELPER}") && fail "the helper reaches for the network or the firewall"
grep -q 'set-credentials "${USER_NAME}" "' "${HELPER}" && fail "the code is on grdctl's command line"
echo "test-share-screen: ok"
