#!/usr/bin/bash
# vmlab-vm: r5-windows
# Rounds 5 and 6's windows group (plans/ui-ideas/spec-windows.md, section 11), in the test account's session, asked of
# GNOME Shell through Eval (the lab-only vmlab-eval@lab extension, never in the image) and driven with keys.py's
# keyboard and pointer, and a wheel of the shell's own virtual pointer (keys.py has none):
#   57.1  the schema's new keys and defaults, the parts running, the keys bound;
#   57.2  the drop-down terminal: Super+Alt+Return starts kitty on every workspace and above, at the top of the work
#         area, with the keys; again hides it, minimized and still running; again shows it on another workspace, sliding;
#   57.3  the scratchpad: Move to the scratchpad in the window menu, shown and hidden with the terminal, Take out, the
#         tiling giving the window back and keeping the terminal, drop-down off giving the terminal back;
#   57.4  keep on top: Super+Alt+P, the ring and the pin above the window, a real click on the pin, GNOME's own make_above,
#         keep-on-top off;
#   57.5  see-through windows: Super+Alt and the wheel in 5 % steps down to 40 % and back, the window actor's own opacity
#         left alone, no workspace switch, a rule for an app's new window, the share guard's cover keeping it solid, off;
#   57.6  the frame: below nothing and above the focused window in the classic layout, following the focus, none when
#         maximized or in the tiling;
#   57.7  ame desktop window-settings: GNOME's keys, refusals, dialogs refused under the tiling;
#   57.8  Size for recording: 1280 x 720 centred in the work area, from the window menu;
#   57.9  reopen: a closed window's app back on its workspace at its place with Super+Alt+T, five deep, nothing on disk;
#   57.10 a screen lock keeps the drop-down terminal; ten disables leave nothing behind;
#   57.11 Control's entries, each an ame command;
#   57.12 the lights over kitty's own window buttons: three, in GNOME's order, kitty's buttons clipped off its title bar,
#         grey in the background, a real click on each, none on the drop-down terminal;
#   and nothing of Amethystora UI throws meanwhile.
# vmlab-paths: @ui/extension.js @ui/modules/windows.js @ui/modules/windows/* @ui/modules/privacy/guard.js @ui/schemas/*windows* @ui/stylesheets/windows.css @sf/*themed/shell-windows* @sf/etc/xdg/kitty/* @sf/*just/desktop.just @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
SCHEMA=org.gnome.shell.extensions.amethystora-ui.windows
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c 'import json, sys
from gi.repository import GLib
v = GLib.Variant.parse(None, sys.stdin.read().strip(), None, None)
if not v[0]:
    print("EVAL FAILED: " + v[1])
    sys.exit(1)
try:
    x = json.loads(v[1])
except ValueError:
    x = v[1]
