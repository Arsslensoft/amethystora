#!/usr/bin/bash
# What the machine sends by itself (plans/done/PLAN-what-leaves.md): the list `ame security sends` gives and the
# report's group of the same words, each new switch once each way as it works (Fedora's count masked and unmasked,
# NetworkManager's check really off and on again whoever last set it, the firmware catalogue's timer, the account's
# location), and then the machine left idle with resolved's monitor listening: every name it looks up is one the
# list names. That last check is the one that finds the next thing nobody listed.
# vmlab-paths: @sf/usr/libexec/amethystora-sends @sf/usr/libexec/amethystora-hardening @ui/modules/tools/reach*
source "$(dirname "$0")/lib.sh"

H=/usr/libexec/amethystora-hardening
SENDS=/usr/libexec/amethystora-sends
NM=(org.freedesktop.NetworkManager /org/freedesktop/NetworkManager org.freedesktop.NetworkManager ConnectivityCheckEnabled)
IDLE_SECONDS="${WHAT_LEAVES_IDLE:-600}"
NAMES=/run/vmlab-what-leaves-names
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
wait_for 120 pgrep -u "${TEST_USER}" -x gnome-shell

# --- The list ----------------------------------------------------------------------------------------------------

# As the account, which is how the report and ame run it
LIST="$(user_bus "${SENDS}" --json)"
jq -e 'length >= 6 and all(.[]; (.id | startswith("sends-")) and (.state | IN("on", "off")) and (.title | length > 0)
    and (.text | length > 0) and (.details | type == "array"))' <<<"${LIST}" >/dev/null &&
    ok "the list: a record for each, with its title, sentence and state" || not_ok "the list" "${LIST}"
for id in countme connectivity-check location auto-updates firmware-metadata clock homebrew-analytics; do
    jq -e --arg id "sends-${id}" 'any(.[]; .id == $id)' <<<"${LIST}" >/dev/null && ok "the list names ${id}" || not_ok "the list names ${id}"
done
same "Homebrew's analytics are off" "$(jq -r '.[] | select(.id == "sends-homebrew-analytics") | .state' <<<"${LIST}")" off
same "Fedora's count is on as the image ships it" "$(jq -r '.[] | select(.id == "sends-countme") | .state' <<<"${LIST}")" on
SENDS_TEXT="$(user_bus ame security sends)"
grep -q "What this machine sends by itself" <<<"${SENDS_TEXT}" && ok "ame security sends prints the list" || not_ok "ame security sends prints the list" "${SENDS_TEXT}"
REPORT="$(user_bus /usr/libexec/amethystora-security-status --json)"
jq -e 'any(.checks[]; .group == "sends" and .id == "sends-countme") and all(.checks[] | select(.group == "sends"); .accepted == null)' \
    <<<"${REPORT}" >/dev/null && ok "the report has the group" || not_ok "the report has the group"
# Off is information, never a thing to look at: nothing in the group is a check that needs attention
"${H}" is-on countme && "${H}" is-on connectivity-check && "${H}" is-on firmware-metadata && ok "the three switches are on as shipped" ||
    not_ok "the three switches are on as shipped"

state_of() { jq -r --arg id "sends-$1" '.[] | select(.id == $id) | .state' < <(user_bus "${SENDS}" --json); }

# --- Fedora's count ----------------------------------------------------------------------------------------------

same "the count's timer is enabled" "$(systemctl is-enabled rpm-ostree-countme.timer)" enabled
"${H}" set countme off
same "countme off masks the timer" "$(systemctl is-enabled rpm-ostree-countme.timer 2>/dev/null)" masked
same "countme off masks the service" "$(systemctl is-enabled rpm-ostree-countme.service 2>/dev/null)" masked
systemctl start rpm-ostree-countme.service 2>/dev/null && not_ok "a masked count was started" || ok "a masked count cannot be started"
same "the list says it is off" "$(state_of countme)" off
grep -qxP 'systemd/system/rpm-ostree-countme.timer\tmask' <<<"$(AMETHYSTORA_HARDENING_ROOT="" "${H}" managed)" &&
    ok "managed names the mask" || not_ok "managed names the mask"
"${H}" set countme on
same "countme on unmasks the timer" "$(systemctl is-enabled rpm-ostree-countme.timer 2>/dev/null)" enabled
same "the list says it is on" "$(state_of countme)" on
[[ -L /etc/systemd/system/rpm-ostree-countme.timer ]] && not_ok "a mask is left in /etc" || ok "no mask is left in /etc"

# --- The connectivity check --------------------------------------------------------------------------------------

