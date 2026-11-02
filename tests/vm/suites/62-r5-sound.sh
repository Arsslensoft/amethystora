#!/usr/bin/bash
# vmlab-vm: r5-sound
# Round 6's sound group (plans/ui-ideas/spec-sound.md, section 7), in the test account's session, asked of GNOME Shell
# itself through Eval, which a lab-only extension of the test account opens (never in the image), and of PipeWire with
# stand-in devices (null sinks and a null source made by pactl, which the session's PipeWire takes as real ones):
#   1  the audio and brightness modules and the power module's third part, their keys and defaults;
#   2  pause when the headphones come off: a stand-in MPRIS player playing to "headphones" (a sink of form factor
#      headphone) is paused when they go, the notice offers Play on speakers, which plays it again; picking the speakers
#      by hand while the headphones stay pauses nothing;
#   3  clear voice: ame desktop clear-voice on makes Clear voice the input in use, the microphone is not opened while
#      nothing records, the switch in the microphone's menu shows it while something does; off gives the input back;
#   4  sound presets: a preset for the output in use runs PipeWire's equaliser before it, a stream to that output goes
#      through it, the output menu's row shows it chosen and GNOME's list leaves the filter out; Flat stops it;
#   5  the sound sets: Glass, Wood and Retro beside Soft, nine sounds each, ame desktop sounds glass;
#   6  external screens' brightness: nothing given while off; on, the rule copied and the report says so; a stand-in
#      ddcutil gives one screen, which gets its slider, and moving it sets the screen;
#   7  every device's battery: a stand-in mouse at 40% in the battery menu, one notice at 15%, not two;
#   8  ten disables and enables leave nothing behind; Control and the setup file.
# vmlab-paths: @ui/modules/pill/hello.js @ui/modules/pill/levels.js @ui/modules/pill/media.js @ui/extension.js @ui/*audio* @ui/*brightness* @ui/modules/power.js @ui/modules/power/* @ui/modules/quick.js @ui/modules/quick/* @sf/usr/libexec/amethystora-sound @sf/usr/lib/systemd/user/amethystora-clear-voice.service @sf/usr/lib/systemd/user/amethystora-sound-presets.service @sf/usr/share/amethystora/sound/* @sf/usr/share/amethystora/ddc/* @sf/usr/share/sounds/* @sf/usr/libexec/amethystora-security-status @sf/usr/libexec/amethystora-security-watch @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/*just/desktop.just branding/sounds.mjs build_files/base/08-hardening.sh @ui/*pill*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
WORK=/run/vmlab-62
AUDIO=org.gnome.shell.extensions.amethystora-ui.audio
BRIGHT=org.gnome.shell.extensions.amethystora-ui.brightness
POWER=org.gnome.shell.extensions.amethystora-ui.power
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
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
AU="${UI}.get('audio')"
BR="${UI}.get('brightness')"
PW="${UI}.get('power')"
js() { ev "JSON.stringify($1)"; }

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}"
# A media player on the session bus, as an app's would be: Play and Pause, and the state it is in written to a file
cat >"${WORK}/player.py" <<'EOF'
import sys
from gi.repository import Gio, GLib
XML = '''<node>
  <interface name="org.mpris.MediaPlayer2"><property name="Identity" type="s" access="read"/></interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <method name="Play"/><method name="Pause"/>
    <property name="PlaybackStatus" type="s" access="read"/>
  </interface>
</node>'''
status = ['Playing']
out = sys.argv[1]
def write():
    with open(out, 'w') as f:
        f.write(status[0])
def call(conn, sender, path, iface, name, params, invocation):
    status[0] = {'Play': 'Playing', 'Pause': 'Paused'}.get(name, status[0])
    write()
    invocation.return_value(None)
def get(conn, sender, path, iface, name):
    return GLib.Variant('s', status[0] if name == 'PlaybackStatus' else 'VM lab player')
node = Gio.DBusNodeInfo.new_for_xml(XML)
def acquired(conn, name):
    for iface in node.interfaces:
        conn.register_object('/org/mpris/MediaPlayer2', iface, call, get, None)