print(("true" if x else "false") if isinstance(x, bool) else x if isinstance(x, (str, int, float)) else json.dumps(x, separators=(",", ":")))'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
ame_() { in_session ame "$@"; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
is_true() { [[ "$(ev "$1")" == true ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
W="${UI}.get('windows')"
WIN="global.get_window_actors().map(a => a.meta_window)"
byclass() { echo "${WIN}.filter(w => w.get_wm_class() === '$1').sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence())[0]"; }
count_class() { ev "${WIN}.filter(w => w.get_wm_class() === '$1').length"; }
at_least() { (($(count_class "$1") >= $2)); }
none_of() { [[ "$(count_class "$1")" == 0 ]]; }
refused() { ! in_session ame desktop window-settings "$@" >/dev/null 2>&1; }
rect_of() { ev "(() => { const r = $1.get_frame_rect(); return [r.x, r.y, r.width, r.height].join(','); })()"; }
AREA="Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex)"
T="${W}.scratch.terminal"
classic() { ame_ desktop layout classic >/dev/null; sleep 2; }
tiling() { ame_ desktop layout tiling >/dev/null; sleep 2; }
# The shell's own virtual devices, made once: a wheel and Super+Alt held round it (keys.py's pointer has no wheel)
wheel() {
    ev "(() => { const C = imports.gi.Clutter, GLib = imports.gi.GLib; const seat = global.stage.context.get_backend().get_default_seat();
        globalThis.vmlabWheel ??= seat.create_virtual_device(C.InputDeviceType.POINTER_DEVICE);
        globalThis.vmlabKeys ??= seat.create_virtual_device(C.InputDeviceType.KEYBOARD_DEVICE);
        const t = () => GLib.get_monotonic_time(); seat.warp_pointer($1, $2);
        const dir = C.ScrollDirection.$3, n = $4;
        GLib.timeout_add(0, 50, () => { vmlabKeys.notify_keyval(t(), C.KEY_Super_L, C.KeyState.PRESSED); vmlabKeys.notify_keyval(t(), C.KEY_Alt_L, C.KeyState.PRESSED); return false; });
        for (let i = 0; i < n; i++) GLib.timeout_add(0, 120 + 40 * i, () => { vmlabWheel.notify_discrete_scroll(t(), dir, C.ScrollSource.WHEEL); return false; });
        GLib.timeout_add(0, 160 + 40 * n, () => { vmlabKeys.notify_keyval(t(), C.KEY_Alt_L, C.KeyState.RELEASED); vmlabKeys.notify_keyval(t(), C.KEY_Super_L, C.KeyState.RELEASED); return false; });
        return 'wheel'; })()" >/dev/null
    sleep "$(((300 + 40 * $4) / 1000 + 1))"
}
# The point at a fraction of a window's frame, as pixels and as keys.py's fractions of the screen
point_in() { ev "(() => { const f = $1.get_frame_rect(); return Math.round(f.x + f.width * $2) + ',' + Math.round(f.y + f.height * $3); })()"; }
frac_of() { ev "(() => { const [x, y] = [$1]; return (x / global.screen_width).toFixed(5) + ',' + (y / global.screen_height).toFixed(5); })()"; }

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
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
if [[ ${enabled} != *vmlab-eval@lab* ]]; then
    user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session
    sleep 15
fi
ev "Main.overview.hide(); 'hidden'" >/dev/null
# The lab renders in software, where GNOME turns animations off: the slides are what is tested here
ev "global.force_animations = true; 'forced'" >/dev/null
was_tiling="$(gs get org.gnome.shell.extensions.amethystora-ui.tiling enabled)"
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"

# A plain window, of an app with a launcher of its own (reopen asks for one), any number of windows at once
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/applications"
cat >/tmp/vmlab-plain57.js <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: 'org.vmlab.Plain57', flags: 4});
app.connect('activate', () => { const w = new Gtk.ApplicationWindow({application: app, title: 'Plain57', default_width: 640, default_height: 400}); w.present(); });
app.run([]);
EOF
chmod 0644 /tmp/vmlab-plain57.js
cat >"${HOME_DIR}/.local/share/applications/org.vmlab.Plain57.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Plain57
Exec=gjs -m /tmp/vmlab-plain57.js
StartupWMClass=org.vmlab.Plain57
EOF
chown "${TEST_USER}:" "${HOME_DIR}/.local/share/applications/org.vmlab.Plain57.desktop"
APP="Shell.AppSystem.get_default().lookup_app('org.vmlab.Plain57.desktop')"
wait_for 20 is_true "!!imports.gi.${APP}"
open_plain() { ev "imports.gi.${APP}.open_new_window(-1); 'o'" >/dev/null; }
P="$(byclass org.vmlab.Plain57)"
classic
open_plain
wait_for 60 at_least org.vmlab.Plain57 1
sleep 2
ev "Main.activateWindow(${P}); 'a'" >/dev/null

