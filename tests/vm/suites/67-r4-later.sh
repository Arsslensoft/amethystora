#!/usr/bin/bash
# Round 4, group later (plans/beyond/spec-r4-later.md): A, computer to computer, and B, live captions.
#
# A: a stand-in laptop (phonepeer.py with PEER_TYPE=laptop) on a network namespace whose link the firewall counts as
# home, paired by its pin. With the switch `computers` off it is let go; on, it connects, is listed as a laptop, sends a
# file and the top bar draws a computer; off again, it leaves the bus and its pairing stays. A second Amethystora
# machine is the crate's test (apps/connect/tests/computers.rs) and the interop run's.
#
# B: the recognizer in the image, the model fetched by its pinned checksum (from the copy run.mjs hands the guest,
# /var/tmp/vmlab-captions.zip, when the lab keeps one in /vm/cache, else from alphacephei.com; with neither, B3 and what
# needs the model are skipped with one reason), a sentence spoken by espeak-ng and played into a null sink, its words in the box,
# the program paused while nothing plays, nothing written or sent, off, remove, and ten disable/enable cycles.
# vmlab-paths: @ui/extension.js @ui/*captions* @ui/*connect* @ui/*quick* apps/connect/* apps/phone/* @sf/usr/libexec/amethystora-captions @sf/*user-setup.hooks.d/26-phone-computers.sh @sf/*Connect.gschema.xml @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just @sf/*systemd/user/amethystora-connect.service
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
UNIT=amethystora-connect.service
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-67
PEER="${WORK}/laptop"
NS=vmlaptop
HOST_IP=10.78.0.1
CAPTIONS=/usr/libexec/amethystora-captions
MODEL_DIR="${HOME_DIR}/.local/share/amethystora/captions/model"
# Eval's answer is JSON inside a GVariant, which gdbus quotes with ' or " as the text needs: parsed as both, as
# 36-r4-access.sh does; a string printed bare, anything else as JSON
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c 'import json, sys
from gi.repository import GLib
v = GLib.Variant.parse(None, sys.stdin.read().strip(), None, None)
try:
    x = json.loads(v[1])
except ValueError:
    x = v[1]
print(x if isinstance(x, str) else json.dumps(x))'
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
gs() { in_session "gsettings $*" 2>/dev/null; }
MODULE="Main.extensionManager.lookup('${UUID}').stateObj.modules"
state() { ev "JSON.stringify(${MODULE}.get('captions').state())"; }
field() { state | jq -r ".$1"; }

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi

# The lab's Eval, as 24-phone-more.sh has it
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
ready() { [[ "$(ev "typeof ${MODULE}?.get('captions')")" == object ]]; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "Amethystora UI did not start in the session"
    finish
fi
ev "Main.overview.hide(); 'hidden'" >/dev/null
# Nobody types in the lab: GNOME would lock the screen after its idle delay, part way through, and the lock screen stops
# Amethystora UI. Never idle while the suite runs, as it was after
IDLE_BEFORE="$(gs get org.gnome.desktop.session idle-delay)"
gs set org.gnome.desktop.session idle-delay 0
install -d -m 0755 "${WORK}"
restore_animations() {
    ev "global.force_animations = true; if (global._vmlabForced) imports.gi.St.Settings.get().inhibit_animations(); global._vmlabForced = false; 1" >/dev/null 2>&1
}
cleanup() {
    restore_animations
    [[ -n "${IDLE_BEFORE:-}" ]] && gs set org.gnome.desktop.session idle-delay "${IDLE_BEFORE#uint32 }"
    [[ -n "${PEER_PID:-}" ]] && kill "${PEER_PID}" 2>/dev/null
    [[ -n "${PEER2_PID:-}" ]] && kill "${PEER2_PID}" 2>/dev/null
    [[ -n "${SILENT_PID:-}" ]] && kill "${SILENT_PID}" 2>/dev/null
    loginctl unlock-sessions 2>/dev/null
    user_bus gnome-extensions enable "${UUID}" 2>/dev/null
    in_session "ame desktop phone computers off" >/dev/null 2>&1
    in_session "ame desktop phone off" >/dev/null 2>&1
    in_session "ame desktop captions remove" >/dev/null 2>&1
    [[ -n "${SINK_MODULE:-}" ]] && in_session "pactl unload-module ${SINK_MODULE}" >/dev/null 2>&1
    firewall-cmd --zone=amethystora-home --remove-interface=vmlaptop0 >/dev/null 2>&1
    ip link del vmlaptop0 2>/dev/null
    ip netns del "${NS}" 2>/dev/null
    rm -f "${HOME_DIR}/.config/amethystora/connect/devices.json"
}
trap cleanup EXIT

# === A. Computer to computer ===========================================================================================

same "A1 the switch for other computers is off as the image ships" "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect computers)" false
in_session "ame desktop phone computers on" >/dev/null 2>&1
same "A1 ame desktop phone computers on" "$(gs get org.amethystora.Connect computers)" true
in_session "ame desktop phone computers" >/dev/null 2>&1
same "A1 ...and without a word it flips" "$(gs get org.amethystora.Connect computers)" false
same "A10 Control's Phone group has the switch, off, run without a terminal" \
    "$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -r '.items[] | select(.id == "phone-computers") |
        [.group, .state, (.choices | map(.terminal) | unique | tostring)] | join(" ")')" "Phone off [false]"

# The laptop's network and its pairing by pin, as 24-phone-more.sh sets up a phone
cp "${HERE}/phonepeer.py" "${WORK}/"
ip netns del "${NS}" 2>/dev/null
ip link del vmlaptop0 2>/dev/null
ip netns add "${NS}"
ip link add vmlaptop0 type veth peer name vmlaptop1
ip link set vmlaptop1 netns "${NS}"
nmcli device set vmlaptop0 managed no 2>/dev/null
ip addr add "${HOST_IP}/24" dev vmlaptop0
ip link set vmlaptop0 up
ip -n "${NS}" addr add 10.78.0.2/24 dev vmlaptop1
ip -n "${NS}" link set vmlaptop1 up
ip -n "${NS}" link set lo up
firewall-cmd --zone=amethystora-home --add-interface=vmlaptop0 >/dev/null
CONNECT_DIR="${HOME_DIR}/.config/amethystora/connect"
# The unit as it is now, after a live push of it too
user_systemctl daemon-reload
in_session "ame desktop phone on" >/dev/null 2>&1
wait_for 30 test -s "${CONNECT_DIR}/certificate.pem" || not_ok "the program made no certificate"
in_session "ame desktop phone off" >/dev/null 2>&1
python3 "${WORK}/phonepeer.py" init "${PEER}" >/dev/null
LAPTOP_ID="$(cat "${PEER}/id")"
PIN="$(python3 "${WORK}/phonepeer.py" pin "${PEER}")"
jq -n --arg id "${LAPTOP_ID}" --arg pin "${PIN}" '{version: 1, devices: {($id): {name: "Lab laptop", type: "laptop", protocol: 8,
    incoming: ["kdeconnect.ping", "kdeconnect.share.request", "kdeconnect.clipboard"],
    outgoing: ["kdeconnect.share.request", "kdeconnect.clipboard"],
    pin: $pin, address: null, port: null, paired_at: 1}}}' >"${CONNECT_DIR}/devices.json"
