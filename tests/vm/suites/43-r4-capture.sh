#!/usr/bin/bash
# vmlab-vm: capture
# Round 4, group capture (plans/beyond/spec-r4-capture.md, section 5), in the test account's session, asked of GNOME
# Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - Present mode (R4-02): off costs nothing; offered, its tile; presenting keeps the screen awake (gnome-session's
#     idle inhibitor), holds banners and shows the pill, and says what it held; the halo and the torch (a press on a real
#     window still reaches it while the halo is up); the screen question and its Present; a share or screen it started
#     for ends it, also when it went during a lock; Scribble's ink and keys, Print over it; a lock keeps it;
#   - Pin a capture (R4-17): the thumbnail's slide, size, place, buttons and six seconds; Copy, Show in Files and Delete;
#     a pin from the thumbnail and from the bar (no file, the clipboard kept), wheel, Ctrl+wheel, drag, click-through and
#     the cross by real input, six at most; the thumbnail off gives GNOME's words back; pinning off lets go; a lock keeps
#     the pins; a share blurs pins and a thumbnail;
#   - Widgets on the desktop (R4-19): off runs nothing; on, a card in its corner, right under the windows after a restack
#     and over a workspace's own picture, its size, class and six lines, a click and a menu; a broken one and its folder;
#     the bar leaves it alone; the plugins module off stops them;
#   - the light theme and a theme switch;
#   - ten disables and enables with everything on, nothing left behind, and nothing thrown.
# vmlab-paths: @ui/extension.js @ui/*present* @ui/*deskwidgets* @ui/*screenshot* @ui/*pill* @ui/*tiling/backgrounds* @ui/*focus/picture* @sf/*themed/shell-present* @sf/*themed/shell-deskwidgets* @sf/*themed/shell-screenshot* @sf/usr/share/gnome-shell/extensions/amethystora-widgets@* @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just @sf/usr/libexec/amethystora-cover-private @ui/*plugins*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
PRESENT=org.gnome.shell.extensions.amethystora-ui.present
SHOT=org.gnome.shell.extensions.amethystora-ui.screenshot
DESK=org.gnome.shell.extensions.amethystora-ui.deskwidgets
WIDGETS="${HOME_DIR}/.config/amethystora/widgets"
LAB="${HOME_DIR}/.local/share/vmlab-capture"
# Eval's answer decoded, as suite 32 does
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
like() { [[ "$1" == $2 ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
P="${UI}.get('present')"
S="${UI}.get('screenshot')"
W="${UI}.get('deskwidgets')"
pst() { ev "JSON.stringify(${P}.state()$1)"; }
inhibited() {
    user_bus gdbus call --session --dest org.gnome.SessionManager --object-path /org/gnome/SessionManager \
        --method org.gnome.SessionManager.IsInhibited 8 2>/dev/null
}
# A virtual pointer and keyboard of the shell's own seat, as the harness drives them: moves, clicks, drags, keys
INPUT="const C = imports.gi.Clutter, GLib = imports.gi.GLib;
const seat = global.stage.context.get_backend().get_default_seat();
globalThis.__vmp ??= seat.create_virtual_device(C.InputDeviceType.POINTER_DEVICE);
globalThis.__vmk ??= seat.create_virtual_device(C.InputDeviceType.KEYBOARD_DEVICE);
const now = () => GLib.get_monotonic_time();"
move() { ev "${INPUT} __vmp.notify_absolute_motion(now(), $1, $2); 'moved'" >/dev/null; sleep 0.3; }
click() {
    # The press a moment after the move and the release a moment after the press, as a hand does: a window takes a press
    # and a release in the same instant for no click
    ev "${INPUT} __vmp.notify_absolute_motion(now(), $1, $2); GLib.timeout_add(0, 100, () => {
        __vmp.notify_button(now(), C.BUTTON_PRIMARY, C.ButtonState.PRESSED); GLib.timeout_add(0, 80, () => {
        __vmp.notify_button(now(), C.BUTTON_PRIMARY, C.ButtonState.RELEASED); return false; }); return false; }); 'clicked'" >/dev/null
    sleep 1
}
drag() {
    ev "${INPUT} const [x1, y1, x2, y2] = [$1, $2, $3, $4]; __vmp.notify_absolute_motion(now(), x1, y1); let i = 0;
        GLib.timeout_add(0, 30, () => { if (i === 0) __vmp.notify_button(now(), C.BUTTON_PRIMARY, C.ButtonState.PRESSED);
        else if (i <= 10) __vmp.notify_absolute_motion(now(), x1 + (x2 - x1) * i / 10, y1 + (y2 - y1) * i / 10);
        else { __vmp.notify_button(now(), C.BUTTON_PRIMARY, C.ButtonState.RELEASED); return false; } i++; return true; }); 'dragging'" >/dev/null
    sleep 1
}
# Keysyms by name, pressed in order and let go in reverse
keys() {
    ev "${INPUT} const k = [$(printf 'C.KEY_%s,' "$@")]; k.forEach((v, i) => __vmk.notify_keyval(now() + i * 1000, v, C.KeyState.PRESSED));
        [...k].reverse().forEach((v, i) => __vmk.notify_keyval(now() + 10000 + i * 1000, v, C.KeyState.RELEASED)); 'keys'" >/dev/null
    sleep 0.6
}
# The screenshot bar on an area, then a capture (normal) or a pin; the last argument is how long to wait after
capture() {
    ev "Main.overview.hide(); Main.screenshotUI.open().then(() => {}); 'opened'" >/dev/null
    sleep 1.5
    ev "(() => { const a = Main.screenshotUI._areaSelector; a._startX = $1; a._startY = $2; a._lastX = $1 + $3 - 1;
        a._lastY = $2 + $4 - 1; a._updateSelectionRect(); return 'framed'; })()" >/dev/null
    if [[ "${5:-normal}" == pin ]]; then
        keys Control_L p
    else
        ev "Main.screenshotUI._onCaptureButtonClicked(); 'taken'" >/dev/null
    fi
    sleep "${6:-1.5}"
}
# A wheel step (UP or DOWN) at a point; a key held down or let go, for Ctrl and the wheel
scroll() {
    move "$1" "$2"
    ev "${INPUT} __vmp.notify_discrete_scroll(now(), C.ScrollDirection.$3, C.ScrollSource.WHEEL); 'scrolled'" >/dev/null
    sleep 0.5
}
keys_down() { ev "${INPUT} __vmk.notify_keyval(now(), C.KEY_$1, C.KeyState.PRESSED); 'down'" >/dev/null; sleep 0.2; }
keys_up() { ev "${INPUT} __vmk.notify_keyval(now(), C.KEY_$1, C.KeyState.RELEASED); 'up'" >/dev/null; sleep 0.2; }
pins() { ev "JSON.stringify(${S}.pins()$1)"; }
card() { ev "(() => { const c = ${S}._card; if (!c) return 'none'; const a = c.actor; const m = Main.layoutManager.primaryMonitor;
    return JSON.stringify({right: m.x + m.width - a.x - a.width, bottom: m.y + m.height - a.y - a.height, width: a.width,
    picture: a.get_child_at_index(0).height, buttons: a.get_child_at_index(1).get_children().filter(b => b.accessible_name).map(b => b.accessible_name)}); })()"; }