# --- 57.1 Keys and defaults -----------------------------------------------------------------------------------------
same "the new keys and their defaults" \
    "$(gs list-recursively "${SCHEMA}" | grep -E " (drop-down|scratchpad|keep-on-top|see-through|frame|recording-sizes|reopen)" | cut -d' ' -f2- | LC_ALL=C sort | paste -sd ';')" \
    "drop-down true;drop-down-key ['<Super><Alt>Return'];frame false;keep-on-top true;keep-on-top-key ['<Super><Alt>p'];recording-sizes true;reopen true;reopen-key ['<Super><Alt>t'];scratchpad true;see-through false;see-through-apps @a{si} {}"
check "the parts that are on run" is_true "['scratch', 'ontop', 'sizes', 'reopen'].every(p => ${W}.running(p)) && !${W}.running('seeThrough') && !${W}.running('frame')"
check "the three keys are bound" is_true "['drop-down-key', 'keep-on-top-key', 'reopen-key'].every(k => (Main.wm._allowedKeybindings[k] ?? 0) !== 0)"

# --- 57.2 The drop-down terminal ------------------------------------------------------------------------------------
keys super+alt+return
check "Super+Alt+Return starts the drop-down terminal" wait_for 30 is_true "!!${T} && !${T}.minimized"
sleep 1
check "it is kitty, on every workspace and above the rest" is_true "${T}.get_wm_class() === 'org.amethystora.DropDown' && ${T}.is_on_all_workspaces() && ${T}.is_above()"
same "it lies along the top of the work area, 14 px in, half its height" "$(rect_of "${T}")" \
    "$(ev "(() => { const a = ${AREA}; return [a.x + 14, a.y, a.width - 28, Math.round(a.height / 2)].join(','); })()")"
check "it has the keys" wait_for 5 is_true "global.display.focus_window === ${T}"
pid="$(ev "${T}.get_pid()")"
keys super+alt+return
check "again hides it, minimized" wait_for 5 is_true "${T}.minimized"
check "and still running, the same kitty" is_true "${T}.get_pid() === ${pid}"
ev "global.workspace_manager.get_workspace_by_index(1).activate(global.get_current_time()); 'ws'" >/dev/null
sleep 1
ev "globalThis.vmlabSlide = []; const a = ${T}.get_compositor_private(); globalThis.vmlabSlideId = a.connect('notify::translation-y', () => vmlabSlide.push(Math.round(a.translation_y))); 'x'" >/dev/null
keys super+alt+return
check "again shows it on the second workspace" wait_for 5 is_true "!${T}.minimized && ${T}.get_compositor_private().visible"
sleep 1
check "sliding down from above the screen" is_true "vmlabSlide.length > 3 && vmlabSlide[0] < -100 && vmlabSlide.at(-1) === 0"
ev "${T}.get_compositor_private().disconnect(vmlabSlideId); global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 'x'" >/dev/null
sleep 1