nm_check() { busctl get-property "${NM[@]}" | awk '{ print $2 }'; }
same "NetworkManager checks the connection as the image ships it" "$(nm_check)" true
"${H}" set connectivity-check off
same "off: the file is there" "$(grep -c '^enabled=false$' /etc/NetworkManager/conf.d/60-amethystora-connectivity.conf)" 1
same "off: NetworkManager does not check" "$(nm_check)" false
same "off: the list reads what NetworkManager does" "$(state_of connectivity-check)" off
"${H}" set connectivity-check on
same "on: the file is gone" "$([[ -e /etc/NetworkManager/conf.d/60-amethystora-connectivity.conf ]] && echo there || echo gone)" gone
same "on: NetworkManager checks again" "$(nm_check)" true
# GNOME Settings' switch sets the property over D-Bus, and NetworkManager keeps it apart from /etc and reads it
# last: the list reads what it does, and the command sets what it was asked even after GNOME's switch
busctl set-property "${NM[@]}" b false
same "GNOME's switch turns the check off" "$(nm_check)" false
same "the list reads GNOME's switch" "$(state_of connectivity-check)" off
"${H}" set connectivity-check on
same "on after GNOME's switch turned it off: NetworkManager checks" "$(nm_check)" true
"${H}" set connectivity-check off
busctl set-property "${NM[@]}" b true
same "GNOME's switch turns it on over the file" "$(nm_check)" true
"${H}" set connectivity-check off
same "off after GNOME's switch turned it on: NetworkManager does not check" "$(nm_check)" false
"${H}" set connectivity-check on
same "back to the image's" "$(nm_check)" true

# --- The firmware catalogue ----------------------------------------------------------------------------------------

if systemctl cat fwupd-refresh.timer >/dev/null 2>&1; then
    "${H}" set firmware-metadata off
    same "firmware-metadata off masks the timer" "$(systemctl is-enabled fwupd-refresh.timer 2>/dev/null)" masked
    same "the timer does not run" "$(systemctl is-active fwupd-refresh.timer 2>/dev/null)" inactive
    same "the list says it is off" "$(state_of firmware-metadata)" off
    "${H}" set firmware-metadata on
    same "firmware-metadata on enables the timer" "$(systemctl is-enabled fwupd-refresh.timer 2>/dev/null)" enabled
    same "the timer runs again" "$(systemctl is-active fwupd-refresh.timer 2>/dev/null)" active
else
    skip "no fwupd-refresh.timer on this image"
fi

# --- Location, the account's --------------------------------------------------------------------------------------

user_bus ame security location off >/dev/null
same "location off" "$(user_bus gsettings get org.gnome.system.location enabled)" false
same "the list says location is off" "$(state_of location)" off
user_bus ame security location on >/dev/null
same "location on" "$(user_bus gsettings get org.gnome.system.location enabled)" true
same "the list says location is on" "$(state_of location)" on

# --- Idle ----------------------------------------------------------------------------------------------------------

# Every name resolved while the machine is left alone, from resolved's own monitor, is one of the list's hosts or
# the machine's own network. Fedora's count is asked for as well, to see its request in the monitor.
rm -f "${NAMES}"
(resolvectl monitor --json=short --no-pager 2>/dev/null |
    jq --unbuffered -r 'select(type == "object") | .question[]?.name? // empty | strings' >>"${NAMES}") &
MONITOR=$!
sleep 3
systemctl start --no-block rpm-ostree-countme.service
sleep "${IDLE_SECONDS}"
kill "${MONITOR}" 2>/dev/null
pkill -f 'resolvectl monitor' 2>/dev/null
sort -u "${NAMES}" | sed 's/\.$//' >"${NAMES}.sorted"
ALLOWED='(^|\.)(fedoraproject\.org|ghcr\.io|githubusercontent\.com|github\.com|flathub\.org|fwupd\.org|beacondb\.net|ntp\.org|cloudflare\.com|clamav\.net|brew\.sh|local|lan|home\.arpa|localhost|invalid|arpa)$|^_|^$'
UNLISTED="$(grep -vE "${ALLOWED}" "${NAMES}.sorted" || true)"
if [[ -z "${UNLISTED}" ]]; then
    ok "every name looked up while idle is one the list names ($(wc -l <"${NAMES}.sorted") names)"
else
    not_ok "names looked up while idle that the list does not name" "${UNLISTED}"
fi
if grep -q 'fedoraproject.org' "${NAMES}.sorted"; then
    ok "Fedora's count, started by hand, asked Fedora's mirror list"
else
    skip "the count asked nothing: the machine was counted this week already, or has no network"
fi
# Masked, it is silent
"${H}" set countme off
: >"${NAMES}"
(resolvectl monitor --json=short --no-pager 2>/dev/null |
    jq --unbuffered -r 'select(type == "object") | .question[]?.name? // empty | strings' >>"${NAMES}") &
MONITOR=$!
sleep 3
systemctl start rpm-ostree-countme.service 2>/dev/null
sleep 20
kill "${MONITOR}" 2>/dev/null
pkill -f 'resolvectl monitor' 2>/dev/null
grep -q 'mirrors.fedoraproject.org' "${NAMES}" && not_ok "Fedora's mirror list was asked with the count off" || ok "with the count off, Fedora's mirror list is not asked"
"${H}" set countme on

finish
