#!/usr/bin/bash
# The phone link (plans/PLAN-amethystora-connect.md) on a machine with no phone: GSConnect is gone, the program is
# there and off, `ame desktop phone on` starts it and nothing else, it listens where the firewall lets phones in and
# where the security watcher expects it, and `off` takes every listener away again. A pairing needs a phone, which the
# lab has not: apps/connect/tests holds the interop test against Fedora's KDE Connect daemon.
# vmlab-paths: apps/connect/* apps/phone/* @ui/*connect* @sf/*just/desktop.just @sf/usr/lib/firewalld/* @sf/*Connect.gschema.xml
source "$(dirname "$0")/lib.sh"

UNIT=amethystora-connect.service
NAME=org.amethystora.Connect
PORT=1716
HOME_DIR="/var/home/${TEST_USER}"

# --- The image -------------------------------------------------------------------------------------------------------
left="$(rpm -q --qf '%{NAME}\n' nautilus-gsconnect gnome-shell-extension-gsconnect 2>/dev/null | grep -v 'is not installed')"
if [[ -n "${left}" ]]; then
    not_ok "GSConnect is still installed" "${left}"
else
    ok "GSConnect and its Files extension are not installed"
fi
for file in /usr/libexec/amethystora-connect /usr/bin/amethystora-phone /usr/lib/systemd/user/${UNIT}; do
    check "${file} is in the image" test -e "${file}"
done
# The clipboard is shared only when chosen, one way at a time, and both ways start off
keys="$(GSETTINGS_BACKEND=memory gsettings list-keys org.amethystora.Connect 2>&1 | sort | paste -sd ' ')"
if [[ "${keys}" == "clicker clipboard-receive clipboard-send codes computers device-name music photo-folder" ]]; then
    ok "the phone link's settings are the name phones see, the two clipboard switches, where pictures go, other computers, music and the clicker"
else
    not_ok "the phone link has other settings than the name, the two clipboard switches, the photo folder and computers" "${keys}"
fi
for key in clipboard-send clipboard-receive; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect "${key}")" == false ]] &&
        ok "${key} is off as the image ships" || not_ok "${key} is not off as the image ships"
done

# Phones reach the machine only on the networks named home
if firewall-cmd --permanent --zone=amethystora --query-service=kdeconnect >/dev/null 2>&1; then
    not_ok "the default zone is open to phones"
else
    ok "the default zone is closed to phones"
fi
check "the home zone is open to phones" firewall-cmd --permanent --zone=amethystora-home --query-service=kdeconnect

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

listening() { ss -Hltun "sport = :${PORT}" 2>/dev/null | grep -q .; }
owned() { user_bus busctl --user list --no-pager 2>/dev/null | grep -q "^${NAME} "; }
active() { [[ "$(user_systemctl is-active "${UNIT}" 2>/dev/null)" == active ]]; }
inactive() { ! active; }
silent() { ! listening; }
call() { user_bus busctl --user "$@" 2>&1; }

# --- Off costs nothing ---------------------------------------------------------------------------------------------------
if [[ "$(user_systemctl is-enabled "${UNIT}" 2>/dev/null)" != enabled ]] && ! active && ! owned && ! listening; then
    ok "off as the image ships: not enabled, not running, no bus name, nothing listening on ${PORT}"
else
    not_ok "the phone link is not off as the image ships" \
        "enabled: $(user_systemctl is-enabled "${UNIT}" 2>&1)" "active: $(user_systemctl is-active "${UNIT}" 2>&1)" \
        "$(ss -Htulnp "sport = :${PORT}" 2>&1)"
fi

# --- On ------------------------------------------------------------------------------------------------------------------
out="$(in_session "ame desktop phone on" 2>&1)"
if wait_for 30 active; then
    ok "ame desktop phone on starts the service"
else
    not_ok "ame desktop phone on did not start the service" "${out}" "$(journalctl -b _SYSTEMD_USER_UNIT=${UNIT} --no-pager -o cat | tail -n 20)"
fi
[[ "$(user_systemctl is-enabled "${UNIT}" 2>/dev/null)" == enabled ]] && ok "...and enables it for the next login" ||
    not_ok "...but does not enable it for the next login"