# --- 57.3 The scratchpad --------------------------------------------------------------------------------------------
menu_line() {
    ev "(() => { const w = $1; Main.wm._windowMenuManager.showWindowMenuForWindow(w, imports.gi.Meta.WindowMenuType.WM, w.get_frame_rect());
        const m = Main.wm._windowMenuManager._manager.activeMenu; const it = m?._getMenuItems().find(i => i.label?.text === '$2');
        if (!it) { m?.close(); return 'none'; } it.activate(imports.gi.Clutter.get_current_event()); return 'chosen'; })()"
}
same "the window menu offers Move to the scratchpad" "$(menu_line "${P}" 'Move to the scratchpad')" chosen
check "the window goes out of sight, on every workspace and above" wait_for 5 is_true "${P}.minimized && ${P}.is_on_all_workspaces() && ${P}.is_above() && ${W}.scratch.pad.includes(${P})"
check "places, show desktop and the motion count it as kept" is_true "${W}.isKept(${P})"
check "it has no ring: the shell keeps it above" is_true "!${W}.ontop.ringOf(${P})"
ev "Main.activateWindow(${T}); 'a'" >/dev/null
sleep 1
keys super+alt+return
check "the key hides the terminal and the scratchpad together" wait_for 5 is_true "${T}.minimized && ${P}.minimized"
keys super+alt+return
check "and shows them together" wait_for 5 is_true "!${T}.minimized && !${P}.minimized"
same "the window menu offers Take out of the scratchpad" "$(menu_line "${P}" 'Take out of the scratchpad')" chosen
check "Take out gives the window back as it was" wait_for 5 is_true "!${P}.minimized && !${P}.is_on_all_workspaces() && !${P}.is_above() && !${W}.scratch.pad.includes(${P})"
menu_line "${P}" 'Move to the scratchpad' >/dev/null
wait_for 5 is_true "${W}.scratch.pad.includes(${P})"
tiling
check "the tiling gives the scratchpad's window back and keeps the terminal" wait_for 5 is_true "!${W}.scratch.pad.length && !${P}.is_on_all_workspaces() && !!${T} && ${T}.is_on_all_workspaces()"
check "the tiling does not put the terminal on its strip" is_true "!${UI}.get('tiling').engine.isTiled(${T})"
classic
ame_ desktop windows drop-down off >/dev/null
sleep 1
check "drop-down off gives the terminal back as an ordinary window" wait_for 5 is_true "!${T} && ${WIN}.some(w => w.get_wm_class() === 'org.amethystora.DropDown' && !w.is_above() && !w.is_on_all_workspaces() && !w.minimized)"
# The scratchpad shares the key (spec 1, decision 5): it goes with both off
ame_ desktop windows scratchpad off >/dev/null
sleep 1
check "and the key goes" is_true "(Main.wm._allowedKeybindings['drop-down-key'] ?? 0) === 0"
ev "${WIN}.filter(w => w.get_wm_class() === 'org.amethystora.DropDown').forEach(w => w.delete(global.get_current_time())); 'x'" >/dev/null
ame_ desktop windows drop-down on >/dev/null
ame_ desktop windows scratchpad on >/dev/null
sleep 1

# --- 57.4 Keep on top -----------------------------------------------------------------------------------------------
ev "Main.activateWindow(${P}); 'a'" >/dev/null
sleep 1
keys super+alt+p
check "Super+Alt+P keeps the window on top" wait_for 5 is_true "${P}.is_above()"
check "with a ring above its actor and a glow below" wait_for 5 is_true "(() => { const r = ${W}.ontop.ringOf(${P}); const a = ${P}.get_compositor_private(); const p = a.get_parent(); const kids = p.get_children(); return !!r && r.ring.visible && kids.indexOf(r.ring) === kids.indexOf(a) + 1 && kids.indexOf(r.glow) === kids.indexOf(a) - 1; })()"
same "the ring stands 2 px out of the frame" "$(ev "(() => { const r = ${W}.ontop.ringOf(${P}).ring; return [r.x, r.y, r.width, r.height].join(','); })()")" \
    "$(ev "(() => { const f = ${P}.get_frame_rect(); return [f.x - 2, f.y - 2, f.width + 4, f.height + 4].join(','); })()")"
pin="$(ev "(() => { const [x, y] = ${W}.ontop.ringOf(${P}).pin.get_transformed_position(); const s = ${W}.ontop.ringOf(${P}).pin.width / 2; return Math.round(x + s) + ',' + Math.round(y + s); })()")"
keys "click:$(frac_of "${pin}")"
check "a click on the pin lets it go, and the ring goes" wait_for 5 is_true "!${P}.is_above() && !${W}.ontop.ringOf(${P})"
ev "${P}.make_above(); 'x'" >/dev/null
check "GNOME's own Always on Top draws the ring too" wait_for 5 is_true "!!${W}.ontop.ringOf(${P})"
ame_ desktop windows keep-on-top off >/dev/null
check "keep-on-top off: no ring, no key, the window still on top" wait_for 5 is_true "!${W}.running('ontop') && ${P}.is_above() && (Main.wm._allowedKeybindings['keep-on-top-key'] ?? 0) === 0"
ame_ desktop windows keep-on-top on >/dev/null
ev "${P}.unmake_above(); 'x'" >/dev/null

