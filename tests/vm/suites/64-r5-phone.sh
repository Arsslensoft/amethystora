#!/usr/bin/bash
# The phone, a little further (plans/ui-ideas/spec-phone.md, round 5's board 58, suite 64): ringing the phone, music from
# either side and the phone as a slide clicker. The phone is phonepeer.py, the stand-in of suite 24, from a network
# namespace whose link the firewall counts as home, paired by its pin, and listing the ring, the players and the presenter;
# the computer's own player is mprisstandin.py on the session bus. The numbers are the spec's acceptance criteria (7).
# vmlab-paths: apps/connect/* apps/phone/* @ui/*connect* @ui/*present* @ui/*pill/media* @sf/*just/desktop.just @sf/*amethystora-control-list @sf/*org.amethystora.Connect.gschema.xml
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
UNIT=amethystora-connect.service
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-64
PEER="${WORK}/phone"
NS=vmphone64
HOST_IP=10.78.0.1
PLAYER_DIR=/tmp/vmlab-64-player
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
gs() { in_session "gsettings $*" 2>/dev/null; }
MODULES="Main.extensionManager.lookup('${UUID}').stateObj.modules"
INDICATOR="Main.panel.statusArea['amethystora-connect']"

skip "the crate's tests (tests/media.rs: ring, both directions of music with a stand-in MPRIS player, the clicker's keys) run in the image build's apps stage"
skip "what a real phone lists and how its presenter sends Next and Previous (spec 0, UNVERIFIED) need the maintainer's phone"

# --- 1. The keys, off as shipped, and the commands ------------------------------------------------------------------------
for key in music clicker; do
    same "1 ${key} is off as the image ships" "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect "${key}")" false
done

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
for key in music clicker; do
    in_session "ame desktop phone ${key} on" >/dev/null 2>&1
    same "1 ame desktop phone ${key} on" "$(gs get org.amethystora.Connect "${key}")" true
    in_session "ame desktop phone ${key} off" >/dev/null 2>&1
    same "1 ame desktop phone ${key} off" "$(gs get org.amethystora.Connect "${key}")" false
done

# --- 2. Control's entries ---------------------------------------------------------------------------------------------------
LIST="$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null)"
for id in phone-music phone-clicker; do
    same "2 Control lists ${id}, a switch, off" "$(jq -r --arg id "${id}" '.items[] | select(.id == $id) | "\(.kind) \(.state)"' <<<"${LIST}")" "switch off"
done
same "2 Control lists phone-ring, an action run without a terminal" \
    "$(jq -r '.items[] | select(.id == "phone-ring") | "\(.kind) \(.command | join(" ")) \(.terminal)"' <<<"${LIST}")" \
    "action ame desktop phone ring false"

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
connect_ready() { [[ "$(ev "typeof ${MODULES}?.get('connect')")" == object ]]; }
if ! wait_for 240 has_session || ! wait_for 120 connect_ready; then
    not_ok "the connect module did not start in the session"
    finish
fi
ev "Main.overview.hide(); 'hidden'" >/dev/null

