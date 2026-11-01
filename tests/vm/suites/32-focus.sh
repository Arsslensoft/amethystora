#!/usr/bin/bash
# vmlab-vm: focus
# Focus profiles (plans/beyond/spec-focus.md, section 8), in the test account's session, asked of GNOME Shell itself
# through Eval, which a lab-only extension of the test account opens (never in the image):
#   - the schema, and Off costing nothing; the Focus tile in quick settings and the profile's mark in the pill;
#   - Work holding banners but what passes, and saying how many wait when it ends;
#   - Games: the dock's autohide laid over its key, a performance hold with tuned-ppd, Night Light paused;
#   - Sleep: Night Light's two keys written and put back, the amber accent, the night picture, a power-saver hold;
#   - Night Light put back at the next start after GNOME Shell was killed with Sleep on;
#   - an end after an hour, the two triggers, Off setting a trigger aside, a profile picked by hand winning;
#   - editing a profile while it is on, ten disables and enables mid-profile, Control and the setup file;
#   - a real lock and unlock: Sleep's Night Light as the person left it, the hold taken again, Off kept;
#   - Steam: its own window is no game, a game it starts (class steam_app_N) is, and ends when that window closes.
# vmlab-paths: @ui/extension.js @ui/*focus* @sf/*themed/shell-focus* @sf/usr/libexec/amethystora-setup* @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
FOCUS=org.gnome.shell.extensions.amethystora-ui.focus
DOCK=org.gnome.shell.extensions.amethystora-ui.dock
COLOR=org.gnome.settings-daemon.plugins.color
STATE="${HOME_DIR}/.local/state/amethystora"
# Eval's answer decoded: GNOME Shell gives the result as JSON inside a GVariant; a string comes back as it is (the
# JSON a script built with JSON.stringify), anything else as JSON
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
F="${UI}.get('focus')"
# One field of the focus module's state, as JSON
st() { ev "JSON.stringify(${F}.state()$1)"; }
profile_is() { [[ "$(st .profile)" == "\"$1\"" ]]; }
pill_activity() { ev "${UI}.get('pill').state().activity"; }
holds() {
    gdbus call --system --dest org.freedesktop.UPower.PowerProfiles --object-path /org/freedesktop/UPower/PowerProfiles \
        --method org.freedesktop.DBus.Properties.Get org.freedesktop.UPower.PowerProfiles ActiveProfileHolds 2>/dev/null
}
paused() {
    user_bus gdbus call --session --dest org.gnome.SettingsDaemon.Color --object-path /org/gnome/SettingsDaemon/Color \
        --method org.freedesktop.DBus.Properties.Get org.gnome.SettingsDaemon.Color DisabledUntilTomorrow 2>/dev/null
}
has_performance() {
    gdbus call --system --dest org.freedesktop.UPower.PowerProfiles --object-path /org/freedesktop/UPower/PowerProfiles \
        --method org.freedesktop.DBus.Properties.Get org.freedesktop.UPower.PowerProfiles Profiles 2>/dev/null |
        grep -q "'performance'"
}
pick() { ev "${F}.pick('$1'); 'picked'" >/dev/null; sleep 1; }
holds_have() { [[ "$(holds)" == *"$1"* ]]; }
holds_lack() { [[ "$(holds)" != *"$1"* ]]; }
paused_is() { [[ "$(paused)" == "(<$1>,)" ]]; }
like() { [[ "$1" == $2 ]]; }
no_record() { ! grep -q '"night"' "${STATE}/focus.json"; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

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
# A game and a tool, each a window under its own app id with a launcher saying what it is
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/vmlab-focus" "${HOME_DIR}/.local/share/applications"
cat >"${HOME_DIR}/.local/share/vmlab-focus/win.js" <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: ARGV[0]});
app.connect('activate', () => new Gtk.ApplicationWindow({application: app, title: ARGV[0], default_width: 640, default_height: 400}).present());
app.run([]);
EOF
printf '[Desktop Entry]\nType=Application\nName=Test Game\nExec=gjs -m %s org.vmlab.Game\nCategories=Game;ArcadeGame;\n' \
    "${HOME_DIR}/.local/share/vmlab-focus/win.js" >"${HOME_DIR}/.local/share/applications/org.vmlab.Game.desktop"
