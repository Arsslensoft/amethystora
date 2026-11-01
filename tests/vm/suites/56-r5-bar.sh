#!/usr/bin/bash
# vmlab-vm: r5-bar
# Rounds 5 and 6, group bar (plans/ui-ideas/spec-bar.md, its acceptance criteria), in the test account's session, asked
# of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   1 the keys with their defaults;
#   2 arranging the bar: the clock first on the left, meters hidden, the clock and the system menu never hidden, and
#     the bar exactly as it was once the layout is reset; the helper and the recipe say the same;
#   3 the bar's size: 28, 32 and 40 pixels inside its edges, the pill's plate with it, touch mode still taller;
#   4 at the bottom: the main bar at the screen's bottom with the room kept, its menus opening upwards, the dock above
#     it, and back at the top;
#   5 hidden until touched: no room kept, the bar hidden, a barrier on its edge, and the peek showing it;
#   6 animation speed: 0.4, 0.6 and 0.9 of the time, off holding animations back without GNOME's switch, and back;
#   7 text size: GNOME's text scaling at 115 %, the clock's text larger, and back;
#   8 opening and closing: with pop, a window that opens and one that closes are drawn by the module and end as GNOME
#     leaves them; gnome gives GNOME its handlers back;
#   9 the clock: a second time zone after GNOME's clock, on the 12-hour clock too, and gone again;
#  10 the light desktop: the slow computer's notice for a machine drawn in software, Use the light desktop turning it
#     on (solid glass, shorter motion, no dock previews) and remembered, and off again;
#  11 ten disables and enables leaving nothing; Control's entries and the setup file.
# The VM draws in software, where GNOME turns its animations off: what moves is checked with global.force_animations on,
# and put back after.
# vmlab-paths: @ui/extension.js @ui/*bars* @ui/*tweaks* @ui/*comfort* @ui/*motion* @ui/*pill* @ui/*dock* @sf/usr/libexec/amethystora-bar-layout @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/*just/desktop.just @sf/*manual/desktop.md @sf/*manual/themes.md
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HERE="$(dirname "$0")"
HOME_DIR="/var/home/${TEST_USER}"
BARS=org.gnome.shell.extensions.amethystora-ui.bars
MOTION=org.gnome.shell.extensions.amethystora-ui.motion
PILL=org.gnome.shell.extensions.amethystora-ui.pill
COMFORT=org.gnome.shell.extensions.amethystora-ui.comfort
STATE="${HOME_DIR}/.local/state/amethystora"
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
# JSON compared without its spaces: the expressions stringify compactly, the expectations are written with ", "
same() { if [[ "${2//, /,}" == "${3//, /,}" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
is() { [[ "$(ev "$1")" == "$2" ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
B="${UI}.get('bars')"
state_is() { is "Main.extensionManager.lookup('${UUID}').state" "$1"; }
reset_all() {
    for key in layout position autohide size; do gs reset "${BARS}" "${key}"; done
    for key in speed open-close; do gs reset "${MOTION}" "${key}"; done
    gs reset "${PILL}" second-zone
    gs reset "${COMFORT}" light
    for key in clock-format clock-show-seconds text-scaling-factor; do gs reset org.gnome.desktop.interface "${key}"; done
}

if ! has_session; then
    bash "${HERE}/10-account.sh" >/dev/null
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
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
reset_all
loginctl unlock-sessions
systemctl restart gdm
ready() { is "typeof ${B}?.placed" object; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "the bars module did not start in the session"
    finish
fi
wait_for 30 is "Main.layoutManager._startingUp" false
ev "Main.overview.hide(); 'hidden'" >/dev/null
wait_for 10 is "Main.overview.visible" false
cleanup() {
    ev "global.force_animations = false; 'x'" >/dev/null
    user_bus systemctl --user stop 'vmlab-r5bar-*' >/dev/null 2>&1
    reset_all
}
trap cleanup EXIT

# --- 1: the keys ----------------------------------------------------------------------------------------------------------
same "1 the bars' keys" "$(gs list-keys "${BARS}" | sort | paste -sd ' ')" "autohide every-screen layout peek position size"
same "1 their defaults" "$(for k in layout position autohide size; do gs get "${BARS}" "${k}"; done | paste -sd ' ')" \
    "@a{sas} {} 'top' false 'regular'"
same "1 the motion's" "$(gs get "${MOTION}" speed) $(gs get "${MOTION}" open-close)" "'normal' 'gnome'"
same "1 no second zone, no light desktop" "$(gs get "${PILL}" second-zone) $(gs get "${COMFORT}" light)" "'' false"

# --- 2: arranging the bar -------------------------------------------------------------------------------------------------
order='JSON.stringify([Main.panel._leftBox, Main.panel._centerBox, Main.panel._rightBox].map(b => b.get_children().map(c => Object.entries(Main.panel.statusArea).find(([k, v]) => v.container === c)?.[0] ?? "?")))'
before="$(ev "${order}")"
in_session "ame desktop top-bar item clock left 0" >/dev/null
in_session "ame desktop top-bar item memory hidden" >/dev/null
wait_for 10 is "${B}.placed.left[0]" clock
same "2 the clock first on the left" "$(ev "${B}.placed.left[0]")" clock
same "2 the memory meter hidden" "$(ev "JSON.stringify([${B}.placed.hidden.includes('memory'), Main.panel.statusArea['amethystora-meters-memory']?.container.visible ?? false])")" \
    '[true, false]'
same "2 the clock's menu still opens from it" "$(ev "Main.panel.statusArea.dateMenu.menu.open(); const o = Main.panel.statusArea.dateMenu.menu.isOpen; Main.panel.statusArea.dateMenu.menu.close(); o")" true
if in_session "ame desktop top-bar item system hidden" >/dev/null 2>&1; then
    not_ok "2 the system menu was hidden"
else
    ok "2 the system menu is never hidden"
fi
same "2 the helper says where they are" "$(in_session "/usr/libexec/amethystora-bar-layout show" | sed -n 's/^left: //p' | cut -d' ' -f1)" clock
in_session "ame desktop top-bar layout reset" >/dev/null
wait_for 10 is "${order}" "${before}"
same "2 reset: the bar as it was" "$(ev "${order}")" "${before}"
same "2 reset: the meter back" "$(ev "Main.panel.statusArea['amethystora-meters-memory']?.container.visible ?? true")" true

# --- 3: the size ----------------------------------------------------------------------------------------------------------
# The panel's height less its own top and bottom edges (the floating style draws 1 px each)
inner='(() => { const n = Main.panel.get_theme_node(); return Math.round(Main.panel.height - n.get_border_width(0) - n.get_border_width(2)); })()'
for pair in compact:28 regular:32 roomy:40; do
    in_session "ame desktop top-bar size ${pair%%:*}" >/dev/null
    wait_for 10 is "${inner}" "${pair##*:}"
    same "3 ${pair%%:*} is ${pair##*:} px" "$(ev "${inner}")" "${pair##*:}"
done
same "3 the pill's plate follows" "$(ev "Math.round(${UI}.get('pill').clock?._pill?.plateHeight ?? 0)")" 34
ev "Main.layoutManager.uiGroup.add_style_class_name('amethystora-touch'); 'x'" >/dev/null
same "3 touch mode still wins" "$(ev "${inner}")" 44
ev "Main.layoutManager.uiGroup.remove_style_class_name('amethystora-touch'); 'x'" >/dev/null
in_session "ame desktop top-bar size regular" >/dev/null

# --- 4: at the bottom -----------------------------------------------------------------------------------------------------
area='(() => { const w = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex); return JSON.stringify([w.y, w.y + w.height]); })()'
bottom_edge='(() => { const m = Main.layoutManager.primaryMonitor; return Math.round(m.y + m.height); })()'
in_session "ame desktop top-bar position bottom" >/dev/null
wait_for 10 is "${B}.bottom" true
same "4 the main bar at the bottom" "$(ev "Math.round(Main.layoutManager.panelBox.y + Main.layoutManager.panelBox.height)")" "$(ev "${bottom_edge}")"
same "4 the windows' room starts at the top and ends above it" \
    "$(ev "(() => { const w = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex); return w.y === Main.layoutManager.primaryMonitor.y && w.y + w.height <= Main.layoutManager.panelBox.y; })()")" true
same "4 its menus open upwards" "$(ev "Main.panel.statusArea.quickSettings.menu._boxPointer._userArrowSide === imports.gi.St.Side.BOTTOM")" true
same "4 the dock sits above it" \
    "$(ev "(() => { const d = ${UI}.get('dock')?.docks?.find(s => s.monitorIndex === Main.layoutManager.primaryIndex && s.edgeName === 'bottom'); return !d || Math.round(d.strip.y + d.strip.height) <= Math.round(Main.layoutManager.panelBox.y); })()")" true
in_session "ame desktop top-bar position top" >/dev/null
wait_for 10 is "${B}.bottom" false
same "4 back at the top" "$(ev "JSON.stringify([Main.layoutManager.panelBox.y === Main.layoutManager.primaryMonitor.y, Main.panel.statusArea.quickSettings.menu._boxPointer._userArrowSide === imports.gi.St.Side.TOP])")" '[true, true]'

# --- 5: hidden until touched ----------------------------------------------------------------------------------------------
in_session "ame desktop top-bar autohide on" >/dev/null
wait_for 10 is "Main.layoutManager.panelBox.visible" false
same "5 hidden, its room given back" \
    "$(ev "JSON.stringify([Main.layoutManager.panelBox.visible, Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex).y === Main.layoutManager.primaryMonitor.y, ${B}._barriers.has(Main.layoutManager.primaryIndex)])")" \
    '[false, true, true]'
ev "global.force_animations = true; ${B}._peek(Main.layoutManager.primaryIndex); 'x'" >/dev/null
same "5 pushing against the edge brings it in" "$(ev "JSON.stringify([${B}.peeking(Main.layoutManager.primaryIndex), Main.layoutManager.panelBox.visible])")" '[true, true]'
wait_for 10 is "${B}.peeking(Main.layoutManager.primaryIndex)" false
ev "global.force_animations = false; 'x'" >/dev/null
in_session "ame desktop top-bar autohide off" >/dev/null
wait_for 10 is "Main.layoutManager.panelBox.visible" true
same "5 off: in view and its room kept" "$(ev "Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex).y > Main.layoutManager.primaryMonitor.y")" true

# --- 6: animation speed ---------------------------------------------------------------------------------------------------
ev "global.force_animations = true; 'x'" >/dev/null
wait_for 10 is "imports.gi.St.Settings.get().enable_animations" true
for pair in snappy:0.4 relaxed:0.9 normal:0.6; do
    in_session "ame desktop animation-speed ${pair%%:*}" >/dev/null
    wait_for 10 is "Math.round(imports.gi.St.Settings.get().slow_down_factor * 100) / 100" "${pair##*:}"
    same "6 ${pair%%:*}" "$(ev "Math.round(imports.gi.St.Settings.get().slow_down_factor * 100) / 100")" "${pair##*:}"
done
in_session "ame desktop animation-speed off" >/dev/null
wait_for 10 is "imports.gi.St.Settings.get().enable_animations" false
same "6 off holds animations back, GNOME's switch untouched" \
    "$(ev "imports.gi.St.Settings.get().enable_animations") $(gs get org.gnome.desktop.interface enable-animations)" "false true"
in_session "ame desktop animation-speed normal" >/dev/null
wait_for 10 is "imports.gi.St.Settings.get().enable_animations" true
same "6 back" "$(ev "imports.gi.St.Settings.get().enable_animations")" true

# --- 7: text size ---------------------------------------------------------------------------------------------------------
width='Main.panel.statusArea.dateMenu._clockDisplay.get_preferred_width(-1)[1]'
narrow="$(ev "${width}")"
in_session "ame desktop text-size 115" >/dev/null
same "7 GNOME's text scaling" "$(gs get org.gnome.desktop.interface text-scaling-factor | python3 -c 'import sys; print(round(float(sys.stdin.read()), 2))')" 1.15
wait_for 10 python3 -c "import sys; sys.exit(0 if float('$(ev "${width}")') > float('${narrow}') else 1)"
wide="$(ev "${width}")"
if python3 -c "import sys; sys.exit(0 if float('${wide}') > float('${narrow}') else 1)"; then
    ok "7 the clock's text is larger"
else
    not_ok "7 the clock's text is larger" "before ${narrow}, after ${wide}"
fi
in_session "ame desktop text-size 100" >/dev/null

# --- 8: opening and closing -----------------------------------------------------------------------------------------------
ev "(() => { const M = ${UI}.get('motion'); globalThis.vmlabArrive = []; const self = {};
    global.window_manager.connectObject(
        'map', (wm, a) => globalThis.vmlabArrive.push(['map', M.arriving(a), a.opacity, Math.round(a.scale_x * 100)]),
        'destroy', (wm, a) => globalThis.vmlabArrive.push(['destroy', M.arriving(a)]), self);
    globalThis.vmlabArriveStop = () => global.window_manager.disconnectObject(self); return 'x'; })()" >/dev/null
in_session "ame desktop window-motion pop" >/dev/null
same "8 GNOME's map and destroy stand aside" "$(ev "[...${UI}.get('motion')._blocked.keys()].sort().join()")" "destroy,map,minimize,unminimize"
user_bus systemd-run --user --collect --unit="vmlab-r5bar-files-$$" nautilus --new-window >/dev/null
wait_for 60 is "globalThis.vmlabArrive.some(r => r[0] === 'map')" true
same "8 a window pops in" "$(ev "JSON.stringify(globalThis.vmlabArrive.find(r => r[0] === 'map'))")" '["map", true, 0, 92]'
wait_for 10 is "Main.wm._mapping.size" 0
same "8 and ends as GNOME leaves it" "$(ev "JSON.stringify(global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').map(a => [a.opacity, a.scale_x, a.scale_y]))")" \
    '[[255, 1, 1]]'
ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').forEach(a => a.meta_window.delete(global.get_current_time())); 'x'" >/dev/null
wait_for 30 is "globalThis.vmlabArrive.some(r => r[0] === 'destroy')" true
same "8 and leaves backwards" "$(ev "JSON.stringify(globalThis.vmlabArrive.find(r => r[0] === 'destroy'))")" '["destroy", true]'
wait_for 10 is "Main.wm._destroying.size" 0
same "8 nothing left behind" "$(ev "Main.wm._destroying.size + Main.wm._mapping.size")" 0
in_session "ame desktop window-motion gnome" >/dev/null
same "8 gnome: GNOME's own again" "$(ev "[...${UI}.get('motion')._blocked.keys()].sort().join()")" "map,minimize,unminimize"
ev "globalThis.vmlabArriveStop(); global.force_animations = false; 'x'" >/dev/null

# --- 9: the clock ---------------------------------------------------------------------------------------------------------
in_session "ame desktop clock zone Asia/Tokyo" >/dev/null
wait_for 10 is "(${UI}.get('pill').zone?.text ?? '').startsWith('Tokyo ')" true
tokyo="$(TZ=Asia/Tokyo date +%H:%M)"
same "9 Tokyo's time after the clock" "$(ev "${UI}.get('pill').zone.text.replace('∶', ':')")" "Tokyo ${tokyo}"
same "9 in GNOME's clock box" "$(ev "Main.panel.statusArea.dateMenu._clockDisplay.get_parent().get_children().some(c => c.has_style_class_name?.('amethystora-pill-zone'))")" true
in_session "ame desktop clock format 12h" >/dev/null
wait_for 10 is "/ [AP]M$/.test(${UI}.get('pill').zone.text)" true
same "9 on the 12-hour clock" "$(ev "/ [AP]M$/.test(${UI}.get('pill').zone.text)")" true
if in_session "ame desktop clock zone Not/AZone" >/dev/null 2>&1; then not_ok "9 an unknown zone was taken"; else ok "9 an unknown zone is refused"; fi
in_session "ame desktop clock zone off" >/dev/null
in_session "ame desktop clock format 24h" >/dev/null
same "9 off: gone" "$(ev "${UI}.get('pill').zone?.text ?? null")" null

# --- 10: the light desktop ------------------------------------------------------------------------------------------------
C="${UI}.get('comfort')"
same "10 this machine is drawn in software" "$(ev "JSON.stringify((() => { const f = ${C}.checkSlow(); return [f.slow, f.software]; })())")" '[true, true]'
same "10 offered once, in a notification" "$(ev "${C}._slow._notification?.title ?? null")" "This computer draws the screen without a graphics driver"
ev "${C}._slow._notification.actions.find(a => a.label === 'Use the light desktop').activate(); 'x'" >/dev/null
wait_for 10 is "${C}.light" true
same "10 the light desktop: solid glass and shorter motion" \
    "$(ev "JSON.stringify([${C}.light, ${C}.lighter, Math.round(imports.gi.St.Settings.get().slow_down_factor * 100) / 100])")" '[true, true, 0.3]'
same "10 remembered" "$(jq -r .asked "${STATE}/light-desktop.json" 2>/dev/null | cut -d'|' -f1)" software
same "10 not asked again" "$(ev "(() => { ${C}._own.set_boolean('light', false); return ${C}._slow.check(null).offered; })()")" false
in_session "ame desktop light-desktop off" >/dev/null
same "10 off" "$(gs get "${COMFORT}" light)" false

# --- 11: ten disables and enables, Control and the setup file -------------------------------------------------------------
in_session "ame desktop top-bar position bottom" >/dev/null
in_session "ame desktop top-bar item clock left 0" >/dev/null
census='(() => { const m = {}; const v = a => { const k = a.style_class || a.constructor.name; m[k] = (m[k] ?? 0) + 1; a.get_children().forEach(v); }; v(Main.uiGroup); m.statusArea = Object.keys(Main.panel.statusArea).length; m.uiClasses = Main.uiGroup.get_style_class_name(); m.ownBoxes = Object.prototype.hasOwnProperty.call(Main.layoutManager, "_updateBoxes"); return m; })()'
off() { ev "Main.extensionManager.disableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 2 && sleep 1; }
on() { ev "Main.extensionManager.enableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 1; }
off
ev "globalThis.vmlabCensus = ${census}; 'x'" >/dev/null
same "11 off: the bar at the top, in GNOME's order" \
    "$(ev "JSON.stringify([Main.layoutManager.panelBox.y === Main.layoutManager.primaryMonitor.y, Main.panel.statusArea.dateMenu.container.get_parent() === Main.panel._centerBox])")" '[true, true]'
cycles=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
    on || break
    off || break
    cycles=$((cycles + 1))
done
same "11 ten cycles" "${cycles}" 10
same "11 leave nothing" "$(ev "(() => { const a = ${census}, b = globalThis.vmlabCensus;
    return JSON.stringify(Object.keys({...a, ...b}).filter(k => (a[k] ?? 0) !== (b[k] ?? 0)).map(k => k + ' ' + a[k] + '/' + b[k]).slice(0, 12)); })()")" "[]"
on
wait_for 60 ready
same "11 on again: arranged and at the bottom" "$(ev "JSON.stringify([${B}.placed.left[0], ${B}.bottom])")" '["clock", true]'
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "11 Control's entries" \
    "$(jq -r '[.items[] | select(.id | IN("top-bar-layout", "top-bar-size", "top-bar-position", "top-bar-autohide", "animation-speed", "text-size", "light-desktop", "window-motion", "clock-format", "clock-seconds", "clock-zone")) | .id] | sort | join(",")' <<<"${control}")" \
    "animation-speed,clock-format,clock-seconds,clock-zone,light-desktop,text-size,top-bar-autohide,top-bar-layout,top-bar-position,top-bar-size,window-motion"
same "11 the bar's entry lists the clock on the left" \
    "$(jq -r '.items[] | select(.id == "top-bar-layout") | .bar_items[] | select(.id == "clock") | .zone' <<<"${control}")" left
in_session "ame desktop animation-speed relaxed" >/dev/null
in_session "ame desktop clock zone Europe/Paris" >/dev/null
in_session "ame setup save" >/dev/null 2>&1
same "11 the setup file carries them" \
    "$(jq -c '[.settings.bars.position, .settings.bars.layout.left[0], .settings.motion.speed, .settings.pill["second-zone"]]' "${HOME_DIR}/.config/amethystora/setup.json")" \
    '["bottom","clock","relaxed","Europe/Paris"]'

finish