# --- The phone's network, and the pairing by pin, as suite 24 makes them --------------------------------------------------
install -d -m 0700 "${WORK}"
cp "${HERE}/phonepeer.py" "${WORK}/"
install -d -m 0755 "${PLAYER_DIR}"
install -m 0644 "${HERE}/mprisstandin.py" "${PLAYER_DIR}/"
ip netns del "${NS}" 2>/dev/null
ip link del vmphone64a 2>/dev/null
ip netns add "${NS}"
ip link add vmphone64a type veth peer name vmphone64b
ip link set vmphone64b netns "${NS}"
nmcli device set vmphone64a managed no 2>/dev/null
ip addr add "${HOST_IP}/24" dev vmphone64a
ip link set vmphone64a up
ip -n "${NS}" addr add 10.78.0.2/24 dev vmphone64b
ip -n "${NS}" link set vmphone64b up
ip -n "${NS}" link set lo up
firewall-cmd --zone=amethystora-home --add-interface=vmphone64a >/dev/null
PEER_PID=""
PLAYER_PID=""
cleanup() {
    [[ -n "${PEER_PID}" ]] && kill "${PEER_PID}" 2>/dev/null
    [[ -n "${PLAYER_PID}" ]] && kill "${PLAYER_PID}" 2>/dev/null
    pkill -f "${PLAYER_DIR}/mprisstandin.py" 2>/dev/null
    user_bus gnome-extensions enable "${UUID}" 2>/dev/null
    ev "${MODULES}.get('present')?.stop(); 'stopped'" >/dev/null 2>&1
    in_session "ame desktop present off" >/dev/null 2>&1
    in_session "ame desktop phone music off" >/dev/null 2>&1
    in_session "ame desktop phone clicker off" >/dev/null 2>&1
    in_session "ame desktop phone off" >/dev/null 2>&1
    firewall-cmd --zone=amethystora-home --remove-interface=vmphone64a >/dev/null 2>&1
    ip link del vmphone64a 2>/dev/null
    ip netns del "${NS}" 2>/dev/null
    rm -rf "${PLAYER_DIR}"
    rm -f "${HOME_DIR}/.config/amethystora/connect/devices.json"
}
trap cleanup EXIT

PHONE_IN_MORE="kdeconnect.findmyphone.request,kdeconnect.mpris,kdeconnect.mpris.request"
PHONE_OUT_MORE="kdeconnect.mpris,kdeconnect.mpris.request,kdeconnect.mousepad.request,kdeconnect.presenter"
CONNECT_DIR="${HOME_DIR}/.config/amethystora/connect"
user_systemctl daemon-reload
in_session "ame desktop phone on" >/dev/null 2>&1
wait_for 30 test -s "${CONNECT_DIR}/certificate.pem" || not_ok "the program made no certificate"
in_session "ame desktop phone off" >/dev/null 2>&1
python3 "${WORK}/phonepeer.py" init "${PEER}" >/dev/null
PHONE_ID="$(cat "${PEER}/id")"
PIN="$(python3 "${WORK}/phonepeer.py" pin "${PEER}")"
jq -n --arg id "${PHONE_ID}" --arg pin "${PIN}" '{version: 1, devices: {($id): {name: "Lab phone", type: "phone", protocol: 8,
    incoming: ["kdeconnect.ping", "kdeconnect.findmyphone.request"], outgoing: ["kdeconnect.ping"],
    pin: $pin, address: null, port: null, paired_at: 1}}}' >"${CONNECT_DIR}/devices.json"
chown "${TEST_USER}:" "${CONNECT_DIR}/devices.json"
chmod 0600 "${CONNECT_DIR}/devices.json"
in_session "ame desktop phone on" >/dev/null 2>&1
listening() { ss -Hltn "sport = :1716" | grep -q .; }
wait_for 30 listening || not_ok "the program does not listen" "$(journalctl -b _SYSTEMD_USER_UNIT=${UNIT} --no-pager -o cat | tail -n 20)"

