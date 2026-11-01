#!/usr/bin/bash
# More from the phone (plans/beyond/spec-phone-files-first.md, section 9, suite 24): the phone's signal and battery in
# the top bar's capsule, pictures in Pictures/Phone and the photo-folder setting, and a shared link going to the shell
# instead of a notification. The phone is phonepeer.py, a stand-in that speaks the protocol from a network namespace
# whose link the firewall counts as home, paired with the program by its pin.
#
# 1 and 2 (the crate's tests and the interop run against Fedora's kdeconnectd) are the build's and apps/connect/tests'.
# vmlab-paths: @ui/*connect* apps/connect/* apps/phone/* @sf/*themed/shell-connect* @sf/*just/desktop.just @sf/*systemd/user/amethystora-connect.service
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
UNIT=amethystora-connect.service
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-24
PEER="${WORK}/phone"
NS=vmphone
HOST_IP=10.77.0.1
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
gs() { in_session "gsettings $*" 2>/dev/null; }
# within SECONDS COMMAND...: until the command succeeds, tried every tenth of a second
within() {
    local deadline=$(($(date +%s%N) + $1 * 1000000000))
    shift
    until "$@" >/dev/null 2>&1; do
        (($(date +%s%N) < deadline)) || return 1
        sleep 0.1
    done
}
INDICATOR="Main.panel.statusArea['amethystora-connect']"

skip "1 the crate's tests (connectivity vectors V-CR-1 to 9, the loopback) run in the image build's apps stage"
skip "2 the interop run against kdeconnectd is apps/connect/tests/interop/run.sh, outside the lab"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi

# The lab's Eval, as 17-pill.sh has it
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
if [[ ${enabled} != *vmlab-eval@lab* ]]; then
    user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    systemctl restart gdm
fi
connect_ready() { [[ "$(ev "typeof Main.extensionManager.lookup('${UUID}')?.stateObj?.modules?.get('connect')")" == object ]]; }
if ! wait_for 240 has_session || ! wait_for 120 connect_ready; then
    not_ok "the connect module did not start in the session"
    finish
fi
ev "Main.overview.hide(); 'hidden'" >/dev/null

# --- The phone's network: a namespace on a veth pair whose computer end is in the home zone -----------------------------
install -d -m 0700 "${WORK}"
cp "${HERE}/phonepeer.py" "${WORK}/"
ip netns del "${NS}" 2>/dev/null
ip link del vmphone0 2>/dev/null
ip netns add "${NS}"
ip link add vmphone0 type veth peer name vmphone1
ip link set vmphone1 netns "${NS}"
nmcli device set vmphone0 managed no 2>/dev/null
ip addr add "${HOST_IP}/24" dev vmphone0
ip link set vmphone0 up
ip -n "${NS}" addr add 10.77.0.2/24 dev vmphone1
ip -n "${NS}" link set vmphone1 up
ip -n "${NS}" link set lo up
firewall-cmd --zone=amethystora-home --add-interface=vmphone0 >/dev/null
PEER_PID=""
cleanup() {
    [[ -n "${PEER_PID}" ]] && kill "${PEER_PID}" 2>/dev/null
    # Amethystora UI back on, whatever step the suite stopped at
    user_bus gnome-extensions enable "${UUID}" 2>/dev/null
    in_session "ame desktop phone photos pictures" >/dev/null 2>&1
    in_session "ame desktop phone off" >/dev/null 2>&1
    firewall-cmd --zone=amethystora-home --remove-interface=vmphone0 >/dev/null 2>&1
    ip link del vmphone0 2>/dev/null
    ip netns del "${NS}" 2>/dev/null
    rm -f "${HOME_DIR}/.config/amethystora/connect/devices.json"
}
trap cleanup EXIT

# --- The pairing, by pin: the program's own certificate first (a new one voids every pin), then the phone's pin --------
CONNECT_DIR="${HOME_DIR}/.config/amethystora/connect"
# The unit as it is now, after a live push of it too
user_systemctl daemon-reload
in_session "ame desktop phone on" >/dev/null 2>&1
wait_for 30 test -s "${CONNECT_DIR}/certificate.pem" || not_ok "the program made no certificate"
in_session "ame desktop phone off" >/dev/null 2>&1
python3 "${WORK}/phonepeer.py" init "${PEER}" >/dev/null
PHONE_ID="$(cat "${PEER}/id")"
PIN="$(python3 "${WORK}/phonepeer.py" pin "${PEER}")"
jq -n --arg id "${PHONE_ID}" --arg pin "${PIN}" '{version: 1, devices: {($id): {name: "Lab phone", type: "phone", protocol: 8,
    incoming: ["kdeconnect.ping", "kdeconnect.battery.request", "kdeconnect.share.request"],
    outgoing: ["kdeconnect.battery", "kdeconnect.share.request", "kdeconnect.connectivity_report"],
    pin: $pin, address: null, port: null, paired_at: 1}}}' >"${CONNECT_DIR}/devices.json"