chown "${TEST_USER}:" "${CONNECT_DIR}/devices.json"
chmod 0600 "${CONNECT_DIR}/devices.json"
in_session "ame desktop phone on" >/dev/null 2>&1
listening() { ss -Hltn "sport = :1716" | grep -q .; }
wait_for 30 listening || not_ok "the program does not listen" "$(journalctl -b _SYSTEMD_USER_UNIT=${UNIT} --no-pager -o cat | tail -n 20)"
DEVICE="/org/amethystora/Connect/Device/${LAPTOP_ID}"
# The program tells a caller that is neither its app nor GNOME Shell (this suite's gdbus) nothing: its properties read
# empty and its methods are refused. So it is asked from inside GNOME Shell, as the connect module asks, through Eval;
# the answer is the reply's value as JSON, a string bare
connect_bus() { # PATH INTERFACE METHOD [SIGNATURE ARGS-AS-JS]
    ev "(() => { const {Gio, GLib} = imports.gi; const args = '${4:-}' ? new GLib.Variant('${4:-()}', ${5:-[]}) : null;
        const v = Gio.DBus.session.call_sync('org.amethystora.Connect', '$1', '$2', '$3', args, null, Gio.DBusCallFlags.NONE, 3000, null).recursiveUnpack();
        return v.length === 1 ? v[0] : v; })()" 2>/dev/null
}
prop() { connect_bus "${DEVICE}" org.freedesktop.DBus.Properties Get '(ss)' "['org.amethystora.Connect.Device', '$1']"; }
listed() { connect_bus /org/amethystora/Connect org.amethystora.Connect GetDevices | grep -q "Device/${LAPTOP_ID}"; }
root_prop() { connect_bus /org/amethystora/Connect org.freedesktop.DBus.Properties Get '(ss)' "['org.amethystora.Connect', '$1']"; }
# peer [TYPE]: the paired laptop's stand-in, as a laptop or as another kind it claims to be
peer() {
    rm -f "${PEER}/commands" "${PEER}/log"
    PEER_TYPE="${1:-laptop}" PEER_NAME="Lab laptop" ip netns exec "${NS}" python3 "${WORK}/phonepeer.py" run "${PEER}" "${HOST_IP}" \
        >"${WORK}/peer.out" 2>&1 &
    PEER_PID=$!
}
same "A1 the program says Computers false" "$(root_prop Computers)" false

# A2: off, the paired laptop is neither listed nor let in
if listed; then not_ok "A2 a paired computer is listed while computers are off"; else ok "A2 a paired computer is not listed while off"; fi
peer
let_go() { grep -q "closed" "${PEER}/log" 2>/dev/null || ! kill -0 "${PEER_PID}" 2>/dev/null; }
check "A2 its connection is let go" wait_for 15 let_go
if listed; then not_ok "A2 ...but it is listed"; else ok "A2 ...and it is still not listed"; fi
kill "${PEER_PID}" 2>/dev/null

# A3-A5: on, it is back, a laptop, paired; a file from it lands in its folder; the top bar draws a computer
in_session "ame desktop phone computers on" >/dev/null 2>&1
# The program hears the setting from dconf a moment after gsettings has written it
computers_on() { [[ "$(root_prop Computers)" == true ]]; }
if wait_for 5 computers_on; then ok "A3 Computers true on the bus"; else not_ok "A3 Computers true on the bus" "found: $(root_prop Computers)"; fi
check "A3 the paired laptop is listed again" wait_for 10 listed
peer
connected() { [[ "$(prop Connected)" == true && "$(prop Paired)" == true ]]; }
if wait_for 30 connected; then
    ok "A3 the laptop connects as a paired computer, without a prompt"
else
    not_ok "A3 the laptop did not connect" "$(cat "${PEER}/log" "${WORK}/peer.out" 2>&1)" \
        "$(journalctl -b _SYSTEMD_USER_UNIT=${UNIT} --no-pager -o cat | tail -n 20)"
fi
same "A3 its type" "$(prop Type)" laptop
# A file is never overwritten: a run before this one would leave its own report.pdf for the check to read
rm -rf "${HOME_DIR}/Downloads/Phone/Lab laptop"
head -c 70000 /dev/urandom >"${WORK}/report.pdf"
printf 'share %s\n' "${WORK}/report.pdf" >>"${PEER}/commands"
landed() { test -f "${HOME_DIR}/Downloads/Phone/Lab laptop/report.pdf"; }
if wait_for 20 landed; then
    same "A5 a file from the laptop lands whole in Downloads/Phone/Lab laptop" \
        "$(sha256sum <"${HOME_DIR}/Downloads/Phone/Lab laptop/report.pdf")" "$(sha256sum <"${WORK}/report.pdf")"
    same "A5 ...0600 in a folder of 0700" "$(stat -c %a "${HOME_DIR}/Downloads/Phone/Lab laptop/report.pdf" "${HOME_DIR}/Downloads/Phone/Lab laptop" | paste -sd ' ')" "600 700"
else
    not_ok "A5 no file from the laptop" "$(ls -laR "${HOME_DIR}/Downloads/Phone" 2>&1 | head -n 20)"
fi