no_card() { [[ "$(card)" == none ]]; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab" "${LAB}"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
# A window with a button that counts its clicks, for the halo and the cards under windows
cat >"${LAB}/win.js" <<'EOF'
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: ARGV[0]});
app.connect('activate', () => {
    const win = new Gtk.ApplicationWindow({application: app, title: ARGV[0], default_width: 800, default_height: 600});
    const button = new Gtk.Button({label: 'Click me'});
    let n = 0;
    button.connect('clicked', () => GLib.file_set_contents(`${GLib.get_user_runtime_dir()}/vmlab-clicks`, String(++n)));
    win.set_child(button);
    win.present();
});
app.run([]);
EOF
# Three plugins on the desktop (one with a menu and a long text) and one for the bar: the lab has no host, so the plugins module is handed what the
# host would hand it (_list), and its host is a stand-in whose Action leaves a file (what a click asked for)
PL="${UI}.get('plugins')"
inject_plugins() {
    ev "(() => { const m = ${PL}; const gio = imports.gi.GLib;
        m._host = {stop() {}, pause() {}, resume() {}, action(id, a) { gio.file_set_contents('${LAB}/vmlab-' + id.split('.')[1] + '-clicked', a); }};
        m._list([
            {id: 'vmlab.clock', name: 'Clock', places: {desktop: {corner: 'top-left'}}, output: {text: '12:00 Today', lines: ['12:00', 'Today']}},
            {id: 'vmlab.build', name: 'Build', places: {desktop: {corner: 'top-right'}}, actions: ['clicked'], output: {text: 'Build status main green', lines: ['Build status', 'main · green'], class: 'good', click: 'clicked'}},
            {id: 'vmlab.menu', name: 'Menu', places: {desktop: {corner: 'bottom-left'}}, actions: ['go'], output: {text: '1 2 3 4 5 6 7 8', lines: ['1', '2', '3', '4', '5', '6', '7', '8'], menu: [{label: 'A line'}, {label: 'Go', action: 'go'}]}},
            {id: 'vmlab.bar', name: 'Bar', places: {bar: {position: 'right'}}, output: {text: 'bar'}},
        ]); return 'injected'; })()" >/dev/null
}
chown -R "${TEST_USER}:" "${HOME_DIR}/.config/amethystora" "${HOME_DIR}/.local/share"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
for schema in "${PRESENT}" "${SHOT}" "${DESK}"; do
    for key in $(in_session "gsettings list-keys ${schema}"); do gs reset "${schema}" "${key}"; done