DEVICE="/org/amethystora/Connect/Device/${PHONE_ID}"
# The program tells this suite's gdbus nothing, so its properties are read from inside GNOME Shell, as the module reads them
connect_prop() { # PATH INTERFACE NAME
    ev "(() => { const {Gio, GLib} = imports.gi;
        return JSON.stringify(Gio.DBus.session.call_sync('org.amethystora.Connect', '$1', 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', ['$2', '$3']), null, Gio.DBusCallFlags.NONE, 3000, null).recursiveUnpack()[0]); })()" 2>/dev/null
}
prop() { connect_prop "${DEVICE}" org.amethystora.Connect.Device "$1"; }
send() { printf '%s\n' "$*" >>"${PEER}/commands"; }
seen() { grep -c -- "$1" "${PEER}/log" 2>/dev/null; }
# The stand-in phone, (again) from a fresh log: the program lets a phone go when a switch changes what it lists, and a
# phone comes back by itself; this one is started again instead
start_peer() {
    [[ -n "${PEER_PID}" ]] && { send quit; sleep 1; kill "${PEER_PID}" 2>/dev/null; }
    rm -f "${PEER}/commands" "${PEER}/log"
    ip netns exec "${NS}" env PEER_IN_MORE="${PHONE_IN_MORE}" PEER_OUT_MORE="${PHONE_OUT_MORE}" \
        python3 "${WORK}/phonepeer.py" run "${PEER}" "${HOST_IP}" >"${WORK}/peer.out" 2>&1 &
    PEER_PID=$!
    wait_for 30 connected
}
connected() { [[ "$(prop Connected)" == true && "$(prop Paired)" == true ]]; }
let_go() { grep -q "closed by the computer" "${PEER}/log" 2>/dev/null; }
if ! start_peer; then
    not_ok "the stand-in phone did not connect as a paired phone" "$(cat "${PEER}/log" "${WORK}/peer.out" 2>&1)" \
        "$(journalctl -b _SYSTEMD_USER_UNIT=${UNIT} --no-pager -o cat | tail -n 20)"
    finish
fi
ok "the stand-in phone is paired and connected"

# --- 3. Ring the phone --------------------------------------------------------------------------------------------------
has_feature() { prop Features | grep -q "\"$1\""; }
check "3 Features has ring" wait_for 10 has_feature ring
row_shown() {
    [[ "$(ev "(() => { const s = [...${INDICATOR}._sections.values()][0]; return s._rows.find(r => r.feature === 'ring').item.visible; })()")" == true ]]
}
check "3 the phone's menu shows Ring the phone" wait_for 10 row_shown
same "3 ...labelled so" "$(ev "[...${INDICATOR}._sections.values()][0]._rows.find(r => r.feature === 'ring').item.label.text")" "Ring the phone"
ev "(() => { ${MODULES}.get('connect')._actions.ring(${MODULES}.get('connect')._ready()[0]); return 'asked'; })()" >/dev/null
rung() { [[ "$(seen 'got kdeconnect.findmyphone.request')" -ge "$1" ]]; }
check "3 the menu's row sends the phone one kdeconnect.findmyphone.request" wait_for 10 rung 1
same "3 ...with no fields" "$(grep 'got kdeconnect.findmyphone.request' "${PEER}/log" | head -n 1 | awk '{print $4}')" "{}"
in_session "timeout 60 ame desktop phone ring" >/dev/null 2>&1
check "3 ame desktop phone ring rings it too, without a window" wait_for 10 rung 2
same "3 ...and no Phone window is left open" "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.amethystora.Phone')")" false

# --- 4. Off: neither music nor the clicker is listed or heard ---------------------------------------------------------------
listed() { grep "their $1 " "${PEER}/log" | tail -n 1 | tr ', ' '\n\n' | grep -qx -- "$2"; }
for kind in kdeconnect.mpris kdeconnect.mpris.request kdeconnect.presenter kdeconnect.mousepad.request; do
    if listed incoming "${kind}"; then not_ok "4 off, the program lists ${kind}"; else ok "4 off, the program does not list ${kind}"; fi
done
send 'packet kdeconnect.mpris {"playerList": ["Music"]}'
sleep 2
same "4 a phone's player list changes nothing" "$(prop Players)" "[]"
same "4 ...and nothing is asked of it" "$(seen 'got kdeconnect.mpris.request')" 0

# --- 5. Music from either side: the phone's player in the pill ------------------------------------------------------------
in_session "ame desktop phone music on" >/dev/null 2>&1
check "5 music on lets the phone go, to come back with the new lists" wait_for 10 let_go
start_peer || not_ok "5 the phone did not come back"
for side in incoming outgoing; do
    if listed "${side}" kdeconnect.mpris && listed "${side}" kdeconnect.mpris.request; then
        ok "5 the program lists the media packets as ${side}"
    else
        not_ok "5 the media packets are not listed as ${side}" "$(grep their "${PEER}/log")"
    fi
done
asked_list() { grep 'got kdeconnect.mpris.request' "${PEER}/log" | grep -q '"requestPlayerList":true'; }
check "5 the program asks the phone for its players" wait_for 10 asked_list
send 'packet kdeconnect.mpris {"playerList": ["Music"]}'
asked_state() { grep 'got kdeconnect.mpris.request' "${PEER}/log" | grep '"player":"Music"' | grep -q '"requestNowPlaying":true'; }
check "5 ...and for the state of the one it lists" wait_for 10 asked_state
send 'packet kdeconnect.mpris {"player": "Music", "title": "Night Drive", "artist": "Lab Band", "isPlaying": true, "canPause": true, "canGoNext": true, "length": 240000, "pos": 1000}'
in_players() { prop Players | grep -q '"title":"Night Drive"'; }
check "5 the program's Players has it" wait_for 10 in_players
in_pill() { [[ "$(ev "${MODULES}.get('pill')?._media?.player?.source ?? ''")" == "Music on Lab phone" ]]; }
if wait_for 10 in_pill; then
    ok "5 the pill's now playing shows the phone's player"
else
    not_ok "5 the pill does not show the phone's player" "$(ev "${MODULES}.get('pill')?._media?.text ?? 'nothing'")"
fi
same "5 ...with its title and artist" "$(ev "${MODULES}.get('pill')?._media?.text ?? ''")" "Night Drive · Lab Band"
ev "${MODULES}.get('pill')._media.player.playPause(); 'pressed'" >/dev/null
told() { grep 'got kdeconnect.mpris.request' "${PEER}/log" | grep '"player":"Music"' | grep -q '"action":"PlayPause"'; }
check "5 the pill's Pause sends the phone PlayPause for that player" wait_for 10 told

# --- 6. Music from either side: this computer's player on the phone ------------------------------------------------------
user_bus python3 "${PLAYER_DIR}/mprisstandin.py" "${PLAYER_DIR}/calls.log" &
PLAYER_PID=$!
sleep 2
send 'packet kdeconnect.mpris.request {"requestPlayerList": true}'
its_list() { grep 'got kdeconnect.mpris ' "${PEER}/log" | grep -q '"playerList":\["VM lab player"\]'; }
check "6 the phone is sent this computer's players" wait_for 10 its_list
same "6 ...and no album art is offered" "$(grep 'got kdeconnect.mpris ' "${PEER}/log" | grep playerList | tail -n 1 | grep -o '"supportAlbumArtPayload":false')" '"supportAlbumArtPayload":false'
send 'packet kdeconnect.mpris.request {"player": "VM lab player", "requestNowPlaying": true}'
its_state() { grep 'got kdeconnect.mpris ' "${PEER}/log" | grep '"player":"VM lab player"' | grep -q '"title":"Lab song"'; }
check "6 its state answers the phone: the title" wait_for 10 its_state
send 'packet kdeconnect.mpris.request {"player": "VM lab player", "action": "Next"}'
nexted() { grep -qx Next "${PLAYER_DIR}/calls.log" 2>/dev/null; }
check "6 the phone's Next reaches the player" wait_for 10 nexted

# --- 7. The clicker on, not presenting: no key ---------------------------------------------------------------------------
in_session "ame desktop present on" >/dev/null 2>&1
in_session "ame desktop phone clicker on" >/dev/null 2>&1
check "7 the clicker on lets the phone go too" wait_for 10 let_go
start_peer || not_ok "7 the phone did not come back"
if listed incoming kdeconnect.presenter; then ok "7 the program lists the presenter"; else not_ok "7 the presenter is not listed"; fi
if listed incoming kdeconnect.mousepad.request; then not_ok "7 the remote keyboard is listed"; else ok "7 the remote keyboard stays unlisted"; fi
check "7 Features has clicker" wait_for 10 has_feature clicker
present_state() { ev "JSON.stringify(${MODULES}.get('present').state())"; }
clicks() { ev "${MODULES}.get('present').state().clicks"; }
send 'packet kdeconnect.mousepad.request {"specialKey": 9}'
sleep 2
same "7 not presenting, a Page Down from the phone makes no key" "$(clicks)" 0

# --- 8. Presenting: the three keys and nothing else -------------------------------------------------------------------------
ev "${MODULES}.get('present').start(); 'started'" >/dev/null
sleep 1
same "8 presenting listens to the phone" "$(ev "${MODULES}.get('present').state().phoneListened")" true
n=0
for body in '{"specialKey": 9}' '{"specialKey": 8}' '{"key": "b"}'; do
    n=$((n + 1))
    send "packet kdeconnect.mousepad.request ${body}"
    clicked() { [[ "$(clicks)" == "$1" ]]; }
    check "8 ${body} is a click" wait_for 6 clicked "${n}"
done
for body in '{"key": "x"}' '{"dx": 4.0, "dy": 1.0}' '{"specialKey": 9, "ctrl": true}'; do
    send "packet kdeconnect.mousepad.request ${body}"
    send 'packet kdeconnect.presenter {"dx": 3.0, "dy": 1.0}'
    sleep 1
    same "8 ${body} is no key" "$(clicks)" "${n}"
done
same "8 the keys come from a virtual keyboard made for them" "$(ev "${MODULES}.get('present').state().keyboard")" true
ev "${MODULES}.get('present').stop(); 'stopped'" >/dev/null
sleep 1
same "8 presenting stopped: the keyboard is gone and nothing listens" \
    "$(ev "(() => { const s = ${MODULES}.get('present').state(); return [s.keyboard, s.phoneListened].join(' '); })()")" "false false"
send 'packet kdeconnect.mousepad.request {"specialKey": 9}'
sleep 2
same "8 ...and a key from the phone does nothing" "$(clicks)" "${n}"

# --- 9. Off costs nothing ------------------------------------------------------------------------------------------------------
in_session "ame desktop phone clicker off" >/dev/null 2>&1
wait_for 10 let_go
in_session "ame desktop phone music off" >/dev/null 2>&1
start_peer || not_ok "9 the phone did not come back"
before="$(seen 'got kdeconnect.mpris')"
send 'packet kdeconnect.mpris.request {"requestPlayerList": true}'
sleep 2
same "9 music off: the phone's request is not answered" "$(seen 'got kdeconnect.mpris')" "${before}"
same "9 ...and the program keeps no players" "$(prop Players)" "[]"
same "9 the pill shows no phone player" "$(ev "${MODULES}.get('connect')?.mediaSource?.players.length ?? 0")" 0

# --- 10. Ten disable and enable cycles with the phone connected and music on ----------------------------------------------
in_session "ame desktop phone music on" >/dev/null 2>&1
wait_for 10 let_go
start_peer || not_ok "10 the phone did not come back"
send 'packet kdeconnect.mpris {"player": "Music", "title": "Night Drive", "isPlaying": true}'
wait_for 10 in_players
since="$(date '+%Y-%m-%d %H:%M:%S')"
for _ in $(seq 10); do
    user_bus gnome-extensions disable "${UUID}"
    sleep 1
    user_bus gnome-extensions enable "${UUID}"
    wait_for 20 connect_ready
done
phone_playing() { [[ "$(ev "${MODULES}.get('connect')?.mediaSource?.players.length ?? 0")" == 1 ]]; }
check "10 after ten cycles the phone's player is there once" wait_for 20 phone_playing
errors="$(journalctl -b --since "${since}" _COMM=gnome-shell --no-pager -o cat | grep -c 'JS ERROR')"
same "10 no JS ERROR" "${errors}" 0
finish