Gio.bus_own_name(Gio.BusType.SESSION, 'org.mpris.MediaPlayer2.vmlab62', 0, acquired, None, None)
write()
GLib.MainLoop().run()
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell" "${WORK}"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
for schema in "${AUDIO}" "${BRIGHT}"; do
    for key in $(in_session "gsettings list-keys ${schema}"); do gs reset "${schema}" "${key}"; done
done
gs reset "${POWER}" device-batteries
loginctl unlock-sessions
systemctl restart gdm
ready() { [[ "$(ev "typeof ${AU}?.unplug?.state === 'function' && typeof ${PW}?.devices?.state === 'function'")" == true ]]; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "the audio module did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
DDC_STAND_IN=/usr/local/bin/ddcutil
cleanup() {
    for schema in "${AUDIO}" "${BRIGHT}"; do
        for key in $(in_session "gsettings list-keys ${schema}" 2>/dev/null); do gs reset "${schema}" "${key}"; done
    done
    user_bus systemctl --user stop amethystora-clear-voice amethystora-sound-presets vmlab-player vmlab-rec vmlab-play \
        >/dev/null 2>&1
    for module in $(in_session "pactl list short modules" 2>/dev/null | awk '/vmlab62/ { print $1 }'); do
        in_session "pactl unload-module ${module}" >/dev/null 2>&1
    done
    rm -f "${DDC_STAND_IN}" /etc/udev/rules.d/60-amethystora-ddc.rules /etc/modules-load.d/amethystora-ddc.conf
    rm -f "${HOME_DIR}/.local/state/amethystora/clear-voice-previous"
    loginctl unlock-sessions
}
trap cleanup EXIT

# --- 1: the modules and their keys --------------------------------------------------------------------------------
same "1 the audio module's keys" "$(gs list-keys "${AUDIO}" | sort | paste -sd ' ')" \
    "clear-voice pause-on-unplug presets sound-presets"
same "1 pausing on, clear voice and presets off" \
    "$(gs get "${AUDIO}" pause-on-unplug) $(gs get "${AUDIO}" clear-voice) $(gs get "${AUDIO}" sound-presets)" "true false false"
same "1 the brightness module's one key, off" "$(gs get "${BRIGHT}" external-screens)" false
same "1 the devices' batteries on" "$(gs get "${POWER}" device-batteries)" true
same "1 off, the brightness module made nothing" "$(js "${BR}.state().started")" false

# Stand-in devices, made by PipeWire's own pulse server: speakers, headphones and a microphone
pa() { in_session "pactl $*"; }
pa load-module module-null-sink sink_name=vmlab62-speakers "sink_properties=device.description=VM-Speakers" >/dev/null
pa load-module module-null-sink sink_name=vmlab62-phones \
    "'sink_properties=device.description=VM-Headphones device.form_factor=headphone'" >/dev/null
pa load-module module-null-sink sink_name=vmlab62-mic media.class=Audio/Source/Virtual \
    "sink_properties=device.description=VM-Mic" >/dev/null
sleep 2
pa set-default-source vmlab62-mic
player() { cat "${WORK}/player.state" 2>/dev/null; }
playing() { [[ "$(player)" == Playing ]]; }
paused() { [[ "$(player)" == Paused ]]; }
play() {
    in_session "gdbus call --session --dest org.mpris.MediaPlayer2.vmlab62 --object-path /org/mpris/MediaPlayer2 \
        --method org.mpris.MediaPlayer2.Player.Play" >/dev/null
    wait_for 10 playing
}
user_bus systemd-run --user --unit vmlab-player --collect -- /usr/bin/python3 "${WORK}/player.py" "${WORK}/player.state" \
    >/dev/null 2>&1
wait_for 20 playing

# --- 2: pause when the headphones come off ------------------------------------------------------------------------
active_is() { [[ "$(js "${AU}.unplug.state().active?.headphones ?? null")" == "$1" ]]; }
pa set-default-sink vmlab62-phones
if wait_for 20 active_is true; then
    ok "2 the stand-in headphones are seen as headphones"
    PHONES_MODULE="$(pa list short modules | awk '/sink_name=vmlab62-phones/ { print $1 }')"
    pa unload-module "${PHONES_MODULE}"
    check "2 the player paused when the headphones went" wait_for 15 paused
    notice() { [[ "$(js "${AU}.unplug.state().notice?.title ?? null")" == "\"$1\"" ]]; }
    check "2 the notice says so" wait_for 10 notice "Your headphones went away"
    same "2 it offers to play on speakers" "$(js "${AU}.unplug.state().notice.actions")" '["Play on speakers"]'
    ev "${AU}.unplug.play(); 'played'" >/dev/null
    check "2 Play on speakers plays it again" wait_for 10 playing
    # Back, and the speakers picked by hand while the headphones stay: nothing pauses
    pa load-module module-null-sink sink_name=vmlab62-phones \
        "'sink_properties=device.description=VM-Headphones device.form_factor=headphone'" >/dev/null
    sleep 2
    pa set-default-sink vmlab62-phones
    wait_for 20 active_is true
    pa set-default-sink vmlab62-speakers
    sleep 4
    check "2 picking the speakers by hand pauses nothing" playing
else
    not_ok "2 the stand-in headphones were not seen as headphones" "$(js "${AU}.unplug.state()")"
fi
gs set "${AUDIO}" pause-on-unplug false
same "2 off, the part is stopped" "$(js "${AU}.unplug === null")" true
gs reset "${AUDIO}" pause-on-unplug

# --- 3: clear voice -----------------------------------------------------------------------------------------------
in_session "ame desktop clear-voice on" >/dev/null 2>&1
same "3 Clear voice is the input in use" "$(pa get-default-source)" amethystora-clear-voice
same "3 its key is on" "$(gs get "${AUDIO}" clear-voice)" true
mic_state() { pa list short sources | awk '$2 == "vmlab62-mic" { print $NF }'; }
case "$(mic_state)" in
    RUNNING) not_ok "3 the microphone runs while nothing records" ;;
    *) ok "3 the microphone is not opened while nothing records" ;;