done
# The classic layout, so that a window can be put where a card is
gs set org.gnome.shell.extensions.amethystora-ui.tiling enabled false
loginctl unlock-sessions
systemctl restart gdm
ready() { [[ "$(ev "typeof ${P}?.state + typeof ${W}?.cards")" == functionfunction ]]; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "the present and deskwidgets modules did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
cleanup() {
    for schema in "${PRESENT}" "${SHOT}" "${DESK}"; do
        for key in $(in_session "gsettings list-keys ${schema}" 2>/dev/null); do gs reset "${schema}" "${key}"; done
    done
    gs reset org.gnome.shell.extensions.amethystora-ui.tiling enabled
    rm -rf "${WIDGETS}"/vmlab-* "${LAB}"
    user_bus systemctl --user stop vmlab-win vmlab-win2 >/dev/null 2>&1
}
trap cleanup EXIT

# --- R4-02 Present mode ------------------------------------------------------------------------------------------------
same "1 the present schema as shipped" \
    "$(gs get "${PRESENT}" enabled) $(gs get "${PRESENT}" pointer-mark) $(gs get "${PRESENT}" ask-on-screen) $(gs get "${PRESENT}" scribble)" \
    "false 'halo' true ['<Super><Alt>d']"
same "2 off: no tile, key, screen watch, inhibitor, timer or Scribble" \
    "$(pst | jq -c '[.tile, .keybinding, .screens, .mark, .inhibitor, .timer, .capture]') $(ev "String(${P}.scribble)")" \
    '[false,false,false,null,0,false,false] null'
keys Super_L Alt_L d
same "2 off: Super+Alt+D draws nothing" "$(pst .scribble) $(ev "Main.modalCount")" "false 0"
in_session "ame desktop present on" >/dev/null
sleep 1
same "3 offered: the Present tile (id present), the key, the screen watch" \
    "$(pst | jq -c '[.tile, .keybinding, .screens]') $(ev "${P}._toggle._amethystoraQuickId + ' ' + ${P}._toggle.subtitle")" \
    '[true,true,true] present Halo'
# Quiet off, so that the notice at the end comes from presenting alone
gs set org.gnome.shell.extensions.amethystora-ui.pill quiet false
# and from this suite's banner alone: what earlier suites left in GNOME's queue would be held and counted too
ev "Main.messageTray.getSources().forEach(s => [...s.notifications].forEach(n => n.destroy())); 'cleared'" >/dev/null
ev "${P}._toggle.emit('clicked', 1); 'clicked'" >/dev/null
sleep 2
same "4 presenting: the session is kept from idling" "$(inhibited)" "(true,)"
same "4 the pill shows Presenting" "$(ev "${UI}.get('pill')._activities.top?.id")" present
check "4 the pill reads Presenting · m:ss" like "$(ev "${UI}.get('pill')._activities.top?.text")" 'Presenting · [0-9]:[0-9][0-9]'
in_session "notify-send 'vmlab present held'" >/dev/null
sleep 1
same "4 a banner is held while presenting" "$(ev "${UI}.get('pill').hold.reason + ' ' + (Main.messageTray._notification?.title ?? 'none')")" \
    "presenting none"
move 960 540
same "5 the halo is centred on the pointer" "$(ev "(() => { const a = ${P}._pointer.actor; return [a.translation_x + a.width / 2, a.translation_y + a.height / 2, a.width].join(' '); })()")" \
    "960 540 52"
gs set "${PRESENT}" pointer-mark torch
move 700 400
same "5 the torch: the hole round the pointer, the four panes round the hole" \
    "$(ev "(() => { const m = ${P}._pointer._mark; const h = m._hole; const s = h.width; const [t, b, l, r] = m._panes; const x0 = h.translation_x, y0 = h.translation_y;
        return [x0 + s / 2, y0 + s / 2, t.translation_y + t.height === y0, b.translation_y === y0 + s, l.translation_x + l.width === x0, r.translation_x === x0 + s].join(' '); })()")" \
    "700 400 true true true true"
gs set "${PRESENT}" pointer-mark halo
user_bus systemd-run --user --unit vmlab-win --collect -- gjs -m "${LAB}/win.js" org.vmlab.Capture >/dev/null
window_up() { [[ "$(ev "global.display.list_all_windows().some(w => w.title === 'org.vmlab.Capture')")" == true ]]; }
clicks() { cat "/run/user/$(id -u "${TEST_USER}")/vmlab-clicks" 2>/dev/null || echo 0; }
if wait_for 30 window_up; then
    sleep 2
    rm -f "/run/user/$(id -u "${TEST_USER}")/vmlab-clicks"
    centre="$(ev "(() => { const w = global.display.list_all_windows().find(w => w.title === 'org.vmlab.Capture'); const r = w.get_frame_rect(); return (r.x + r.width / 2) + ' ' + (r.y + r.height / 2); })()")"
    # shellcheck disable=SC2086
    click ${centre}
    same "5 with the halo up, a click still reaches the window" "$(clicks)" 1
else
    not_ok "5 the test window did not open"
fi
ev "${P}.stop(); 'stopped'" >/dev/null
sleep 2
same "6 stopped: the inhibitor is gone" "$(inhibited)" "(false,)"
same "6 the pill says the held banner is waiting, with quiet off" "$(ev "${UI}.get('pill')._activities.top?.text ?? 'clock'")" "1 waiting in the list"
gs reset org.gnome.shell.extensions.amethystora-ui.pill quiet
# 7: a second screen, as Mutter would report it (the lab has one head): the module's list of connectors stood in for;
# the question is the real notification, and its Present the real action
present_action() {
    ev "(() => { const n = Main.messageTray.getSources().find(s => s.title === 'Present')?.notifications.find(x => x.title === 'Present on the new screen?');
        const a = n?.actions.find(x => x.label === 'Present'); if (!a) return 'none'; a.activate(); return 'clicked'; })()"
}
same "7 a new screen is asked about once" \
    "$(ev "(() => { const p = ${P}; const base = Object.getPrototypeOf(p)._connectors.call(p); p._connectors = () => [...base, 'VMLAB-1'];
        p._screensChanged(); p._screensChanged(); return String(p.asked); })()")" "1"