printf '[Desktop Entry]\nType=Application\nName=Test Tool\nExec=gjs -m %s org.vmlab.Tool\nCategories=Utility;\n' \
    "${HOME_DIR}/.local/share/vmlab-focus/win.js" >"${HOME_DIR}/.local/share/applications/org.vmlab.Tool.desktop"
printf '[Desktop Entry]\nType=Application\nName=Test Chat\nExec=true\nCategories=Network;\n' \
    >"${HOME_DIR}/.local/share/applications/org.vmlab.Chat.desktop"
# Steam's own launcher as Valve writes it (a store that says Game), and a window of a game Steam starts: no app of its
# own, only its class
printf '[Desktop Entry]\nType=Application\nName=Steam\nExec=gjs -m %s com.valvesoftware.Steam\nCategories=Network;FileTransfer;Game;\n' \
    "${HOME_DIR}/.local/share/vmlab-focus/win.js" >"${HOME_DIR}/.local/share/applications/com.valvesoftware.Steam.desktop"
cat >"${HOME_DIR}/.local/share/vmlab-focus/steamgame.js" <<'EOF'
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
GLib.set_prgname(ARGV[0]);
Gtk.init();
const loop = new GLib.MainLoop(null, false);
const window = new Gtk.Window({title: ARGV[0], default_width: 640, default_height: 400});
window.connect('close-request', () => { loop.quit(); return false; });
window.present();
loop.run();
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
for key in $(in_session "gsettings list-keys ${FOCUS}"); do gs reset "${FOCUS}" "${key}"; done
loginctl unlock-sessions
systemctl restart gdm
focus_ready() { [[ "$(ev "typeof ${F}?.state")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 focus_ready; then
    not_ok "the focus module did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
NIGHT_BEFORE="$(gs get "${COLOR}" night-light-enabled) $(gs get "${COLOR}" night-light-temperature)"
SCHEDULE="$(for k in automatic from to; do gs get "${COLOR}" "night-light-schedule-${k}"; done | paste -sd ' ')"
cleanup() {
    for key in $(in_session "gsettings list-keys ${FOCUS}" 2>/dev/null); do gs reset "${FOCUS}" "${key}"; done
    read -r automatic from to <<<"${SCHEDULE}"
    gs set "${COLOR}" night-light-schedule-automatic "${automatic}"
    gs set "${COLOR}" night-light-schedule-from "${from}"
    gs set "${COLOR}" night-light-schedule-to "${to}"
    user_bus systemctl --user stop vmlab-game vmlab-tool vmlab-steam vmlab-steamgame >/dev/null 2>&1
    rm -f "${HOME_DIR}/.local/share/applications/com.valvesoftware.Steam.desktop"
    # Night Light and its pause as they were, whether or not GNOME Shell is there to put them back
    read -r enabled _ temperature <<<"${NIGHT_BEFORE}"
    gs set "${COLOR}" night-light-enabled "${enabled}"
    gs set "${COLOR}" night-light-temperature "${temperature}"
    user_bus gdbus call --session --dest org.gnome.SettingsDaemon.Color --object-path /org/gnome/SettingsDaemon/Color \
        --method org.freedesktop.DBus.Properties.Set org.gnome.SettingsDaemon.Color DisabledUntilTomorrow '<false>' >/dev/null 2>&1
}
trap cleanup EXIT

# --- 1-2: the schema, and Off costs nothing --------------------------------------------------------------------------
check "1 the focus module runs" test "$(ev "${UI}.has('focus')")" == true
same "1 the schema has its 38 keys" "$(in_session "gsettings list-keys ${FOCUS}" | wc -l)" 38
same "1 nothing picked, both triggers off" "$(gs get "${FOCUS}" profile) $(gs get "${FOCUS}" trigger-games) $(gs get "${FOCUS}" trigger-sleep)" \
    "'off' false false"
same "2 off: no timer, trigger, power connection, picture or grey" \
    "$(st | python3 -c 'import json,sys; s=json.load(sys.stdin); print(s["profile"], s["timers"], s["triggers"], s["power"], s["picture"], s["grey"])')" \
    "off {'end': False, 'night': False} {'games': None, 'sleep': None} {'held': None, 'connected': False} 0 False"

# --- 3: the tile ---------------------------------------------------------------------------------------------------------
ev "Main.panel.statusArea.quickSettings.menu.open(); 'open'" >/dev/null
sleep 1
tile="$(ev "JSON.stringify(${F}.tile.snapshot())")"
same "3 the tile: Off chosen, its state, one line" "$(python3 -c 'import json,sys; t=json.loads(sys.argv[1]); print(t["chosen"], t["state"], len(t["lines"]))' "${tile}")" \
    "off Off 1"
# Two tiles and the gap (378 px with GNOME's own quick settings at scale 1), whatever width the menu has
same "3 the tile is two tiles and the gap wide" \
    "$(ev "(() => { const g = Main.panel.statusArea.quickSettings.menu._grid; const t = g.get_children().find(a => a.constructor.name === 'NightLightToggle' || a.constructor.name === 'DarkModeToggle'); return Math.abs(${F}.tile.block.width - (2 * t.width + g.layout_manager.column_spacing)) < 1 ? 'yes' : ${F}.tile.block.width + ' vs ' + t.width + ' + ' + g.layout_manager.column_spacing; })()")" \
    yes
same "3 the tile comes after the tiles and before the brightness slider" \
    "$(ev "(() => { const g = Main.panel.statusArea.quickSettings.menu._grid; const c = g.get_children(); const i = c.indexOf(${F}.tile.block); const b = c.findIndex(a => a.constructor.name === 'BrightnessItem'); const t = c.findIndex(a => a.constructor.name === 'NightLightToggle' || a.constructor.name === 'DarkModeToggle'); return (i > t) + ' ' + (b < 0 || i < b); })()")" \
    "true true"

# --- 4-6: Work ------------------------------------------------------------------------------------------------------
ev "${F}.tile.click('work'); 'clicked'" >/dev/null
sleep 1
same "4 a click on Work picks it by hand" "$(gs get "${FOCUS}" profile) $(st .origin)" "'work' \"hand\""
same "4 three lines, as the board's" "$(ev "${F}.tile.snapshot().lines.length")" 3
same "4 the pill shows the mark" "$(pill_activity)" focus
check "4 the mark reads Work · the time" like "$(ev "${UI}.get('pill').state().text")" 'Work · [0-9]*'
ev "Main.panel.statusArea.quickSettings.menu.close(); 'closed'" >/dev/null
# What the session showed before (GNOME's "System was put in unsafe mode", for the lab's Eval) is let go: with nobody
# at the keyboard GNOME never takes a banner away, so it would stand in front of every one below
ev "Main.messageTray.getSources().forEach(s => [...s.notifications].forEach(n => n.destroy())); 'cleared'" >/dev/null
sleep 1
in_session "notify-send 'vmlab focus held'" >/dev/null
sleep 1
same "5 an ordinary banner is held under Work" "$(ev "${UI}.get('pill').hold.focusHeld")" 1
same "5 and is not shown" "$(ev "Main.messageTray._notification?.title ?? 'none'")" none
# The banner shown now, by its title; GNOME never takes an urgent one away by itself, so the urgent one comes last
banner_is() { [[ "$(ev "Main.messageTray._notification?.title ?? 'none'")" == "$1" ]]; }
in_session "ame desktop focus edit work apps org.vmlab.Chat.desktop" >/dev/null
sleep 1
# Kept on the bus until its banner is closed: GNOME destroys an app's source, banner and all, as soon as its sender
# leaves the bus, which a bare notify-send does at once (only a source without an app is spared)
in_session "timeout 30 notify-send --wait --hint=string:desktop-entry:org.vmlab.Chat 'vmlab focus chat'" >/dev/null 2>&1 &
check "5 a banner from an app Work lets through shows" wait_for 5 banner_is "vmlab focus chat"
# Nobody at the keyboard: GNOME would hold the chat's banner up for ever, so it is closed by hand
ev "Main.messageTray._notification?.destroy(); 'closed'" >/dev/null
sleep 1
in_session "notify-send -u critical 'vmlab focus urgent'" >/dev/null
check "5 an urgent one comes through" wait_for 15 banner_is "vmlab focus urgent"
ev "Main.messageTray._notification?.destroy(); 'closed'" >/dev/null
gs reset "${FOCUS}" work-apps
sleep 1
same "5 a listed app passes, others wait, urgent ones keep their sound" \
    "$(ev "(() => { const h = ${UI}.get('pill').hold; const n = (app, urgency = 1) => ({urgency, forFeedback: false, source: {app: {get_id: () => app}}}); return [h._lets(n('org.gnome.Calendar.desktop')), h._lets(n('org.vmlab.Other.desktop')), h._silenced(n('org.gnome.Calendar.desktop')), h._silenced(n('x', 3))].join(' '); })()")" \
    "true false true false"
ev "${F}.pick('off'); 'picked'" >/dev/null
sleep 0.5
same "6 Off: the pill says once how many wait" "$(pill_activity) $(ev "${UI}.get('pill').state().text")" "waiting 1 waiting in the list"
sleep 5
same "6 what Work held is in the list" \
    "$(ev "Main.messageTray.getSources().flatMap(s => s.notifications).some(n => n.title === 'vmlab focus held')")" true

# --- 7: Games -------------------------------------------------------------------------------------------------------
pick games
same "7 the dock hides while a window touches it, its key untouched" "$(ev "${UI}.get('dock').autohide") $(gs get "${DOCK}" autohide)" \
    "touch 'off'"
if has_performance; then
    check "7 a performance hold with the power daemon" holds_have "'performance'>, 'Reason': <'Focus: Games'"
else
    skip "7 this machine has no performance mode, so Games asks for no hold"
fi
check "7 Night Light paused through the daemon" paused_is true
pick off
check "7 Off: no hold left" holds_lack Focus
check "7 Off: Night Light no longer paused" paused_is false
same "7 Off: the dock as set" "$(ev "${UI}.get('dock').autohide")" off

# --- 8: Sleep -------------------------------------------------------------------------------------------------------
pick sleep
accent_on() { [[ "$(ev "Main.uiGroup.has_style_class_name('amethystora-accent-amber')")" == true ]]; }
check "8 the amber accent is on the top group within 20 s" wait_for 20 accent_on
same "8 Night Light on at its warmest" "$(gs get "${COLOR}" night-light-enabled) $(gs get "${COLOR}" night-light-temperature)" \
    "true uint32 1700"
check "8 the state file holds what Night Light was" grep -q '"before"' "${STATE}/focus.json"
same "8 the apps follow the amber accent" "$(cat "${STATE}/workspace-accent" 2>/dev/null)" amber
same "8 the night picture on every screen" "$(st .picture)" "$(ev "Main.layoutManager.monitors.length")"
check "8 a power-saver hold" holds_have "'power-saver'"
pick off
same "8 Off: Night Light as it was" "$(gs get "${COLOR}" night-light-enabled) $(gs get "${COLOR}" night-light-temperature)" \
    "${NIGHT_BEFORE}"
check "8 Off: the record is gone" no_record
same "8 Off: no accent class, no picture" "$(ev "Main.uiGroup.has_style_class_name('amethystora-accent-amber')") $(st .picture)" \
    "false 0"
pick sleep
gs set "${COLOR}" night-light-temperature 2000
pick off
same "8 a value changed by hand during Sleep stays" "$(gs get "${COLOR}" night-light-temperature)" "uint32 2000"
read -r _ temperature <<<"${NIGHT_BEFORE}"
gs set "${COLOR}" night-light-temperature "${temperature#uint32 }"

# --- 9: a crash with Sleep on ------------------------------------------------------------------------------------------
pick sleep
# It ends five seconds from now, while GNOME Shell is down: the next start finds it over and puts Night Light back
gs set "${FOCUS}" ends "$(($(date +%s) + 5))"
# and the state file is broken: its copy stands in
printf '{' >"${STATE}/focus.json"
# A shell that dies within a minute of starting has GNOME turn every user extension off (its flag file and
# org.gnome.Shell-disable-extensions.service), which would leave every later suite without Amethystora UI: this step
# is about Focus after a crash, not GNOME's safe mode, so the flag goes first and the key is put back after
rm -f "/run/user/$(id -u "${TEST_USER}")/gnome-shell-disable-extensions"
pkill -9 -u "${TEST_USER}" -x gnome-shell
sleep 8
loginctl unlock-sessions
systemctl restart gdm
wait_for 240 has_session && user_bus gsettings set org.gnome.shell disable-user-extensions false
if wait_for 240 has_session && wait_for 120 focus_ready; then
    sleep 5
    same "9 after the crash: the profile is over" "$(gs get "${FOCUS}" profile)" "'off'"
    same "9 after the crash: Night Light as it was" \
        "$(gs get "${COLOR}" night-light-enabled) $(gs get "${COLOR}" night-light-temperature)" "${NIGHT_BEFORE}"
    check "9 after the crash: the record is gone" no_record
    check "9 the daemon let the dead shell's hold go" holds_lack Focus
else
    not_ok "9 the session did not come back after GNOME Shell was killed"
    finish
fi
ev "Main.overview.hide(); 'hidden'" >/dev/null

# --- 10: an end ---------------------------------------------------------------------------------------------------------
in_session "ame desktop focus work hour" >/dev/null
sleep 1
left=$(($(gs get "${FOCUS}" ends | awk '{print $NF}') - $(date +%s)))
check "10 ends about an hour from now" test "${left}" -ge 3590 -a "${left}" -le 3600
check "10 the tile says until when" like "$(ev "${F}.tile.snapshot().state")" 'On until *'
gs set "${FOCUS}" ends "$(($(date +%s) + 2))"
check "10 it ends at its time" wait_for 5 profile_is off

# --- 11: Sleep at Night Light's times ---------------------------------------------------------------------------------
hour=$((10#$(date +%H)))
gs set "${COLOR}" night-light-schedule-automatic false
gs set "${COLOR}" night-light-schedule-from "$(((hour + 23) % 24)).0"
gs set "${COLOR}" night-light-schedule-to "$(((hour + 2) % 24)).0"
in_session "ame desktop focus trigger sleep on" >/dev/null
check "11 Sleep starts at Night Light's times" wait_for 3 profile_is sleep
same "11 by its trigger, one timeout to the next boundary" "$(st .origin) $(st .timers)" '"night" {"end":false,"night":true}'
in_session "ame desktop focus off" >/dev/null
check "11 Off by hand ends it" wait_for 3 profile_is off
sleep 2
same "11 and it waits for its next start" "$(st .profile)" '"off"'
# Its next start, after the Off: the span starts again at the next minute, by the trigger's own timeout
read -r next_h next_m <<<"$(date -d '+1 minute' '+%H %M')"
gs set "${COLOR}" night-light-schedule-from "$(awk -v h="$((10#${next_h}))" -v m="$((10#${next_m}))" 'BEGIN { printf "%.6f", h + m / 60 }')"
check "11 at its next start it is on again" wait_for 70 profile_is sleep
in_session "ame desktop focus work" >/dev/null
check "13 a profile picked by hand wins" wait_for 3 profile_is work
in_session "ame desktop focus off" >/dev/null
in_session "ame desktop focus trigger sleep off" >/dev/null
check "11 trigger off: nothing on" wait_for 3 profile_is off

# --- 12: Games while a game is in front ------------------------------------------------------------------------------
in_session "ame desktop focus trigger games on" >/dev/null
# The tool alone first: the game's window, once open, has the keyboard and starts Games before any click
user_bus systemd-run --user --collect --unit=vmlab-tool gjs -m "${HOME_DIR}/.local/share/vmlab-focus/win.js" org.vmlab.Tool >/dev/null
win_ready() { [[ "$(ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class()?.startsWith('org.vmlab')).length")" == "$1" ]]; }
wait_for 30 win_ready 1
activate() { ev "global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === '$1').activate(global.get_current_time()); 'ok'" >/dev/null; sleep 1; }
activate org.vmlab.Tool
same "12 a tool in front starts nothing" "$(st .profile)" '"off"'
user_bus systemd-run --user --collect --unit=vmlab-game gjs -m "${HOME_DIR}/.local/share/vmlab-focus/win.js" org.vmlab.Game >/dev/null
wait_for 30 win_ready 2
activate org.vmlab.Game
same "12 a game in front starts Games" "$(st .profile) $(st .origin)" '"games" "game"'
activate org.vmlab.Tool
same "12 another app in front keeps it" "$(st .profile)" '"games"'
user_bus systemctl --user stop vmlab-game >/dev/null 2>&1
check "12 the game closing ends it" wait_for 5 profile_is off
user_bus systemd-run --user --collect --unit=vmlab-steam gjs -m "${HOME_DIR}/.local/share/vmlab-focus/win.js" com.valvesoftware.Steam >/dev/null
steam_ready() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'com.valvesoftware.Steam')")" == true ]]; }
wait_for 30 steam_ready
activate com.valvesoftware.Steam
same "12 Steam's own window starts nothing, though its launcher says Game" "$(st .profile)" '"off"'
user_bus systemd-run --user --collect --unit=vmlab-steamgame gjs -m "${HOME_DIR}/.local/share/vmlab-focus/steamgame.js" steam_app_1 >/dev/null
steamgame_ready() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'steam_app_1')")" == true ]]; }
wait_for 60 steamgame_ready
activate steam_app_1
same "12 a game Steam starts is followed by its window's class" "$(st .profile) $(st .triggers.games.game)" '"games" "class:steam_app_1"'
activate com.valvesoftware.Steam
same "12 back in Steam's library, Games stays" "$(st .profile)" '"games"'
user_bus systemctl --user stop vmlab-steamgame >/dev/null 2>&1
check "12 the game's window closing ends it, Steam still open" wait_for 5 profile_is off
in_session "ame desktop focus trigger games off" >/dev/null
user_bus systemctl --user stop vmlab-tool vmlab-steam >/dev/null 2>&1

# --- 14: editing a profile ------------------------------------------------------------------------------------------------
pick work
in_session "ame desktop focus edit work power power-saver" >/dev/null
sleep 2
check "14 an edit applies at once: a power-saver hold under Work" holds_have "'power-saver'>, 'Reason': <'Focus: Work'"
in_session "ame desktop focus edit work reset" >/dev/null
sleep 2
check "14 reset: Work holds nothing again" holds_lack Focus
in_session "ame desktop focus edit work power turbo" >/dev/null 2>&1
same "14 a wrong value is refused" "$?" 64
pick off

# --- 15: ten disables and enables while Sleep is on ---------------------------------------------------------------------
pick sleep
sleep 2
CYCLE="(async () => { const uuid = '${UUID}'; const MT = await import('resource:///org/gnome/shell/ui/messageTray.js');
    const count = () => Main.uiGroup.get_n_children() + global.window_group.get_n_children();
    Main.extensionManager.disableExtension(uuid); await new Promise(r => setTimeout(r, 500)); const before = count();
    const originals = [MT.MessageTray.prototype._showNotification, MT.Notification.prototype.playSound];
    for (let i = 0; i < 10; i++) { Main.extensionManager.enableExtension(uuid); await new Promise(r => setTimeout(r, 500)); Main.extensionManager.disableExtension(uuid); await new Promise(r => setTimeout(r, 500)); }
    const after = count(); const effects = Main.layoutManager.uiGroup.get_effects().length + global.stage.get_effects().length;
    const methods = MT.MessageTray.prototype._showNotification === originals[0] && MT.Notification.prototype.playSound === originals[1];
    Main.extensionManager.enableExtension(uuid);
    globalThis.vmlabFocusCycle = (after - before) + ' ' + effects + ' ' + methods; })(); 'started'"