# A6: the clipboard, each way only while its switch says so
in_session "ame desktop phone clipboard receive on" >/dev/null 2>&1
in_session "ame desktop phone clipboard send on" >/dev/null 2>&1
sleep 2
printf '%s\n' 'packet kdeconnect.clipboard {"content": "copied on the lab laptop"}' >>"${PEER}/commands"
pasted() {
    ev "imports.gi.St.Clipboard.get_default().get_text(imports.gi.St.ClipboardType.CLIPBOARD, (c, t) => { global._vm67clip = t; }); 1" >/dev/null
    sleep 0.3
    [[ "$(ev "global._vm67clip")" == "copied on the lab laptop" ]]
}
check "A6 what the laptop copies is on this computer's clipboard" wait_for 10 pasted
ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.CLIPBOARD, 'copied on the desk'); 1" >/dev/null
sent() { [[ "$(grep -c 'got kdeconnect.clipboard' "${PEER}/log" 2>/dev/null)" -ge 1 ]]; }
check "A6 what this computer copies reaches the laptop" wait_for 10 sent
in_session "ame desktop phone clipboard send off" >/dev/null 2>&1
in_session "ame desktop phone clipboard receive off" >/dev/null 2>&1

# A4: an unpaired laptop asks to pair: the program's prompt and the app's card speak of the other computer
PEER2="${WORK}/laptop2"
python3 "${WORK}/phonepeer.py" init "${PEER2}" >/dev/null
LAPTOP2_ID="$(cat "${PEER2}/id")"
PEER_TYPE=laptop PEER_NAME="Lab laptop two" ip netns exec "${NS}" python3 "${WORK}/phonepeer.py" run "${PEER2}" "${HOST_IP}" \
    >"${WORK}/peer2.out" 2>&1 &
PEER2_PID=$!
prop2() {
    connect_bus "/org/amethystora/Connect/Device/${LAPTOP2_ID}" org.freedesktop.DBus.Properties Get '(ss)' \
        "['org.amethystora.Connect.Device', '$1']"
}
here2() { [[ "$(prop2 Connected)" == true ]]; }
check "A4 an unpaired laptop is listed, connected" wait_for 20 here2
printf 'packet kdeconnect.pair {"pair": true, "timestamp": %s}\n' "$(date +%s)" >>"${PEER2}/commands"
asked() { [[ "$(prop2 Pairing)" == incoming ]]; }
check "A4 its request is waiting" wait_for 10 asked
same "A4 ...with a key of eight characters to compare" "$(prop2 PairingKey | tr -d '\n' | wc -c)" 8
prompted() { [[ "$(ev "Main.messageTray.getSources().some(s => s.notifications.some(n => (n.body ?? '').includes('the key on the other computer')))")" == true ]]; }
check "A4 the prompt asks to compare it with the key on the other computer" wait_for 10 prompted
user_bus gdbus call --session --dest org.amethystora.Connect --object-path "/org/amethystora/Connect/Device/${LAPTOP2_ID}" \
    --method org.amethystora.Connect.Device.RejectPairing >/dev/null 2>&1
kill "${PEER2_PID}" 2>/dev/null
PEER2_PID=""
# The glyph's names in the order they are tried: a laptop's own first (TYPE_ICONS, spec A.5), then the computer's
icon() { ev "(Main.panel.statusArea['amethystora-connect']?._icon.gicon.get_names() ?? []).join(' ')"; }
computer_icon() { [[ " $(icon) " == *" computer-symbolic "* ]]; }
check "A9 the top bar draws the computer" wait_for 10 computer_icon

# A7: off again, it leaves the bus, and its pairing stays
in_session "ame desktop phone computers off" >/dev/null 2>&1
gone() { ! listed; }
check "A7 off: the laptop leaves the program's list" wait_for 10 gone
if jq -e --arg id "${LAPTOP_ID}" '.devices[$id]' "${CONNECT_DIR}/devices.json" >/dev/null; then
    ok "A7 ...and its pairing is kept"
else
    not_ok "A7 ...but its pairing is gone"
fi
check "A7 ...and its connection is closed" wait_for 15 let_go
kill "${PEER_PID}" 2>/dev/null