same "7 its Present starts presenting for that screen" "$(present_action) $(pst .presenting)" "clicked true"
same "7 unplugged, presenting ends, the question off or not" \
    "$(ev "(() => { const p = ${P}; p._settings.set_boolean('ask-on-screen', false); const base = Object.getPrototypeOf(p)._connectors.call(p);
        p._connectors = () => base; p._screensChanged(); p._settings.set_boolean('ask-on-screen', true); return String(p.presenting); })()") $(inhibited)" \
    "false (false,)"
same "7 the question's Present after the screen has gone starts nothing" \
    "$(ev "(() => { ${P}.start({connector: 'VMLAB-1'}); delete ${P}._connectors; return String(${P}.presenting); })()")" "false"
# 8: a share running when it starts, as GNOME's indicator holds it: its end ends presenting
SHARE="const GObject = imports.gi.GObject; globalThis.VmlabShare ??= GObject.registerClass({Signals: {stopped: {}}}, class VmlabShare extends GObject.Object {});
    const list = Main.panel.statusArea.quickSettings._remoteAccess._handles;
    const add = h => (list.add ? list.add(h) : list.push(h)); const del = h => (list.delete ? list.delete(h) : list.splice(list.indexOf(h), 1));"
same "8 presenting ends with the share it started during" \
    "$(ev "(() => { ${SHARE} const h = new VmlabShare(); h.is_recording = false; add(h);
        ${P}.start(); const share = ${P}._forShare; h.emit('stopped'); del(h); return [share, ${P}.presenting].join(' '); })()")" \
    "true false"
# 9: Scribble
keys Super_L Alt_L d
same "9 Super+Alt+D opens Scribble, the shell holding input" "$(pst .scribble) $(ev "Main.modalCount")" "true 1"
same "9 the toolbar is 22 px from the bottom, centred" \
    "$(ev "(() => { const b = ${P}.scribble.toolbar; const m = Main.layoutManager.primaryMonitor; return [m.y + m.height - b.y - b.height, Math.trunc(b.x + b.width / 2 - m.x - m.width / 2)].join(' '); })()")" \
    "22 0"
drag 500 300 900 500
keys 2
keys h
drag 500 600 1100 620
same "9 two strokes, the second yellow at .45" \
    "$(ev "(() => { const s = ${P}.scribble; return [s.strokes, s._strokes[1].tool, s._strokes[1].actor.opacity].join(' '); })()")" \
    "2 highlighter 115"
keys Control_L z
same "9 Ctrl+Z takes the last back" "$(pst .strokes)" 1
keys Print
same "9 Print while drawing opens the screenshot bar over the ink" "$(ev "Main.screenshotUI.visible")" true
keys Escape
same "9 closing the bar leaves the ink" "$(pst .scribble) $(pst .strokes)" "true 1"
keys Super_L Alt_L d
same "9 Super+Alt+D again wipes the ink and gives the input back" "$(pst .scribble) $(pst .strokes) $(ev "Main.modalCount")" "false 0 0"
keys Super_L Alt_L d
keys Escape
same "9 Esc does the same" "$(pst .scribble) $(ev "Main.modalCount")" "false 0"
# 10: a real lock while presenting, with one inhibitor of ours before and after
locked() { [[ "$(ev "Main.screenShield?.locked ?? false")" == true ]]; }
unlocked() { [[ "$(ev "Main.sessionMode.currentMode")" == user ]] && ready; }
lock() {
    loginctl lock-session "$(loginctl show-user "${TEST_USER}" -p Display --value)"
    wait_for 30 locked
}
unlock() {
    sleep 2
    loginctl unlock-sessions
    wait_for 30 unlocked
}
lock_and_unlock() { lock && unlock; }
ours() {
    local n=0 path
    for path in $(user_bus gdbus call --session --dest org.gnome.SessionManager --object-path /org/gnome/SessionManager \
        --method org.gnome.SessionManager.GetInhibitors 2>/dev/null | grep -o "/org/gnome/SessionManager/Inhibitor[0-9]*"); do
        user_bus gdbus call --session --dest org.gnome.SessionManager --object-path "${path}" \
            --method org.gnome.SessionManager.Inhibitor.GetReason 2>/dev/null | grep -q "'Presenting'" && n=$((n + 1))
    done
    echo "${n}"
}
ev "${P}.start(); 'started'" >/dev/null
sleep 1
same "10 one inhibitor of ours while presenting" "$(ours)" 1
if lock_and_unlock; then
    sleep 3
    same "10 after a lock, still presenting, one inhibitor" "$(pst .presenting) $(ours)" "true 1"
else
    not_ok "10 the screen did not lock and unlock"