ev "${CYCLE}" >/dev/null
cycle_done() { [[ -n "$(ev "globalThis.vmlabFocusCycle ?? ''")" ]]; }
wait_for 60 cycle_done
same "15 nothing left in the shell, GNOME's methods back" "$(ev "globalThis.vmlabFocusCycle")" "0 0 true"
wait_for 30 focus_ready
sleep 2
same "15 Sleep on again after them, Night Light warm" "$(st .profile) $(gs get "${COLOR}" night-light-temperature)" \
    '"sleep" uint32 1700'
same "15 one hold, not eleven" "$(holds | grep -o 'Focus: Sleep' | wc -l)" 1

# --- 15b: a real lock and unlock ------------------------------------------------------------------------------------
locked() { [[ "$(ev "Main.screenShield?.locked ?? false")" == true ]]; }
unlocked() { [[ "$(ev "Main.sessionMode.currentMode")" == user ]] && focus_ready; }
lock_and_unlock() {
    loginctl lock-session "$(loginctl show-user "${TEST_USER}" -p Display --value)"
    wait_for 30 locked || return 1
    sleep 2
    loginctl unlock-sessions
    wait_for 30 unlocked
}
gs set "${COLOR}" night-light-temperature 2000
if lock_and_unlock; then
    sleep 3
    same "15b after a lock: Sleep on, Night Light as the person left it" \
        "$(st .profile) $(gs get "${COLOR}" night-light-enabled) $(gs get "${COLOR}" night-light-temperature)" \
        '"sleep" true uint32 2000'
    check "15b after a lock: the record is still there" grep -q '"night"' "${STATE}/focus.json"
    same "15b after a lock: one hold, taken again" "$(holds | grep -o 'Focus: Sleep' | wc -l)" 1