esac
user_bus systemd-run --user --unit vmlab-rec --collect -- /usr/bin/parecord --device=amethystora-clear-voice /dev/null \
    >/dev/null 2>&1
mic_running() { [[ "$(mic_state)" == RUNNING ]]; }
check "3 recording from Clear voice opens the microphone" wait_for 15 mic_running
input_shown() { [[ "$(ev "Main.panel.statusArea.quickSettings._volumeInput._input.visible")" == true ]]; }
if wait_for 15 input_shown; then
    ev "Main.panel.statusArea.quickSettings.menu.open(); Main.panel.statusArea.quickSettings._volumeInput._input.menu.open(); 'open'" >/dev/null
    sleep 2
    same "3 the microphone's menu has the switch, on" "$(js "[${AU}.voice.state().shown, ${AU}.voice.state().switch]")" "[true,true]"
    ev "${AU}.voice.toggle(); 'toggled'" >/dev/null
    key_off() { [[ "$(gs get "${AUDIO}" clear-voice)" == false ]]; }
    check "3 the switch turns it off" wait_for 15 key_off
    ev "Main.panel.closeQuickSettings(); 'closed'" >/dev/null
else
    not_ok "3 GNOME's microphone slider did not show while recording"
    in_session "ame desktop clear-voice off" >/dev/null 2>&1
fi
user_bus systemctl --user stop vmlab-rec >/dev/null 2>&1
same "3 off gives the input back" "$(pa get-default-source)" vmlab62-mic
voice_inactive() { [[ "$(user_bus systemctl --user is-active amethystora-clear-voice)" != active ]]; }
check "3 off stops its unit" wait_for 10 voice_inactive

# --- 4: sound presets ---------------------------------------------------------------------------------------------
pa set-default-sink vmlab62-speakers
in_session "ame desktop sound-presets on" >/dev/null 2>&1
in_session "ame desktop sound-preset laptop" >/dev/null 2>&1
presets_active() { [[ "$(user_bus systemctl --user is-active amethystora-sound-presets)" == active ]]; }
check "4 a preset runs the presets' unit" wait_for 15 presets_active
same "4 the key names the output" "$(gs get "${AUDIO}" presets)" "{'vmlab62-speakers': 'laptop'}"
filter_there() { pa list short sinks | awk '{ print $2 }' | grep -qx amethystora-preset-1; }
check "4 the filter is there" wait_for 15 filter_there
same "4 the output in use stays the speakers" "$(pa get-default-sink)" vmlab62-speakers
user_bus systemd-run --user --unit vmlab-play --collect -- /usr/bin/pacat --playback --device=vmlab62-speakers /dev/zero \
    >/dev/null 2>&1