fi
ev "${P}.stop(); 'stopped'" >/dev/null
# 10b: what presenting was for goes during the lock: it does not come back, and nothing keeps the session awake
ev "(() => { const p = ${P}; const base = Object.getPrototypeOf(p)._connectors.call(p); p._connectors = () => [...base, 'VMLAB-2']; p.start({connector: 'VMLAB-2'}); return 'started'; })()" >/dev/null
if lock_and_unlock; then
    sleep 3
    same "10b the screen presented for is gone after the lock: not presenting, not inhibited" "$(pst .presenting) $(inhibited)" "false (false,)"
else
    not_ok "10b the screen did not lock and unlock"
fi
ev "(() => { ${SHARE} globalThis.vmlabShare = new VmlabShare(); vmlabShare.is_recording = false; add(vmlabShare); ${P}.start(); return 'started'; })()" >/dev/null
if lock; then
    ev "(() => { ${SHARE} del(vmlabShare); return 'ended'; })()" >/dev/null
    if unlock; then
        sleep 3
        same "10b the share presented for ended during the lock: not presenting, not inhibited" "$(pst .presenting) $(inhibited)" "false (false,)"
    else
        not_ok "10b the screen did not unlock"
    fi
else
    not_ok "10b the screen did not lock"
fi
ev "${P}.stop(); 'stopped'" >/dev/null

# --- R4-17 Pin a capture -----------------------------------------------------------------------------------------------
same "11 the screenshot schema" "$(in_session "gsettings list-keys ${SHOT}" | sort | paste -sd ' ') $(gs get "${SHOT}" capture-thumbnail) $(gs get "${SHOT}" capture-pin)" \
    "capture-pin capture-thumbnail tool-settings true true"
# The slide in, measured with animations forced on (the lab draws in software and turns them off)
ev "global.force_animations = true; 'forced'" >/dev/null
# Watched every 10 ms from before the capture: the card comes once the file is written, which a fixed wait can miss
ev "(() => { const s = ${S}; const old = s._card; globalThis.vmlabSlid = false; let n = 0;
    imports.gi.GLib.timeout_add(0, 10, () => { const c = s._card; if (c && c !== old && c.actor.translation_y > 0) vmlabSlid = true;
    return !vmlabSlid && ++n < 500; }); return 'watching'; })()" >/dev/null
capture 400 300 500 300 normal 0.5
same "12 the thumbnail slides in from below" "$(ev "String(globalThis.vmlabSlid)")" true
ev "global.force_animations = true; 'back'" >/dev/null
sleep 1
same "12 the thumbnail: 210 px, 16 px from the corner, a 110 px picture, its buttons" \
    "$(card | jq -c '[.width, .right, .bottom, .picture, (.buttons | map(select(. != "Edit")))]')" \
    '[210,16,16,110,["Copy","Pin on top","Show in Files","Delete"]]'
same "12 no notification of GNOME's beside it" "$(ev "Main.messageTray.getSources().filter(s => s.title === 'Screen Capture').length")" 0
ev "${S}._card.actor.hover = true; ${S}._card._stopTimer(); 'held'" >/dev/null
sleep 7
check "12 it stays while the pointer is on it" test "$(card)" != none
ev "${S}._card._startTimer(); 'released'" >/dev/null
check "12 and goes six seconds after" wait_for 9 no_card
button_at() { ev "(() => { const b = ${S}._card.actor.get_child_at_index(1).get_children().find(x => x.accessible_name === '$1'); const [x, y] = b.get_transformed_position(); return (x + b.width / 2) + ' ' + (y + b.height / 2); })()"; }
capture 400 300 500 300
ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.CLIPBOARD, 'vmlab'); 'text'" >/dev/null
# shellcheck disable=SC2046
click $(button_at Copy)
same "13 Copy puts the picture on the clipboard" "$(ev "imports.gi.St.Clipboard.get_default().get_mimetypes(imports.gi.St.ClipboardType.CLIPBOARD).includes('image/png')")" true
capture 400 300 500 300
user_bus timeout 6 dbus-monitor --session "type='method_call',interface='org.freedesktop.FileManager1',member='ShowItems'" \
    >"${LAB}/showitems" 2>/dev/null &
sleep 1
# shellcheck disable=SC2046
click $(button_at 'Show in Files')
sleep 5
check "13 Show in Files asks the file manager to show it" grep -q ShowItems "${LAB}/showitems"
capture 400 300 500 300
file="$(ev "${S}._card._file.get_path()")"
# shellcheck disable=SC2046
click $(button_at Delete)
sleep 2
check "13 Delete moves the capture to the trash" test ! -e "${file}" -a -n "$(ls "${HOME_DIR}/.local/share/Trash/files/" 2>/dev/null | grep -F "$(basename "${file}")")"
capture 400 300 500 300
# shellcheck disable=SC2046
click $(button_at 'Pin on top')
sleep 1
same "14 a pin from the thumbnail, within 40 % of the work area" \
    "$(ev "(() => { const p = ${S}.pins().at(-1); const a = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex); return [p.width <= a.width * 0.4 + 1, p.height <= a.height * 0.4 + 1].join(' '); })()")" \
    "true true"
