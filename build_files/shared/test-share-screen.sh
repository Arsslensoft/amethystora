#!/usr/bin/bash
# Another screen, and help from someone you trust (R7-26, R7-50, amethystora-share-screen) against stand-ins for GNOME's remote
# desktop tools, the firewall and the settings: what each mode sets, that a new code is made every time and never kept, that the
# port is opened in the home zone alone and closed again, that everything is put back, and that a firewall that refuses
# leaves nothing on. A real RDP client on a second computer is not here, and is in the real-machine checklist.
#
#   test-share-screen.sh   HELPER names another copy
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-share-screen}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
fail() { echo "test-share-screen: $*" >&2; exit 1; }

export HOME="${T}/home" PATH="${T}/bin:${PATH}" LOG="${T}/log" SETTINGS="${T}/settings"
export XDG_STATE_HOME="${HOME}/.local/state" XDG_DATA_HOME="${HOME}/.local/share"
mkdir -p "${T}/bin" "${HOME}"
: >"${LOG}"
printf "screen-share-mode='mirror-primary'\nview-only=true\nenable=false\nnegotiate-port=true\n" >"${SETTINGS}"
# gsettings as a file of key=value lines, grdctl, firewall-cmd, systemctl, systemd-run and the rest as logs
cat >"${T}/bin/gsettings" <<'EOS'
#!/usr/bin/bash
case "$1" in
    get) grep "^$3=" "${SETTINGS}" | cut -d= -f2- ;;
    set) grep -v "^$3=" "${SETTINGS}" >"${SETTINGS}.new" || true; echo "$3=$4" >>"${SETTINGS}.new"; mv "${SETTINGS}.new" "${SETTINGS}"; echo "gsettings set $3 $4" >>"${LOG}" ;;
esac
EOS
printf '#!/usr/bin/bash\necho "grdctl $*" >>"${LOG}"\n' >"${T}/bin/grdctl"
cat >"${T}/bin/firewall-cmd" <<'EOS'
#!/usr/bin/bash
echo "firewall-cmd $*" >>"${LOG}"
[[ -z "${FIREWALL_REFUSES:-}" || "$*" == *--remove-service* ]]
EOS
for name in systemctl systemd-run notify-send; do printf '#!/usr/bin/bash\necho "%s $*" >>"${LOG}"\n' "${name}" >"${T}/bin/${name}"; done
printf '#!/usr/bin/bash\necho 192.168.1.20\n' >"${T}/bin/hostname"
printf '#!/usr/bin/bash\nexit 0\n' >"${T}/bin/openssl"
# openssl as a stand-in writes the two files it was told to
cat >"${T}/bin/openssl" <<'EOS'
#!/usr/bin/bash
while [[ $# -gt 0 ]]; do case "$1" in -keyout) echo key >"$2" ;; -out) echo cert >"$2" ;; esac; shift; done
EOS
chmod +x "${T}"/bin/*

[[ "$("${HELPER}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "no arguments is a usage error"
[[ "$("${HELPER}" second-screen maybe >/dev/null 2>&1; echo $?)" == 64 ]] || fail "a word that is not on or off"
jq -e '.mode == "off" and .port == null' <<<"$("${HELPER}" status)" >/dev/null || fail "status when off"

# A second screen: extend, the code made now and shown, the port in the home zone only, the credentials and the settings
output="$("${HELPER}" second-screen on)"
grep -qx "gsettings set screen-share-mode extend" "${LOG}" || fail "not extended: $(cat "${LOG}")"
grep -qx "gsettings set view-only false" "${LOG}" || fail "a second screen takes input from the other side"
grep -qx "firewall-cmd --zone=amethystora-home --add-service=ms-wbt" "${LOG}" || fail "the port is not opened in the home zone: $(cat "${LOG}")"
grep -q -- '--permanent' "${LOG}" && fail "the port was opened for good"
grep -q "^firewall-cmd --add-" "${LOG}" && fail "a port was opened in the default zone"
code="$(sed -n 's/^Code: //p' <<<"${output}")"
[[ "${code}" =~ ^[0-9]{8}$ ]] || fail "the code is not eight digits: ${output}"
grep -qx "grdctl rdp set-credentials amethystora ${code}" "${LOG}" || fail "the code shown is not the one set"
grep -q 'set-auth-methods credentials' "${LOG}" || fail "only a code may let anyone in"
[[ "$(stat -c %a "${XDG_DATA_HOME}/amethystora/share-screen/rdp.key")" == 600 ]] || fail "the key is readable by others"
jq -e '.mode == "second-screen" and .port == 3389' <<<"$("${HELPER}" status)" >/dev/null || fail "status when on"
grep -rl "${code}" "${HOME}" 2>/dev/null && fail "the code was written to a file"
grep -q 'systemd-run' "${LOG}" && fail "a second screen started a watcher that ends it"

# Off: the credentials gone, RDP off, the port closed and the settings as they were
: >"${LOG}"
"${HELPER}" second-screen off >/dev/null
grep -q "^grdctl rdp clear-credentials" "${LOG}" && grep -q "^grdctl rdp disable" "${LOG}" || fail "off did not clear and disable"
grep -qx "firewall-cmd --zone=amethystora-home --remove-service=ms-wbt" "${LOG}" || fail "off did not close the port"
[[ "$(cat "${SETTINGS}" | sort | paste -sd' ')" == "enable=false negotiate-port=true screen-share-mode='mirror-primary' view-only=true" ]] || fail "settings were not put back: $(cat "${SETTINGS}")"
jq -e '.mode == "off"' <<<"$("${HELPER}" status)" >/dev/null || fail "status after off"

# Help: the screen as it is, view only, a watcher for the lock and the hour, and a new code each time
: >"${LOG}"
first="$("${HELPER}" help on | sed -n 's/^Code: //p')"
grep -qx "gsettings set screen-share-mode mirror-primary" "${LOG}" || fail "help does not mirror the screen"
grep -qx "gsettings set view-only true" "${LOG}" || fail "help can touch the screen"
grep -q "systemd-run --user --quiet --collect --unit=amethystora-share-screen-watch .* watch" "${LOG}" || fail "nothing ends a help when the screen locks"
"${HELPER}" help control on >/dev/null
[[ "$(sed -n 's/^view-only=//p' "${SETTINGS}")" == false ]] || fail "control on"
"${HELPER}" help control off >/dev/null
[[ "$(sed -n 's/^view-only=//p' "${SETTINGS}")" == true ]] || fail "control off"
"${HELPER}" help off >/dev/null
second="$("${HELPER}" help on | sed -n 's/^Code: //p')"
[[ "${first}" != "${second}" ]] || fail "the code was the same twice"
"${HELPER}" help off >/dev/null
"${HELPER}" help control on 2>/dev/null && fail "control was given to nobody"

# A firewall that refuses: nothing stays on
: >"${LOG}"
FIREWALL_REFUSES=1 "${HELPER}" help on >/dev/null 2>&1 && fail "it went on without its port"
grep -q "^grdctl rdp enable" "${LOG}" && fail "RDP was enabled with no port"
grep -q "^grdctl rdp clear-credentials" "${LOG}" || fail "the code stayed after a refusal"
jq -e '.mode == "off"' <<<"$("${HELPER}" status)" >/dev/null || fail "mode after a refusal"

# What the code may not do: leave the machine
grep -qE '\b(curl|wget|socat|ncat)\b' "${HELPER}" && fail "the helper reaches for the network"
grep -q -- '--permanent' "${HELPER}" && fail "the helper changes the firewall for good"
echo "test-share-screen: ok"