else
    not_ok "15b the screen did not lock and unlock"
fi
pick off
same "15b Off after the lock: the value the person set stays" "$(gs get "${COLOR}" night-light-temperature)" "uint32 2000"
read -r _ temperature <<<"${NIGHT_BEFORE}"
gs set "${COLOR}" night-light-temperature "${temperature#uint32 }"
hour=$((10#$(date +%H)))
gs set "${COLOR}" night-light-schedule-automatic false
gs set "${COLOR}" night-light-schedule-from "$(((hour + 23) % 24)).0"
gs set "${COLOR}" night-light-schedule-to "$(((hour + 2) % 24)).0"
in_session "ame desktop focus trigger sleep on" >/dev/null
wait_for 5 profile_is sleep
in_session "ame desktop focus off" >/dev/null
wait_for 5 profile_is off
if lock_and_unlock; then
    sleep 3
    same "15b Off set the night aside, and a lock does not bring it back" "$(st .profile)" '"off"'
else
    not_ok "15b the screen did not lock and unlock"
fi
# The schedule follows the sun: at an unlock nothing flaps while the daemon is asked again, and the answer settles
gs set "${COLOR}" night-light-schedule-automatic true
sleep 2
before="$(st .profile)"
if lock_and_unlock; then
    settled() { [[ "$(st .triggers.sleep.pending)" == false ]]; }
    check "15c following the sun, the trigger settles within 5 s of the unlock" wait_for 5 settled
    same "15c and decides as before the lock (no flap)" "$(st .profile)" "${before}"
else
    not_ok "15c the screen did not lock and unlock"
fi
in_session "ame desktop focus off" >/dev/null
in_session "ame desktop focus trigger sleep off" >/dev/null
gs set "${COLOR}" night-light-schedule-automatic false
# A game set aside by Off stays aside across a lock, and is no longer aside once started again
in_session "ame desktop focus trigger games on" >/dev/null
user_bus systemd-run --user --collect --unit=vmlab-steamgame gjs -m "${HOME_DIR}/.local/share/vmlab-focus/steamgame.js" steam_app_2 >/dev/null
steam2_ready() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'steam_app_2')")" == true ]]; }
wait_for 60 steam2_ready
activate steam_app_2
same "15d a game starts Games" "$(st .profile)" '"games"'
in_session "ame desktop focus off" >/dev/null
wait_for 5 profile_is off
if lock_and_unlock; then
    sleep 3
    activate steam_app_2
    same "15d the game set aside by Off stays aside after a lock" "$(st .profile)" '"off"'