# --- 57.5 See-through windows ---------------------------------------------------------------------------------------
level() { ev "${W}.seeThrough.level(${P})"; }
surface() { ev "${P}.get_compositor_private().get_children().filter(c => /Surface(Container)?Actor/.test(c.constructor.name)).map(c => c.opacity).join(',')"; }
centre="$(point_in "${P}" 0.5 0.6)"
ws="$(ev "global.workspace_manager.get_active_workspace_index()")"
wheel ${centre/,/ } DOWN 2
same "off, Super+Alt and the wheel do nothing to the window" "$(level)" 100
ame_ desktop windows see-through on >/dev/null
sleep 1
ev "Main.activateWindow(${P}); 'a'" >/dev/null
wheel ${centre/,/ } DOWN 4
same "on, four steps down make it 80 %" "$(level)" 80
same "the window's surfaces are drawn at 80 %" "$(surface)" 204
same "the window actor's own opacity is left alone" "$(ev "${P}.get_compositor_private().opacity")" 255
same "the workspace did not change" "$(ev "global.workspace_manager.get_active_workspace_index()")" "${ws}"
wheel ${centre/,/ } DOWN 20
same "it goes no lower than 40 %" "$(level)" 40
wheel ${centre/,/ } UP 30
same "and back up to solid" "$(level)" 100
ame_ desktop windows see-through-app org.vmlab.Plain57.desktop 70 >/dev/null
sleep 1
check "a rule gives the app's windows its level at once" wait_for 5 is_true "${W}.seeThrough.level(${P}) === 70"
open_plain
wait_for 30 at_least org.vmlab.Plain57 2
P2="${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain57').sort((a, b) => b.get_stable_sequence() - a.get_stable_sequence())[0]"
check "and to a new one" wait_for 10 is_true "${W}.seeThrough.level(${P2}) === 70"
ev "${P2}.delete(global.get_current_time()); 'x'" >/dev/null
ame_ desktop windows see-through-app org.vmlab.Plain57.desktop off >/dev/null
# A cover of the share guard, as it would be while the screen is shared: solid while it covers
ev "(() => { const g = ${UI}.get('privacy')._guard; const a = ${P}.get_compositor_private(); g._covering = true; g._decide = () => {}; g._covers.set(a, {destroy() {}, window: ${P}}); return 'covered'; })()" >/dev/null
same "a window the share guard covers is solid" "$(surface)" 255
same "and the wheel does nothing to it" "$(ev "${W}.seeThrough.step(${P}, -2)")" 70
ev "(() => { const g = ${UI}.get('privacy')._guard; g._covers.delete(${P}.get_compositor_private()); g._covering = false; delete g._decide; return 'uncovered'; })()" >/dev/null
same "uncovered, it has its level back" "$(surface)" "$(ev "Math.round(255 * 70 / 100)")"
ame_ desktop windows see-through off >/dev/null
sleep 1
same "off, every window is solid again" "$(surface)" 255

# --- 57.6 The frame -------------------------------------------------------------------------------------------------
ame_ desktop windows frame on >/dev/null
sleep 1
ev "Main.activateWindow(${P}); 'a'" >/dev/null
check "the window in use has the frame, directly above its actor" wait_for 5 is_true "(() => { const f = ${W}.frame; const a = ${P}.get_compositor_private(); const kids = a.get_parent().get_children(); return f.window === ${P} && f.area.visible && kids.indexOf(f.area) === kids.indexOf(a) + 1; })()"
same "2 px out of the frame" "$(ev "(() => { const r = ${W}.frame.area; return [r.x, r.y, r.width, r.height].join(','); })()")" \
    "$(ev "(() => { const f = ${P}.get_frame_rect(); return [f.x - 2, f.y - 2, f.width + 4, f.height + 4].join(','); })()")"