pin_centre() { ev "(() => { const a = ${S}._pins.list.at(-1).actor; return Math.round(a.x + a.width / 2) + ' ' + Math.round(a.y + a.height / 2); })()"; }
read -r px py <<<"$(pin_centre)"
w0="$(pins '.at(-1).width')"
scroll "${px}" "${py}" UP
same "14 a wheel step up grows it by a tenth" "$(ev "Math.round(${S}.pins().at(-1).width / ${w0} * 100)")" 110
keys_down Control_L
scroll "${px}" "${py}" DOWN
keys_up Control_L
same "14 Ctrl and a wheel step down make it a tenth more see-through" "$(pins '.at(-1).opacity')" 0.9
before="$(pins '.at(-1)' | jq -c '[.x, .y]')"
drag "${px}" "${py}" $((px + 100)) $((py + 50))
same "14 a drag moves it" "$(pins '.at(-1)' | jq -c '[.x, .y]')" "$(jq -c '[.[0] + 100, .[1] + 50]' <<<"${before}")"
pin_button() { ev "(() => { const b = ${S}._pins.list.at(-1).pill.get_children().find(x => x.accessible_name === '$1'); const [x, y] = b.get_transformed_position(); return Math.round(x + b.width / 2) + ' ' + Math.round(y + b.height / 2); })()"; }
# Over the counting window, clicks pass through to it
ev "(() => { const w = global.display.list_all_windows().find(w => w.title === 'org.vmlab.Capture'); w.activate(global.get_current_time()); const r = w.get_frame_rect(); const pin = ${S}._pins.list.at(-1); pin.actor.set_position(r.x + r.width / 2 - 100, r.y + r.height / 2 - 75); pin.actor.set_size(200, 150); return 'over'; })()" >/dev/null
sleep 1
# shellcheck disable=SC2046
click $(pin_button 'Clicks pass through')
same "14 the arrow lets clicks pass through, its ring amber" \
    "$(ev "(() => { const a = ${S}._pins.list.at(-1).actor; return [a.reactive, a.has_style_class_name('amethystora-screenshot-pin-through')].join(' '); })()")" "false true"
# The window counts in memory from its start (step 5's click was its first): one more
clicks_before="$(clicks)"
# shellcheck disable=SC2046
click $(pin_centre)
same "14 a click on it lands on the window below" "$(clicks)" $((clicks_before + 1))
n="$(pins .length)"
# shellcheck disable=SC2046
click $(pin_button Close)
same "14 the cross lets it go" "$(pins .length)" $((n - 1))
ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.CLIPBOARD, 'vmlab'); 'text'" >/dev/null
shots_before="$(in_session 'ls "$(xdg-user-dir PICTURES)"/*/*.png 2>/dev/null | wc -l')"
capture 300 200 600 400 pin
same "15 Ctrl+P pins the area exactly where it was" "$(pins '.at(-1)' | jq -c '[.x, .y, .width, .height]')" "[300,200,600,400]"
same "15 and saves nothing" "$(in_session 'ls "$(xdg-user-dir PICTURES)"/*/*.png 2>/dev/null | wc -l')" "${shots_before}"
same "15 and leaves the clipboard as it was" "$(ev "imports.gi.St.Clipboard.get_default().get_mimetypes(imports.gi.St.ClipboardType.CLIPBOARD).includes('image/png')")" false
first="$(ev "${S}._pins.list[0].state.x + ',' + ${S}._pins.list[0].state.y")"
ev "(() => { const s = ${S}; const p = s._pins.list.at(-1).picture; for (let i = 0; i < 6; i++) s._pins.pin(p, {x: 7 + 10 * i, y: 7 + 10 * i, width: 100, height: 80}); return 'seven'; })()" >/dev/null
same "16 six at most, the first let go" "$(pins .length) $(ev "String(${S}.pins().some(p => p.x + ',' + p.y === '${first}'))")" "6 false"
gs set "${SHOT}" capture-thumbnail false
capture 400 300 500 300
same "17 thumbnail off: GNOME's words, no card" \
    "$(card) $(ev "Main.messageTray.getSources().find(s => s.title === 'Screen Capture')?.notifications.at(-1)?.title ?? 'none'")" \
    "none Screenshot captured"
gs set "${SHOT}" capture-thumbnail true
gs set "${SHOT}" capture-pin false
same "17 pinning off: every pin let go" "$(pins .length)" 0
gs set "${SHOT}" capture-pin true
capture 300 200 600 400 pin
ev "(() => { const s = ${S}; s._pins.pin(s._pins.list[0].picture, {x: 900, y: 500, width: 200, height: 150}); s._pins.list[1].setOpacity(0.4); return 'two'; })()" >/dev/null
before="$(pins | jq -c 'map([.x, .y, .width, .height, .opacity])')"
if lock_and_unlock; then
    sleep 3
    same "18 after a lock, the two pins where they were" "$(pins | jq -c 'map([.x, .y, .width, .height, .opacity])')" "${before}"
else
    not_ok "18 the screen did not lock and unlock"
fi
same "19 a share marks and blurs every pin, and its end clears them" \
    "$(ev "(() => { ${SHARE} const h = new VmlabShare(); h.is_recording = false; ${S}._pins._capture._onNewHandle(h);
        const on = ${S}._pins.list.every(p => p.state.shared && p._image.get_effect('amethystora-share-blur') !== null);
        h.emit('stopped'); const off = ${S}._pins.list.every(p => !p.state.shared && !p._image.get_effect('amethystora-share-blur'));
        return [on, off].join(' '); })()")" \
    "true true"