else
    not_ok "15d the screen did not lock and unlock"
fi
user_bus systemctl --user stop vmlab-steamgame >/dev/null 2>&1
sleep 2
user_bus systemd-run --user --collect --unit=vmlab-steamgame gjs -m "${HOME_DIR}/.local/share/vmlab-focus/steamgame.js" steam_app_2 >/dev/null
wait_for 60 steam2_ready
activate steam_app_2
same "15d started again, the same game starts Games" "$(st .profile)" '"games"'
user_bus systemctl --user stop vmlab-steamgame >/dev/null 2>&1
wait_for 5 profile_is off
in_session "ame desktop focus trigger games off" >/dev/null
pick sleep
sleep 2

# --- 18: a theme switch keeps the accent -------------------------------------------------------------------------------
theme="$(in_session "amethystora-theme current" 2>/dev/null)"
in_session "ame desktop theme amethystora-light" >/dev/null 2>&1
check "18 after a theme switch the amber accent is back within 30 s" wait_for 30 accent_on
in_session "ame desktop theme ${theme:-amethystora}" >/dev/null 2>&1
pick off

# --- 16-17: Control and the setup file ---------------------------------------------------------------------------------
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "16 Control has the Focus groups" \
    "$(jq -r '[.items[] | select(.id | startswith("focus")) | .group] | unique | join(",")' <<<"${control}")" \
    "Focus,Focus: Games,Focus: Sleep,Focus: Work"
same "16 Control's Focus says Off" "$(jq -r '.items[] | select(.id == "focus") | .state' <<<"${control}")" off
in_session "ame desktop focus edit games grey on" >/dev/null
in_session "ame desktop focus work" >/dev/null
in_session "ame setup save" >/dev/null 2>&1
same "17 the setup file carries the edit, never what is on" \
    "$(jq -c '.settings.focus' "${HOME_DIR}/.config/amethystora/setup.json")" '{"games-grey":true}'
in_session "ame desktop focus off" >/dev/null

# --- and nothing threw ---------------------------------------------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: the focus module|Amethystora UI: focus" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of focus threw"; else not_ok "focus threw" "${errors}"; fi
finish