sleep 2 # the first paint of a new size comes a frame or two after it, slowly in software
paints="$(ev "${W}.frame.paints")"
ev "${P}.move_frame(true, 100, 120); 'm'" >/dev/null
sleep 1
same "a move moves it without painting it again" "$(ev "${W}.frame.paints")" "${paints}"
ev "${P}.maximize(); 'm'" >/dev/null
check "none round a maximized window" wait_for 5 is_true "!${W}.frame.area.visible"
ev "${P}.unmaximize(); 'u'" >/dev/null
tiling
check "none in the tiling" wait_for 5 is_true "!${W}.running('frame')"
classic
ame_ desktop windows frame off >/dev/null

# --- 57.7 GNOME's window settings -----------------------------------------------------------------------------------
ame_ desktop window-settings focus-follows-pointer on >/dev/null
ame_ desktop window-settings middle-click lower >/dev/null
ame_ desktop window-settings drag-key alt >/dev/null
ame_ desktop window-settings resize-right on >/dev/null
same "focus follows the pointer, middle click lowers, Alt drags, right button resizes" \
    "$(gs get org.gnome.desktop.wm.preferences focus-mode) $(gs get org.gnome.desktop.wm.preferences action-middle-click-titlebar) $(gs get org.gnome.desktop.wm.preferences mouse-button-modifier) $(gs get org.gnome.desktop.wm.preferences resize-with-right-button)" \
    "'sloppy' 'lower' '<Alt>' true"
check "the list says so" test "$(ame_ desktop window-settings | grep -c -e 'focus-follows-pointer on' -e 'middle-click lower' -e 'drag-key alt' -e 'resize-right on')" == 4
for wrong in "drag-key ctrl" "double-click explode" "focus-follows-pointer maybe"; do
    # shellcheck disable=SC2086
    check "ame desktop window-settings ${wrong} is refused" refused ${wrong}
done
tiling
check "attach-dialogs is refused while the tiling keeps dialogs apart" refused attach-dialogs on
classic
ame_ desktop window-settings attach-dialogs on >/dev/null
same "in the classic layout it is set" "$(gs get org.gnome.mutter attach-modal-dialogs)" true
for key in focus-mode action-middle-click-titlebar mouse-button-modifier resize-with-right-button; do
    gs reset org.gnome.desktop.wm.preferences "${key}"
done
gs reset org.gnome.mutter attach-modal-dialogs

# --- 57.8 Size for recording ----------------------------------------------------------------------------------------
sized="$(ev "(() => { const w = ${P}; Main.wm._windowMenuManager.showWindowMenuForWindow(w, imports.gi.Meta.WindowMenuType.WM, w.get_frame_rect());
    const m = Main.wm._windowMenuManager._manager.activeMenu; const sub = m._getMenuItems().find(i => i.label?.text === 'Size for recording');
    if (!sub) return 'none'; const it = sub.menu._getMenuItems().find(i => i.label?.text === '1280 × 720'); if (!it) return 'no size';
    it.activate(imports.gi.Clutter.get_current_event()); return 'chosen'; })()")"
same "the window menu has Size for recording, 1280 × 720" "${sized}" chosen
sleep 1
scale="$(ev "global.display.get_monitor_scale(${P}.get_monitor())")"
if [[ "$(ev "(() => { const a = ${AREA}; return String(a.width >= 1280 / ${scale} && a.height >= 720 / ${scale}); })()")" == true ]]; then
    same "the window is 1280 × 720 of the screen's pixels, centred in the work area" "$(rect_of "${P}")" \
        "$(ev "(() => { const a = ${AREA}; const s = ${scale}; const w = Math.round(1280 / s), h = Math.round(720 / s); return [a.x + Math.round((a.width - w) / 2), a.y + Math.round((a.height - h) / 2), w, h].join(','); })()")"