same "19 a thumbnail taken during a share is blurred" \
    "$(ev "(() => { ${SHARE} globalThis.vmlabShare3 = new VmlabShare(); vmlabShare3.is_recording = false; add(vmlabShare3); return 'sharing'; })()") $(capture 400 300 500 300; ev "String(${S}._card?._image.get_effect('amethystora-share-blur') !== null)")" \
    "sharing true"
ev "(() => { ${SHARE} del(vmlabShare3); return 'ended'; })()" >/dev/null
gs set "${SHOT}" capture-pin false
gs set "${SHOT}" capture-pin true

# --- R4-19 Widgets on the desktop ----------------------------------------------------------------------------------------
sleep 3
inject_plugins
same "20 off: no card, and the bar draws only the bar's plugin" \
    "$(ev "JSON.stringify(${W}.cards())") $(ev "Object.keys(Main.panel.statusArea).filter(k => k.startsWith('amethystora-plugin-vmlab')).join(',')")" \
    "[] amethystora-plugin-vmlab.bar"
in_session "ame desktop desktop-widgets on" >/dev/null
cards_up() { [[ "$(ev "${W}.cards().filter(c => c.text !== null).length")" == 3 ]]; }
check "21 on: the three cards drawn within 10 s" wait_for 10 cards_up
same "21 the clock top left and the build top right, 16 px in" \
    "$(ev "(() => { const a = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex); const c = ${W}.cards(); const clock = c.find(x => x.id === 'vmlab.clock'); const build = c.find(x => x.id === 'vmlab.build'); return [clock.x - a.x, clock.y - a.y, a.x + a.width - build.x - build.width, build.y - a.y].join(' '); })()")" \
    "16 16 16 16"
right_under_windows() {
    [[ "$(ev "(() => { const g = global.window_group.get_children(); const i = g.indexOf(${W}._layer); const first = g.findIndex(c => c instanceof imports.gi.Meta.WindowActor); return String(first < 0 ? i === g.length - 1 : i === first - 1); })()")" == true ]]
}
check "21 the cards' layer is right under the first window" right_under_windows
under() {
    ev "(() => { const b = ${W}.cards().find(x => x.id === 'vmlab.build'); const hit = global.stage.get_actor_at_pos(imports.gi.Clutter.PickMode.REACTIVE, b.x + 10, b.y + 10); return String(hit?.constructor.name); })()"
}
ev "(() => { const w = global.display.list_all_windows().find(w => w.title === 'org.vmlab.Capture'); w.unminimize(); const a = Main.layoutManager.getWorkAreaForMonitor(0); w.move_frame(true, a.x + a.width - 600, a.y); w.activate(global.get_current_time()); return 'moved'; })()" >/dev/null
sleep 1
check "21 a window over a card covers it" like "$(under)" "*SurfaceActor*"
user_bus systemd-run --user --unit vmlab-win2 --collect -- gjs -m "${LAB}/win.js" org.vmlab.Other >/dev/null
sleep 4
ev "global.display.list_all_windows().find(w => w.title === 'org.vmlab.Capture')?.activate(global.get_current_time()); 'raised'" >/dev/null
sleep 1
check "21 and still covers it after a restack" like "$(under)" "*SurfaceActor*"
check "21 and the layer is right under the first window again" right_under_windows
# Both windows out of the way: the cards sit on the wallpaper, under every window
ev "global.display.list_all_windows().filter(w => ['org.vmlab.Capture', 'org.vmlab.Other'].includes(w.title)).forEach(w => w.minimize()); 'minimized'" >/dev/null
sleep 1
same "22 the clock's first line is drawn as a title, the second as a line" \
    "$(ev "(() => { const c = ${W}._cards.get('vmlab.clock').actor.get_children(); return [c[0].has_style_class_name('amethystora-deskwidgets-title'), c[1].has_style_class_name('amethystora-deskwidgets-line')].join(' '); })()")" \
    "true true"
same "22 the build's last line is in the palette's green" \
    "$(ev "${W}._cards.get('vmlab.build').actor.get_children().at(-1).style")" "color: $(sed -n 's/^color2 = "\(.*\)"/\1/p' "${HOME_DIR}/.config/amethystora/current/theme/colors.toml");"