if wait_for 30 owned; then ok "the program owns ${NAME} on the session bus"; else not_ok "nothing owns ${NAME}"; fi
if wait_for 30 listening; then
    ok "it listens on ${PORT}"
else
    not_ok "nothing listens on ${PORT}" "$(ss -Htulnp 2>&1 | head -n 20)"
fi

# The security watcher expects exactly this listener: its name, cut to fifteen characters by the kernel, and port
# against the image's own allowances (allow.d/listeners, one glob a line)
program="$(ss -Htlnp "sport = :${PORT}" 2>/dev/null | sed -nE 's/.*users:\(\("([^"]+)".*/\1/p' | head -n 1)"
allowed=""
while read -r glob; do
    [[ -z "${glob}" || "${glob}" == \#* ]] && continue
    # shellcheck disable=SC2053
    [[ "${program} ${PORT}" == ${glob} ]] && allowed="${glob}"
done </usr/share/amethystora/allow.d/listeners
if [[ -n "${program}" && -n "${allowed}" ]]; then
    ok "the listener (${program}) is one the image allows the watcher to record, not say"
else
    not_ok "the watcher would say the phone link's listener" "program: '${program}'" "$(cat /usr/share/amethystora/allow.d/listeners)"
fi

# The program answers the Phone app and the shell alone (what it is told is a pairing, a text, a file), so what asks
# from here, busctl, is refused: its methods are there to see, and it does not do them for a stranger
methods="$(call introspect "${NAME}" /org/amethystora/Connect --no-pager)"
if grep -q "GetDevices" <<<"${methods}" && grep -q "PushClipboard" <<<"${methods}"; then
    ok "the program serves org.amethystora.Connect on the session bus"
else
    not_ok "the program does not serve its interface" "${methods}"
fi
refused="$(call call "${NAME}" /org/amethystora/Connect org.amethystora.Connect GetDevices)"
if [[ "${refused}" == *"caller not allowed"* ]]; then
    ok "GetDevices is refused to a program that is not the Phone app or the shell"
else
    not_ok "GetDevices answered a program that is not the Phone app or the shell" "${refused}"
fi
for method in Announce PushClipboard; do
    answer="$(call call "${NAME}" /org/amethystora/Connect org.amethystora.Connect "${method}" $([[ ${method} == PushClipboard ]] && echo "s secret"))"
    [[ "${answer}" == *"caller not allowed"* ]] && ok "${method} is refused too" || not_ok "${method} was answered" "${answer}"
done
name="$(call get-property "${NAME}" /org/amethystora/Connect org.amethystora.Connect Name)"
[[ "${name}" == 's ""' ]] && ok "and its properties read as empty to a stranger" || not_ok "a stranger could read a property" "${name}"

# Its certificate and key are the account's alone
config="${HOME_DIR}/.config/amethystora/connect"
if [[ -d "${config}" && "$(stat -c %a "${config}")" == 700 ]] &&
    [[ -z "$(find "${config}" -type f ! -perm 0600)" ]]; then
    ok "its certificate and pairings are in a folder only the account reads"
else
    not_ok "its folder or files are readable by others" "$(ls -la "${config}" 2>&1)"
fi

# Nothing in the journal of the unit says an error
errors="$(journalctl -b "_SYSTEMD_USER_UNIT=${UNIT}" --no-pager -o cat -p err 2>/dev/null | head -n 10)"
if [[ -z "${errors}" ]]; then ok "the service logged no error"; else not_ok "the service logged errors" "${errors}"; fi

# --- Off again -----------------------------------------------------------------------------------------------------------
out="$(in_session "ame desktop phone off" 2>&1)"
if wait_for 30 inactive; then
    ok "ame desktop phone off stops the service"
else
    not_ok "ame desktop phone off left the service running" "${out}"
fi
if wait_for 30 silent; then ok "...and nothing listens on ${PORT}"; else not_ok "...but something still listens on ${PORT}"; fi
[[ "$(user_systemctl is-enabled "${UNIT}" 2>/dev/null)" != enabled ]] && ok "...and the next login does not start it" ||
    not_ok "...but the next login still starts it"
if owned; then not_ok "...but the bus name is still owned"; else ok "...and the bus name is free"; fi

finish