else
    # The lab's 1280 × 800 screen has a work area under 720 high: the size is offered greyed out and does nothing
    skip "the window is 1280 × 720 centred: the lab's work area is too small for it"
fi

# --- 57.9 Reopen ----------------------------------------------------------------------------------------------------
ev "${P}.change_workspace_by_index(1, false); ${P}.move_resize_frame(false, 210, 160, 700, 450); 'm'" >/dev/null
sleep 1
before="$(rect_of "${P}")"
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); ${P}.delete(global.get_current_time()); 'closed'" >/dev/null
wait_for 10 none_of org.vmlab.Plain57
check "the closed window is kept in memory: its app, workspace and place" is_true "(() => { const c = ${W}.reopen.closed[0]; return c?.app === 'org.vmlab.Plain57.desktop' && c.workspace === 1 && c.place.w === 700; })()"
keys super+alt+t
check "Super+Alt+T opens the app again" wait_for 30 at_least org.vmlab.Plain57 1
sleep 2
same "at its place" "$(rect_of "${P}")" "${before}"
check "on its workspace, which is shown" is_true "${P}.get_workspace().index() === 1 && global.workspace_manager.get_active_workspace_index() === 1"
# places.json is the places module's own record of where each app's window sat (older, and on), not reopen's
check "nothing of it is on disk" bash -c "! grep -rqs --exclude='places.json*' 'Plain57' '${HOME_DIR}/.local/state/amethystora' '${HOME_DIR}/.config/amethystora'"
for i in 1 2 3 4 5 6; do
    open_plain
done
wait_for 60 at_least org.vmlab.Plain57 7
sleep 2
ev "${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain57').forEach(w => w.delete(global.get_current_time())); 'x'" >/dev/null
wait_for 10 none_of org.vmlab.Plain57
same "five back at most" "$(ev "${W}.reopen.closed.length")" 5
keys super+alt+t
wait_for 30 at_least org.vmlab.Plain57 1
same "one taken" "$(ev "${W}.reopen.closed.length")" 4
ame_ desktop windows reopen off >/dev/null
sleep 1
same "reopen off forgets" "$(ev "${W}.reopen.closed.length")" 0
ame_ desktop windows reopen on >/dev/null
sleep 1

# --- 57.10 A lock, and ten disables ---------------------------------------------------------------------------------
keys super+alt+return
wait_for 30 is_true "!!${T} && !${T}.minimized"
tid="$(ev "${T}.get_id()")"
loginctl lock-session "${SESSION}"
sleep 5
loginctl unlock-session "${SESSION}"
sleep 5
check "after a screen lock the drop-down terminal is the same window, still on top" wait_for 10 is_true "${T}?.get_id() === ${tid} && ${T}.is_above()"
for _ in $(seq 10); do
    ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
    ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
done
sleep 3
ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
sleep 1
left="$(ev "(() => { const walk = (a, acc) => { for (const c of a.get_children()) { const s = c.get_style_class_name?.() || ''; if (/amethystora-windows/.test(s)) acc.push(s); walk(c, acc); } return acc; }; return walk(Main.layoutManager.uiGroup, []).length; })()")"
same "ten disables leave no actor of the windows module behind" "${left}" 0
check "nor the window menu's hook, nor a key" is_true "!Object.hasOwn(Main.wm._windowMenuManager, 'showWindowMenuForWindow') && ['drop-down-key', 'keep-on-top-key', 'reopen-key'].every(k => (Main.wm._allowedKeybindings[k] ?? 0) === 0)"
ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
sleep 3

# --- 57.11 Control --------------------------------------------------------------------------------------------------
LIST="$(in_session /usr/libexec/amethystora-control-list --json)"
for id in windows-drop-down windows-scratchpad windows-keep-on-top windows-see-through windows-see-through-terminal \
    windows-frame windows-recording-sizes windows-reopen window-settings-focus window-settings-centre \
    window-settings-double-click window-settings-middle-click window-settings-right-click window-settings-drag-key \
    window-settings-resize-right window-settings-attach-dialogs; do
    check "Control lists ${id}" jq -e --arg id "${id}" '.items[] | select(.id == $id)' <<<"${LIST}" >/dev/null