through() { user_bus pw-link -l | grep -A1 '^vmlab62-speakers:playback_FL' | grep -q 'amethystora-preset-1.output'; }
check "4 a stream to the speakers goes through the preset" wait_for 15 through
user_bus systemctl --user stop vmlab-play >/dev/null 2>&1
ev "Main.panel.statusArea.quickSettings.menu.open(); Main.panel.statusArea.quickSettings._volumeOutput._output.menu.open(); 'open'" >/dev/null
sleep 2
same "4 the output menu shows Laptop chosen" \
    "$(js "${AU}.presets.state().buttons.filter(b => b.checked).map(b => b.id)")" '["laptop"]'
same "4 the menu opens with one output too" "$(js "${AU}.presets.state().menuEnabled")" true
hidden="$(js "${AU}.presets.state().hidden")"
[[ "${hidden}" -ge 1 ]] && ok "4 GNOME's output list leaves the filter out" || not_ok "4 the filter is listed as an output" "${hidden}"
same "4 the curve read the presets" "$(js "${AU}.presets.state().curveRead")" true
ev "Main.panel.closeQuickSettings(); 'closed'" >/dev/null
in_session "ame desktop sound-preset flat" >/dev/null 2>&1
presets_inactive() { [[ "$(user_bus systemctl --user is-active amethystora-sound-presets)" != active ]]; }
check "4 Flat stops the unit" wait_for 15 presets_inactive
in_session "ame desktop sound-presets off" >/dev/null 2>&1

# --- 5: the sound sets --------------------------------------------------------------------------------------------
for set in amethystora amethystora-glass amethystora-wood amethystora-retro; do
    same "5 ${set} has nine sounds" "$(find "/usr/share/sounds/${set}/stereo" -name '*.wav' | wc -l)" 9
done
in_session "ame desktop sounds glass" >/dev/null
same "5 ame desktop sounds glass" "$(gs get org.gnome.desktop.sound theme-name)" "'amethystora-glass'"
in_session "ame desktop sounds soft" >/dev/null

# --- 6: external screens' brightness -------------------------------------------------------------------------------
check "6 off, no rule gives the i2c buses" test ! -e /etc/udev/rules.d/60-amethystora-ddc.rules
check "6 off, nothing runs ddcutil" test -z "$(pgrep -x ddcutil)"
GSETTINGS_BACKEND=memory ame desktop screen-brightness on >/dev/null 2>&1
check "6 on copies the image's rule" cmp /etc/udev/rules.d/60-amethystora-ddc.rules /usr/share/amethystora/ddc/60-amethystora-ddc.rules
check "6 on loads i2c-dev" test -d /sys/module/i2c_dev
check "6 the report says the grant" bash -c '/usr/libexec/amethystora-security-status --json | grep -Eq "\"id\": *\"screen-brightness\""'
GSETTINGS_BACKEND=memory ame desktop screen-brightness off >/dev/null 2>&1
check "6 off takes it back" test ! -e /etc/udev/rules.d/60-amethystora-ddc.rules
# A stand-in ddcutil with one screen, which logs what it is asked to set
cat >"${DDC_STAND_IN}" <<'EOF'
#!/usr/bin/bash
case " $* " in
    *" detect "*) printf 'Display 1\n   I2C bus:  /dev/i2c-7\n   DRM connector:      card1-Virtual-1\n   Monitor:             VML:Lab Screen:62\n\n' ;;
    *" getvcp "*) echo "VCP 10 C 40 100" ;;
    *" setvcp "*) echo "$*" >>/run/vmlab-62/ddc.log ;;
esac
EOF
chmod 0755 "${DDC_STAND_IN}"
gs set "${BRIGHT}" external-screens true
ev "Main.panel.statusArea.quickSettings.menu.open(); 'open'" >/dev/null
one_screen() { [[ "$(js "${BR}.state().screens.length")" == 1 && "$(js "${BR}.state().visible")" == true ]]; }
check "6 the screen gets its slider" wait_for 20 one_screen
same "6 at the brightness the screen says" "$(js "${BR}.state().screens[0].value")" 40
ev "${BR}.setForTest(0, 0.7); 'moved'" >/dev/null
set_70() { grep -q -- "--bus 7 --noverify setvcp 10 70" "${WORK}/ddc.log"; }
check "6 moving it sets the screen" wait_for 10 set_70
ev "Main.panel.closeQuickSettings(); 'closed'" >/dev/null
gs set "${BRIGHT}" external-screens false
same "6 off, the block is gone" "$(js "${BR}.state().started")" false
rm -f "${DDC_STAND_IN}"