# A8: the paired laptop calling itself a phone is still a computer: refused while off, let in once on (so that it was the
# switch that refused it)
peer phone
check "A8 the paired laptop, as a phone, is let go while computers are off" wait_for 15 let_go
if listed; then not_ok "A8 ...but it is listed"; else ok "A8 ...and it is not listed"; fi
kill "${PEER_PID}" 2>/dev/null
in_session "ame desktop phone computers on" >/dev/null 2>&1
wait_for 10 listed
peer phone
check "A8 with computers on, the same caller is let in" wait_for 30 connected
kill "${PEER_PID}" 2>/dev/null
in_session "ame desktop phone computers off" >/dev/null 2>&1
in_session "ame desktop phone off" >/dev/null 2>&1

# === B. Live captions ==================================================================================================

# Amethystora UI running: a screen locked before the idle delay was set to never would have stopped it
loginctl unlock-sessions
wait_for 60 ready || not_ok "B Amethystora UI is not running at the start of B" "$(ev "typeof ${MODULE}?.get('captions')")"

# The lab renders in software, where GNOME turns animations off: the fades are checked with them forced on, and put
# back however the suite ends (cleanup)
ev "global._vmlabForced = !imports.gi.St.Settings.get().enable_animations; global.force_animations = true; if (global._vmlabForced) imports.gi.St.Settings.get().uninhibit_animations(); 1" >/dev/null

check "B1 the recognizer is in the image (gst-vosk)" rpm -q gst-vosk
check "B1 GStreamer has its element" gst-inspect-1.0 vosk
in_session "${CAPTIONS} remove" >/dev/null 2>&1
same "B1 a fresh account has no model" "$(in_session "${CAPTIONS} status" 2>/dev/null)" no-model
same "B2 the key is off" "$(gs get org.gnome.shell.extensions.amethystora-ui.captions enabled)" false
same "B2 off: no toggle, no program, no mixer, no box" "$(state | jq -c '[.toggle, .helper, .mixer, .box]')" "[false,null,false,false]"

# B4: not the model, refused, nothing kept
printf 'not a model' >"${WORK}/fake.zip"
chmod 0644 "${WORK}/fake.zip"
if in_session "AMETHYSTORA_CAPTIONS_URL=file://${WORK}/fake.zip ${CAPTIONS} fetch" >/dev/null 2>&1; then
    not_ok "B4 a download that is not the model was taken"
else
    ok "B4 a download that is not the model is refused"
fi
same "B4 ...and nothing is kept" "$(ls -A "${HOME_DIR}/.local/share/amethystora/captions" 2>/dev/null)" ""

# B3: the model, by its pinned checksum, then on
VMLAB_CAPTIONS_ZIP="${VMLAB_CAPTIONS_ZIP:-/var/tmp/vmlab-captions.zip}"
if [[ ! -s "${VMLAB_CAPTIONS_ZIP}" ]] && ! curl -sfI --max-time 15 https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip >/dev/null; then
    skip "B3-B13 need the speech model: the lab has no copy in /vm/cache and this machine cannot reach alphacephei.com"
    finish
fi
if [[ -s "${VMLAB_CAPTIONS_ZIP}" ]]; then
    install -m 0644 "${VMLAB_CAPTIONS_ZIP}" "${WORK}/model.zip"
    fetch="AMETHYSTORA_CAPTIONS_URL=file://${WORK}/model.zip ${CAPTIONS} fetch"
else
    fetch="${CAPTIONS} fetch"
fi
if in_session "${fetch}" >"${WORK}/fetch.out" 2>&1; then ok "B3 the model is fetched"; else not_ok "B3 the model could not be fetched" "$(tail -n 5 "${WORK}/fetch.out")"; fi
same "B3 ...into the captions folder, the zip gone" "$(ls -A "${HOME_DIR}/.local/share/amethystora/captions")" model
same "B3 status says ready" "$(in_session "${CAPTIONS} status" 2>/dev/null)" ready
in_session "ame desktop captions on" >/dev/null 2>&1
same "B3 ame desktop captions on" "$(gs get org.gnome.shell.extensions.amethystora-ui.captions enabled)" true
toggle() { [[ "$(field toggle) $(field checked)" == "true true" ]]; }
check "B3 the quick settings toggle is there, checked" wait_for 10 toggle
same "B14 Control's entry is on, its on in a terminal and its off not" \
    "$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -r '.items[] | select(.id == "captions") |
        [.state, (.choices | map("\(.value):\(.terminal)") | join(","))] | join(" ")')" "on on:true,off:false"