same "22 the name and at most six lines of a long text" "$(ev "${W}._cards.get('vmlab.menu').actor.get_n_children()")" 7
rm -f "${LAB}/vmlab-build-clicked"
card_centre() { ev "(() => { const b = ${W}.cards().find(x => x.id === '$1'); return Math.round(b.x + b.width / 2) + ' ' + Math.round(b.y + b.height / 2); })()"; }
# What the pointer meets at a card's centre, and the modal count: says why when a click there does nothing
reaches() {
    local r
    r="$(ev "(() => { const b = ${W}.cards().find(x => x.id === '$1'); const card = ${W}._cards.get('$1').actor;
        const hit = global.stage.get_actor_at_pos(imports.gi.Clutter.PickMode.REACTIVE, Math.round(b.x + b.width / 2), Math.round(b.y + b.height / 2));
        let win = hit; while (win && !win.meta_window) win = win.get_parent();
        return [hit === card || card.contains(hit), hit?.constructor.name, hit?.name ?? '', Main.modalCount, win?.meta_window?.get_title() ?? '', win?.meta_window?.minimized ?? ''].join(' '); })()")"
    if [[ ${r} == "true "*" 0 "* ]]; then ok "22 a click on $1 reaches its card"; else not_ok "22 a click on $1 reaches its card" "found: ${r}"; fi
}
reaches vmlab.build
reaches vmlab.menu
# shellcheck disable=SC2046
click $(card_centre vmlab.build)
check "22 a click runs its click" wait_for 5 test -e "${LAB}/vmlab-build-clicked"
# shellcheck disable=SC2046
click $(card_centre vmlab.menu)
same "22 a click on a card with a menu opens it" "$(ev "String(${W}._cards.get('vmlab.menu')._menu?.isOpen ?? false)")" true
keys Escape
ev "(() => { ${PL}._list([{id: 'vmlab.build', name: 'Build', places: {desktop: {corner: 'top-right'}}, error: 'it printed nothing', output: null}, {id: 'vmlab.clock', name: 'Clock', places: {desktop: {corner: 'top-left'}}, output: {text: 'x', lines: ['x']}}]); return 'broken'; })()" >/dev/null
broken() { [[ "$(ev "${W}.cards().find(x => x.id === 'vmlab.build')?.error ?? ''")" == *printed* ]]; }
check "23 a plugin that failed shows its warning card" wait_for 5 broken
inject_plugins
# A workspace's own picture: the cards stay over it (with the tiling on, which draws it)
gs set org.gnome.shell.extensions.amethystora-ui.tiling enabled true
picture="$(find /usr/share/backgrounds -name '*.jpg' -o -name '*.png' 2>/dev/null | head -n 1)"
gs set org.gnome.shell.extensions.amethystora-ui.tiling workspace-backgrounds "['${picture}']"
sleep 3
same "21 over a workspace's own picture" \
    "$(ev "(() => { const g = global.window_group.get_children(); const pictures = ${UI}.get('tiling')?.engine?.backgrounds?.layer; return String(!pictures || g.indexOf(${W}._layer) > g.indexOf(pictures)); })()")" true
gs reset org.gnome.shell.extensions.amethystora-ui.tiling workspace-backgrounds
gs set org.gnome.shell.extensions.amethystora-ui.tiling enabled false
# Plugins off in the shell: the cards go, on again and handed their list once more: they come back
gs set org.gnome.shell.extensions.amethystora-ui.plugins show false
sleep 2
same "24 with plugins not drawn, no card" "$(ev "${W}.cards().length")" 0
gs set org.gnome.shell.extensions.amethystora-ui.plugins show true
sleep 1
inject_plugins
cards_back() { [[ "$(ev "${W}.cards().length")" -gt 0 ]]; }
check "24 and with it on again, they are back" wait_for 10 cards_back

# --- the light theme and a theme switch ----------------------------------------------------------------------------------
theme="$(in_session "amethystora-theme current" 2>/dev/null)"
in_session "ame desktop theme amethystora-light" >/dev/null 2>&1
check "theme: the light theme's present, widgets and screenshot colours are rendered" \
    test -s "${HOME_DIR}/.config/amethystora/current/theme/shell-present.css" -a -s "${HOME_DIR}/.config/amethystora/current/theme/shell-deskwidgets.css"
in_session "ame desktop theme ${theme:-amethystora}" >/dev/null 2>&1

# --- 25: ten disables and enables, with everything on ----------------------------------------------------------------------
ORIGINALS="(() => { const ui = Main.screenshotUI; const p = Object.getPrototypeOf(ui); return [!Object.hasOwn(ui, 'open'), !Object.hasOwn(ui, '_saveScreenshot')].join(' '); })()"
ev "${P}.start(); ${P}.toggleScribble(); 'all on'" >/dev/null
for _ in $(seq 10); do
    ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
    sleep 1
    ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
    sleep 2
done
same "25 after ten cycles: nothing of ours left over, one layer of cards, no grab, not presenting" \
    "$(ev "(() => { const ui = Main.layoutManager.uiGroup; const ours = c => ['amethystora-present-halo', 'amethystora-screenshot-pin', 'amethystora-screenshot-thumb'].some(n => c.has_style_class_name?.(n)) || ['amethystoraTorch', 'amethystoraScribble'].includes(c.name);
        const left = ui.get_children().filter(ours).length; const layers = global.window_group.get_children().filter(c => c.name === 'amethystoraDeskWidgets').length;
        return [left, layers, Main.modalCount, ${P}.presenting].join(' '); })()")" \
    "0 1 0 false"
ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
sleep 1
same "25 off: GNOME's screenshot methods are its own again, the key is gone" \
    "$(ev "${ORIGINALS}") $(ev "String(!Main.wm._allowedKeybindings?.['scribble'])")" "true true true"
ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
sleep 2
in_session "ame desktop present off" >/dev/null
same "25 Present mode off: its key is gone" "$(ev "String(!Main.wm._allowedKeybindings?.['scribble'])")" true
ok "ran on $(gnome-shell --version 2>/dev/null); a GNOME 51 image (the beta stream) runs these same steps"

# --- and nothing threw ---------------------------------------------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: the (present|deskwidgets|screenshot) module|widgets on the desktop did not start" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of capture threw"; else not_ok "capture threw" "${errors}"; fi
finish