chown "${TEST_USER}:" "${CONNECT_DIR}/devices.json"
chmod 0600 "${CONNECT_DIR}/devices.json"
in_session "ame desktop phone on" >/dev/null 2>&1
listening() { ss -Hltn "sport = :1716" | grep -q .; }
wait_for 30 listening || not_ok "the program does not listen" "$(journalctl -b _SYSTEMD_USER_UNIT=${UNIT} --no-pager -o cat | tail -n 20)"

DEVICE="/org/amethystora/Connect/Device/${PHONE_ID}"
# The program tells a caller that is neither its app nor GNOME Shell (this suite's gdbus) nothing: its properties read
# empty. So they are read from inside GNOME Shell, as the connect module reads them, through Eval; the answer is the
# value as JSON, a string bare
connect_prop() { # PATH INTERFACE NAME
    ev "(() => { const {Gio, GLib} = imports.gi;
        return Gio.DBus.session.call_sync('org.amethystora.Connect', '$1', 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', ['$2', '$3']), null, Gio.DBusCallFlags.NONE, 3000, null).recursiveUnpack()[0]; })()" 2>/dev/null
}
prop() { connect_prop "${DEVICE}" org.amethystora.Connect.Device "$1"; } # What the program says of the phone
send() { printf '%s\n' "$*" >>"${PEER}/commands"; }
ip netns exec "${NS}" python3 "${WORK}/phonepeer.py" run "${PEER}" "${HOST_IP}" >"${WORK}/peer.out" 2>&1 &
PEER_PID=$!
connected() { [[ "$(prop Connected)" == true && "$(prop Paired)" == true ]]; }
if ! wait_for 30 connected; then
    not_ok "the stand-in phone did not connect as a paired phone" "$(cat "${PEER}/log" "${WORK}/peer.out" 2>&1)" \
        "the shell reads Connected '$(prop Connected)', Paired '$(prop Paired)'" \
        "$(journalctl -b _SYSTEMD_USER_UNIT=${UNIT} --no-pager -o cat | tail -n 20)"
    finish
fi
ok "the stand-in phone is paired and connected"
send 'packet kdeconnect.battery {"currentCharge": 54, "isCharging": false, "thresholdEvent": 0}'

# --- 3-4: the signal, in the program and in the bar ---------------------------------------------------------------------
send 'packet kdeconnect.connectivity_report {"signalStrengths": {"6": {"networkType": "LTE", "signalStrength": 3}}}'
lte() { [[ "$(prop SignalStrength)" == 3 && "$(prop NetworkType)" == LTE ]]; }
check "3 within a second the program says signal 3 on LTE" within 1 lte
same "3 ...and the feature connectivity" "$(prop Features | grep -o connectivity)" connectivity
named() { [[ "$(ev "${INDICATOR}?.accessible_name")" == "$1" ]]; }
capsule_says() { named "Phone: battery 54 per cent, signal 3 of 4 bars"; }
if wait_for 5 capsule_says; then
    ok "3 the top bar's capsule reads \"Phone: battery 54 per cent, signal 3 of 4 bars\""
else
    not_ok "3 the top bar's capsule" "$(ev "${INDICATOR}?.accessible_name")"
fi
same "3 its parts: the battery, its charge, the bars at 3" \
    "$(ev "(() => { const i = ${INDICATOR}; return [i._capsule.visible, i._battery.visible, i._level.text, i._bars.visible, i._bars.level, i._icon.visible].join(' '); })()")" \
    "true true 54% true 3 false"
send 'packet kdeconnect.connectivity_report {"signalStrengths": {}}'
battery_only() { named "Phone: battery 54 per cent"; }
check "4 a report with no SIM takes the signal out of the capsule" wait_for 5 battery_only
same "4 ...and out of the program" "$(prop SignalStrength) [$(prop NetworkType)]" "-1 []"
# Text that is not a short plain word is never shown
send 'packet kdeconnect.connectivity_report {"signalStrengths": {"1": {"networkType": "<b>LTE</b>", "signalStrength": 2}}}'
unknown() { [[ "$(prop NetworkType)" == Unknown ]]; }
check "4 a network type that is not a plain word is Unknown" wait_for 5 unknown

# --- 5-6: pictures -------------------------------------------------------------------------------------------------------
PICTURES="$(in_session "xdg-user-dir PICTURES" 2>/dev/null)"
[[ -n "${PICTURES}" && "${PICTURES}" != "${HOME_DIR}" ]] || PICTURES="${HOME_DIR}/Pictures"
python3 -c 'import random; random.seed(24); open("/run/vmlab-24/holiday.jpg", "wb").write(bytes([0xff, 0xd8, 0xff, 0xe0]) + random.randbytes(200000))'
send "share ${WORK}/holiday.jpg"
landed() { test -f "${PICTURES}/Phone/holiday.jpg"; }
if wait_for 20 landed; then
    same "5 a picture lands in Pictures/Phone, whole" "$(sha256sum <"${PICTURES}/Phone/holiday.jpg")" "$(sha256sum <"${WORK}/holiday.jpg")"
    same "5 ...0600 in a folder of 0700" "$(stat -c %a "${PICTURES}/Phone/holiday.jpg") $(stat -c %a "${PICTURES}/Phone")" "600 700"
else
    not_ok "5 no picture in ${PICTURES}/Phone" "$(ls -la "${PICTURES}" "${HOME_DIR}/Downloads/Phone" 2>&1)"
fi
same "5 PhotoFolder says so" "$(connect_prop /org/amethystora/Connect org.amethystora.Connect PhotoFolder)" "${PICTURES}/Phone"
in_session "ame desktop phone photos downloads" >/dev/null 2>&1
same "6 ame desktop phone photos downloads" "$(gs get org.amethystora.Connect photo-folder)" "'downloads'"
same "6 Control's entry says downloads" \
    "$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -r '.items[] | select(.id == "phone-photos") | .state')" downloads
cp "${WORK}/holiday.jpg" "${WORK}/second.jpg"
send "share ${WORK}/second.jpg"
in_downloads() { test -f "${HOME_DIR}/Downloads/Phone/Lab phone/second.jpg"; }
check "6 the next picture lands in Downloads/Phone/Lab phone" wait_for 20 in_downloads
in_session "ame desktop phone photos pictures" >/dev/null 2>&1
same "6 ame desktop phone photos pictures puts it back" "$(gs get org.amethystora.Connect photo-folder)" "'pictures'"

# --- 7: a link goes to the shell, not to a notification, while the module is a client ---------------------------------
MONITOR="${WORK}/notify.log"
user_bus timeout 20 dbus-monitor --session "type='method_call',interface='org.freedesktop.Notifications',member='Notify'" >"${MONITOR}" 2>&1 &
sleep 2
send 'packet kdeconnect.share.request {"url": "https://example.org/vmlab-24"}'
# The live pill shows it, or, with the pill off, the module raises its own notification in the shell
taken() {
    [[ "$(ev "Main.extensionManager.lookup('${UUID}').stateObj.modules.get('pill')?.phone?._link?.url === 'https://example.org/vmlab-24' ||
        Main.messageTray.getSources().some(s => s.notifications.some(n => n.body === 'https://example.org/vmlab-24'))")" == true ]]
}
check "7 the shell received the link (UrlReceived)" wait_for 10 taken
sleep 2
if grep -q 'sent a link' "${MONITOR}"; then not_ok "7 the program raised a notification for it too"; else ok "7 the program raised no notification for it"; fi
user_bus gnome-extensions disable "${UUID}"
sleep 3
user_bus timeout 15 dbus-monitor --session "type='method_call',interface='org.freedesktop.Notifications',member='Notify'" >"${MONITOR}" 2>&1 &
sleep 2
send 'packet kdeconnect.share.request {"url": "https://example.org/vmlab-24-off"}'
noticed() { grep -q 'sent a link' "${MONITOR}"; }
check "7 with Amethystora UI off, the link is the program's notification as before" wait_for 12 noticed
user_bus gnome-extensions enable "${UUID}"
wait_for 60 connect_ready

# --- 8: ten disable and enable cycles with the phone connected ----------------------------------------------------------
send 'packet kdeconnect.connectivity_report {"signalStrengths": {"6": {"networkType": "LTE", "signalStrength": 3}}}'
wait_for 10 capsule_says
since="$(date '+%Y-%m-%d %H:%M:%S')"
for _ in $(seq 10); do
    user_bus gnome-extensions disable "${UUID}"
    sleep 1
    user_bus gnome-extensions enable "${UUID}"
    wait_for 20 capsule_says
done
check "8 after ten cycles the capsule is there, and says the phone" wait_for 10 capsule_says
same "8 ...once" "$(ev "(() => { let n = 0; const walk = a => { if (a.style_class?.includes?.('amethystora-connect-capsule')) n++; a.get_children().forEach(walk); }; walk(global.stage); return n; })()")" 1
errors="$(journalctl -b --since "${since}" _COMM=gnome-shell --no-pager -o cat | grep -c 'JS ERROR')"
same "8 no JS ERROR" "${errors}" 0

# --- 4: the phone goes, and the capsule with it --------------------------------------------------------------------------
send quit
no_capsule() { [[ "$(ev "${INDICATOR} ? 'there' : 'gone'")" == gone ]]; }
check "4 the phone disconnects: the capsule is gone within 2 s" wait_for 4 no_capsule
same "4 ...and the program forgets the signal" "$(prop SignalStrength)" -1
finish