same "B13 the machine's sends list has the captions' row, nothing sent" \
    "$(user_bus /usr/libexec/amethystora-sends --json 2>/dev/null | jq -r '.[] | select(.id == "sends-captions") | .state')" off

# B5: on, nothing playing: one program, paused, spending nothing
helper_pid() { field helper; }
running() { [[ "$(helper_pid)" =~ ^[0-9]+$ ]]; }
check "B5 the program runs" wait_for 10 running
PID="$(helper_pid)"
cpu() { [[ "$1" =~ ^[0-9]+$ ]] && awk '{ print $14 + $15 }' "/proc/$1/stat" 2>/dev/null; }
# Loading the model, paused from the start, takes a few seconds of CPU: measure once it has settled (no tick in 3 s)
before="$(cpu "${PID}")"
for _ in {1..20}; do
    sleep 3
    settled="$(cpu "${PID}")"
    [[ "${settled}" == "${before}" ]] && break
    before="${settled}"
done
sleep 30
# A paused pipeline still wakes for a bus message now and then: a few ticks (10 ms each) in 30 s, never a stream
now="$(cpu "${PID}")"
paused=$((${now:-999999} - before))
if ((paused >= 0 && paused <= 3)); then ok "B5 ...paused while nothing plays: ${paused} ticks of CPU in 30 s"; else not_ok "B5 ...paused while nothing plays costs ${paused} ticks of CPU in 30 s"; fi
same "B5 ...and no box" "$(field visible)" false

# M2 of the review: a stream that is open and silent (a paused player keeps one) costs next to nothing
SINK_MODULE="$(in_session "pactl load-module module-null-sink sink_name=vmlab67 sink_properties=device.description=vmlab67" 2>/dev/null)"
in_session "pactl set-default-sink vmlab67" >/dev/null 2>&1
in_session "timeout 45 pacat --raw --format=s16le --rate=16000 --channels=1 /dev/zero" >/dev/null 2>&1 &
SILENT_PID=$!
sleep 10
before="$(cpu "${PID}")"
sleep 20
spent=$(( $(cpu "${PID}") - before ))
if (( spent < 100 )); then ok "B5 a silent open stream: ${spent} ticks of CPU in 20 s"; else not_ok "B5 a silent open stream costs ${spent} ticks of CPU in 20 s"; fi
kill "${SILENT_PID}" 2>/dev/null
wait "${SILENT_PID}" 2>/dev/null
SILENT_PID=""

# B6: a sentence played into the null sink, the default output
espeak-ng -s 140 -w "${WORK}/sentence.wav" "And that is why the bridge stayed up. The engineers had planned for a storm far worse than the one that came."
chmod 0644 "${WORK}/sentence.wav"
in_session "paplay ${WORK}/sentence.wav" >/dev/null 2>&1 &
shown() { [[ "$(field visible)" == true && -n "$(field text)" ]]; }
if wait_for 15 shown; then
    ok "B6 words show while the sentence plays: $(field text)"
else
    not_ok "B6 no words" "$(state)" "$(journalctl -b _COMM=gnome-shell --no-pager -o cat | grep -i captions | tail -n 5)"