# --- 7: every device's battery ------------------------------------------------------------------------------------
before="$(ev "Main.messageTray.getSources().flatMap(s => s.notifications).filter(n => /battery at/.test(n.title)).length")"
same "7 a mouse at 40% is listed" "$(js "${PW}.devices.readingForTest({percentage: 40, model: 'Lab Mouse'}).devices.map(d => d.label + ' ' + d.percent)")" '["Mouse 40"]'
ev "${PW}.devices.readingForTest({percentage: 14, model: 'Lab Mouse'}); 'low'" >/dev/null
ev "${PW}.devices.readingForTest({percentage: 13, model: 'Lab Mouse'}); 'lower'" >/dev/null
after="$(ev "Main.messageTray.getSources().flatMap(s => s.notifications).filter(n => /battery at/.test(n.title)).length")"
same "7 one notice at 15% or less, not two" "$((after - before))" 1
battery_shown() { [[ "$(ev "Main.panel.statusArea.quickSettings.menu._grid.get_children().some(a => a._amethystoraQuickId === 'battery' && a.visible)")" == true ]]; }
check "7 the battery tile shows for the devices" battery_shown
ev "Main.panel.statusArea.quickSettings.menu.open(); ${UI}.get('quick').batteryMenu().open(); 'open'" >/dev/null
sleep 2
same "7 the battery menu lists the mouse" "$(js "${PW}.devices.state().open")" true
ev "Main.panel.closeQuickSettings(); 'closed'" >/dev/null
ev "${PW}.devices.forgetTest(); 'gone'" >/dev/null

# --- 8: ten disables and enables, Control and the setup file -------------------------------------------------------
gs set "${BRIGHT}" external-screens true
for _ in $(seq 10); do
    ev "Main.extensionManager.disableExtension('${UUID}'); Main.extensionManager.enableExtension('${UUID}'); 'again'" >/dev/null
done
wait_for 30 ready
same "8 after ten restarts, one screens block" \
    "$(ev "Main.panel.statusArea.quickSettings.menu._grid.get_children().filter(a => a._amethystoraQuickId === 'screens').length")" 1
ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
ui_off() { [[ "$(ev "Main.extensionManager.lookup('${UUID}').state")" == 2 ]]; }
wait_for 30 ui_off
same "8 off, GNOME's sliders are as GNOME made them" \
    "$(ev "[Main.panel.statusArea.quickSettings._volumeOutput._output, Main.panel.statusArea.quickSettings._volumeInput._input].some(s => Object.hasOwn(s, '_sync'))")" false
same "8 off, no screens block is left" \
    "$(ev "Main.panel.statusArea.quickSettings.menu._grid.get_children().filter(a => a._amethystoraQuickId === 'screens').length")" 0
ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
wait_for 30 ready
gs set "${BRIGHT}" external-screens false
entries="$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -r '[.items[] | select(.id | IN("pause-on-unplug",
    "clear-voice", "sound-presets", "sound-preset", "screen-brightness", "device-batteries")) | .id] | sort | join(" ")')"
same "8 Control lists the six" "${entries}" \
    "clear-voice device-batteries pause-on-unplug screen-brightness sound-preset sound-presets"
in_session "ame desktop pause-on-unplug off" >/dev/null
same "8 ame setup save carries pausing" "$(in_session "ame setup save -" 2>/dev/null | jq -c .settings.audio)" '{"pause-on-unplug":false}'
in_session "ame desktop pause-on-unplug on" >/dev/null
printf '%s\n' '{"version": 1, "settings": {"audio": {"presets": {"x": "bass"}}}}' >"${WORK}/bad.json"
chown "${TEST_USER}:" "${WORK}/bad.json"
if in_session "ame setup diff ${WORK}/bad.json" >/dev/null 2>&1; then
    not_ok "8 the setup file took the presets, which name this machine's outputs"
else
    ok "8 the setup file refuses the presets"
fi
finish