done
check "every one of their commands is an ame desktop command" test -z "$(jq -r '.items[] | select(.id | test("^windows-(drop|scratch|keep-on|see|frame|recording|reopen)|^window-settings")) | (.choices[]?.command, .extra.command?) | select(. != null) | .[0:3] | join(" ")' <<<"${LIST}" | grep -vE '^ame desktop (windows|window-settings)$')"

# --- 57.12 The terminal's lights -----------------------------------------------------------------------------------
L="${W}.lights"
K="$(byclass org.vmlab.Kitty57)"
S="${L}.strip(${K})"
user_bus systemd-run --user --collect --quiet -- kitty --class org.vmlab.Kitty57 -o confirm_os_window_close=0
check "a kitty window gets the lights" wait_for 30 is_true "!!${K} && ${L}.windows.includes(${K})"
sleep 2
same "three of them, in GNOME's button order" "$(ev "${S}.box.get_children().map(b => b.style_class.split('-').at(-1)).join(',')")" \
    "$(ev "${L}.order.join(',')")"
check "on the right end of kitty's title bar, as high as it" is_true "(() => { const w = ${K}, f = w.get_frame_rect(), b = w.get_buffer_rect(), box = ${S}.box; return box.visible && box.height === b.y - f.y && box.y === f.y - b.y && box.x + box.width === f.x + f.width - b.x; })()"
check "kitty's own buttons are clipped off its title bar" is_true "${S}.clipped?.has_clip && ${S}.clipped.get_clip()[2] === ${S}.box.x"
check "the drop-down terminal, which has no title bar, gets none" is_true "!${T} || !${L}.windows.includes(${T})"
ev "${K}.activate(global.get_current_time()); 'x'" >/dev/null
check "focused, they are not grey" wait_for 5 is_true "!${S}.box.has_style_class_name('backdrop')"
light() { ev "(() => { const box = ${S}.box, b = box.get_children().find(c => c.has_style_class_name('amethystora-windows-light-$1')); const [x, y] = b.get_transformed_position(); return ((x + b.width / 2) / global.screen_width).toFixed(5) + ',' + ((y + b.height / 2) / global.screen_height).toFixed(5); })()"; }
classic
ev "${K}.move_frame(true, 200, 150); ${K}.activate(global.get_current_time()); 'x'" >/dev/null
sleep 1
keys "click:$(light maximize)"
check "a click on the green light maximizes kitty" wait_for 5 is_true "${K}.maximized_horizontally && ${K}.maximized_vertically"
sleep 1
keys "click:$(light maximize)"
check "and again puts it back" wait_for 5 is_true "!${K}.maximized_horizontally"
sleep 1
keys "click:$(light minimize)"
check "a click on the amber light minimizes it" wait_for 5 is_true "${K}.minimized"
ev "${K}.unminimize(); ${K}.activate(global.get_current_time()); 'x'" >/dev/null
sleep 1
keys "click:$(light close)"
check "a click on the red light closes it" wait_for 10 none_of org.vmlab.Kitty57
same "and its strip goes with it" "$(ev "${L}.windows.length")" 0
[[ "${was_tiling}" == true ]] && tiling

# Back as it was
ev "${WIN}.filter(w => ['org.vmlab.Plain57', 'org.amethystora.DropDown'].includes(w.get_wm_class())).forEach(w => w.delete(global.get_current_time())); 'x'" >/dev/null
[[ "${was_tiling}" == true ]] && tiling
ev "global.force_animations = false; 'unforced'" >/dev/null
rm -f "${HOME_DIR}/.local/share/applications/org.vmlab.Plain57.desktop" /tmp/vmlab-plain57.js
finish