fi
# B12: while words show, a full-screen window is composited (direct scanout held off), which a lab without KMS cannot see
same "B12 the scanout is held off while the words show" "$(field scanoutHeld)" true
ev "(() => { global._vm67opacity = []; ${MODULE}.get('captions')._box.connect('notify::opacity', a => global._vm67opacity.push(a.opacity)); return 1; })()" >/dev/null
geometry="$(state | jq -r '.geometry | join(" ")')"
read -r gx gy gw gh <<<"${geometry}"
area="$(ev "(() => { const a = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex); return [a.x, a.y, a.width, a.height].join(' '); })()")"
read -r ax ay aw ah <<<"${area}"
scale="$(ev "imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor")"
same "B6 the box is centred" "$((gx + gw / 2 - (ax + aw / 2)))" 0
same "B6 ...its bottom 48 px above the work area's" "$((ay + ah - (gy + gh)))" "$((48 * scale))"
style="$(ev "(() => { const b = ${MODULE}.get('captions')._box; const n = b.get_theme_node(); const c = n.get_background_color(); return [c.red, c.green, c.blue, c.alpha, n.get_border_radius(0)].join(' '); })()")"
# 0.75 of 255 is 191.25, which St rounds up
if [[ "${style}" == "0 0 0 19"[12]" $((12 * scale))" ]]; then ok "B6 ...black at 75 %, radius 12"; else not_ok "B6 ...black at 75 %, radius 12" "expected: 0 0 0 191 or 192 $((12 * scale))" "found: ${style}"; fi
wait
hidden() { [[ "$(field visible)" == false ]]; }
check "B7 the box fades away a few seconds after the sound ends" wait_for 10 hidden
between="$(ev "global._vm67opacity.filter(o => o > 0 && o < 255).length")"
if (( between > 0 )); then ok "B7 ...through ${between} steps of opacity"; else not_ok "B7 ...at once, not faded" "$(ev "JSON.stringify(global._vm67opacity)")"; fi
same "B12 ...and released once they are gone" "$(field scanoutHeld)" false
paused() { [[ "$(field playing)" == false ]]; }
check "B7 the program is paused again 10 s after the stream closed" wait_for 20 paused

# B8: nothing written, nothing sent
files="$(ls -l "/proc/${PID}/fd" 2>/dev/null | awk '{ print $NF }' | grep "^${HOME_DIR}" | grep -v "^${MODEL_DIR}" || true)"
same "B8 the program has no file of the home open but the model" "${files}" ""
same "B8 ...and no network socket" "$(ss -Htunp 2>/dev/null | grep -c "pid=${PID},")" 0

# B11: ten disable/enable cycles, one program at the end
since="$(date '+%Y-%m-%d %H:%M:%S')"
for _ in $(seq 10); do
    user_bus gnome-extensions disable "${UUID}"
    sleep 1
    user_bus gnome-extensions enable "${UUID}"
    wait_for 20 ready
done
check "B11 after ten cycles the program runs" wait_for 10 running
same "B11 ...once" "$(pgrep -u "${TEST_USER}" -fc "amethystora-captions listen")" 1
same "B11 ...one box at most" "$(ev "Main.uiGroup.get_children().filter(a => a.style_class === 'amethystora-captions-box').length")" "$(field box | sed 's/true/1/; s/false/0/')"
same "B11 no JS ERROR" "$(journalctl -b --since "${since}" _COMM=gnome-shell --no-pager -o cat | grep -c 'JS ERROR')" 0

# B9: nothing listens at the lock screen, and it listens again after
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"
locked() { [[ "$(loginctl show-session "${SESSION}" -p LockedHint --value)" == yes ]]; }
unlocked() { ! locked; }
no_helper() { ! pgrep -u "${TEST_USER}" -f "amethystora-captions listen" >/dev/null; }
loginctl lock-session "${SESSION}"
if wait_for 30 locked; then
    check "B9 locked: the program has ended" wait_for 10 no_helper
    loginctl unlock-session "${SESSION}"
    wait_for 60 unlocked && wait_for 60 ready
    check "B9 unlocked: it runs again" wait_for 20 running
else
    not_ok "B9 the session did not lock"
fi

# B10: off, then remove
PID="$(helper_pid)"
in_session "ame desktop captions off" >/dev/null 2>&1
ended() { ! kill -0 "${PID}" 2>/dev/null; }
check "B10 off: the program ends within a second" wait_for 2 ended
same "B10 ...no box, no mixer" "$(state | jq -c '[.box, .mixer]')" "[false,false]"
in_session "ame desktop captions remove" >/dev/null 2>&1
same "B10 remove: the model is gone" "$(ls -A "${HOME_DIR}/.local/share/amethystora/captions" 2>/dev/null)" ""
no_toggle() { [[ "$(field toggle)" == false ]]; }
check "B10 ...and the toggle" wait_for 5 no_toggle
same "B13 the sends row goes with the model" \
    "$(user_bus /usr/libexec/amethystora-sends --json 2>/dev/null | jq -r '[.[] | select(.id == "sends-captions")] | length')" 0
finish
