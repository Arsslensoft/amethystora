#!/usr/bin/bash
# vmlab-vm: r4-shelf
# Round 4, group shelf (plans/beyond/spec-r4-shelf.md): the shelf and desks, in the test account's session, asked of
# GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image). Drags are
# made with the shell's virtual pointer (test code only) from a small GTK window of the lab's that offers a file list,
# text or a picture and writes down what is dropped on it; the shelf's own window is read through AT-SPI (atspi.py):
#   - the shelf: its key and recipe, off costing nothing; the glow while a drag is on its way; the first drop handed to
#     the Shelf app on its standard input; the window sticky, above, at its place, the keyboard given back, out of the
#     dock and the tiling, and a look-alike window of another program left alone; drops straight onto it (text, a
#     picture), the items' names; dragging out as a copy, Ctrl keeping the item, onto a dock stack a copy too; a held
#     file deleted greys its item; Remove from an item's menu; Clear ending the program; the shake's card and the row; a
#     text dropped through the shell never on disk, on a command line or in the journal; the light theme; ten cycles;
#   - desks: their keys, off costing nothing; the first arrival's card, Not now kept per desk and `ask`, Set up;
#     desks.json and its copy; an unlock at the same desk asking nothing; learning and its Not now; a known desk applied
#     with a toast, Undo, leaving put back (to a desk that sets nothing); sound, Focus and the power mode, a Focus hold
#     kept; the recipe's rename and forget, Control; a damaged file read from its copy, both damaged; ten cycles with a
#     card and a toast up; the setup file.
# A second real screen is not plugged in here: the desks a test needs are stood in for through the module's _readDesk.
# vmlab-paths: @ui/modules/pill/drop.js @ui/extension.js @ui/*shelf* @ui/*desks* @ui/*dock/drops* @ui/*dock/dash* @ui/*pointer* @sf/*themed/shell-shelf* @sf/*themed/shell-desks* @sf/usr/bin/amethystora-shelf @sf/usr/lib/amethystora-shelf/* @sf/usr/share/applications/amethystora-shelf.desktop apps/* @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$0")"
SHELF_SCHEMA=org.gnome.shell.extensions.amethystora-ui.shelf
DESKS_SCHEMA=org.gnome.shell.extensions.amethystora-ui.desks
DOCK_SCHEMA=org.gnome.shell.extensions.amethystora-ui.dock
FOCUS_SCHEMA=org.gnome.shell.extensions.amethystora-ui.focus
LAB="${HOME_DIR}/.local/share/vmlab-shelf"
DESKS_FILE="${HOME_DIR}/.config/amethystora/desks.json"
# The text dropped: in a file the lab's window reads, never on a command line of its own
MARK="SHELFMARK-$$-$RANDOM"
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
SH="${UI}.get('shelf')"
DK="${UI}.get('desks')"
# A part of each module's state (a jq filter of it)
shelf() { ev "JSON.stringify(${SH}.state())" | jq -r "$1"; }
desks() { ev "JSON.stringify(${DK}.state())" | jq -r "$1"; }
# The Shelf app's window as a person reads it
shelf_has() { user_bus python3 "${HERE}/atspi.py" amethystora-shelf has "$1" >/dev/null 2>&1; }
shelf_lacks() { ! shelf_has "$1"; }
# A drag with the shell's virtual pointer: from, to, a hold at the end before the button goes up, and with SHAKE a shake
# at the end; returns at once, the drag takes about HOLD + 0.7 s. BUTTON 3 is a right click instead.
VP="const C = imports.gi.Clutter, G = imports.gi.GLib;
globalThis._vmlabVp ??= global.stage.context.get_backend().get_default_seat().create_virtual_device(C.InputDeviceType.POINTER_DEVICE);
globalThis._vmlabVk ??= global.stage.context.get_backend().get_default_seat().create_virtual_device(C.InputDeviceType.KEYBOARD_DEVICE);
const move = (x, y) => _vmlabVp.notify_absolute_motion(G.get_monotonic_time(), x, y);"
drag() {
    local fx="$1" fy="$2" tx="$3" ty="$4" hold="${5:-600}" shake="${6:-0}"
    ev "${VP} const from = [${fx}, ${fy}], to = [${tx}, ${ty}];
move(...from);
G.timeout_add(0, 100, () => {
  _vmlabVp.notify_button(G.get_monotonic_time(), 1, C.ButtonState.PRESSED);
  let i = 0;
  G.timeout_add(0, 16, () => {
    i++;
    if (i <= 30) { move(from[0] + (to[0] - from[0]) * i / 30, from[1] + (to[1] - from[1]) * i / 30); return true; }
    if (${shake} && i <= 54) { move(to[0] + ((i % 2) ? 160 : 0) - 80, to[1]); return true; }
    if (${shake} && i === 55) { move(...to); return true; }
    G.timeout_add(0, ${hold}, () => { _vmlabVp.notify_button(G.get_monotonic_time(), 1, C.ButtonState.RELEASED); return false; });
    return false;
  });
  return false;
}); 'dragging'" >/dev/null
}
click() {
    ev "${VP} const t = G.get_monotonic_time(); move($1, $2); _vmlabVp.notify_button(t + 1000, ${3:-1}, C.ButtonState.PRESSED);
_vmlabVp.notify_button(t + 60000, ${3:-1}, C.ButtonState.RELEASED); 1" >/dev/null
}
ctrl() { ev "${VP} _vmlabVk.notify_keyval(G.get_monotonic_time(), C.KEY_Control_L, C.KeyState.$1); 'ctrl'" >/dev/null; }
# The frame of a window, by app id, as "x y width height"
WIN="global.get_window_actors().map(a => a.meta_window)"
frame() {
    ev "const f = ${WIN}.find(w => w.get_gtk_application_id() === '$1')?.get_frame_rect(); f ? [f.x, f.y, f.width, f.height].join(' ') : ''"
}
has_frame() { [[ -n "$(frame "$1")" ]]; }
front() { ev "Main.activateWindow(${WIN}.find(w => w.get_gtk_application_id() === '$1')); 1" >/dev/null; sleep 1; }
# The lab's window of each kind, started under a description of its own (a unit's description defaults to its command)
lab_window() {
    user_bus systemd-run --user --unit "vmlab-drag$1" --description "vmlab shelf $1" --collect \
        gjs -m "${LAB}/drag.js" "$1" "${LAB}" >/dev/null
    wait_for 30 has_frame "org.vmlab.Drag$1"
}
shelf_windows() { shelf '.windows | length'; }
one_shelf() { [[ "$(shelf_windows)" == 1 ]]; }
shelf_running() { pgrep -u "${TEST_USER}" -f '^/usr/lib/amethystora-shelf/amethystora-shelf' >/dev/null; }
no_shelf_running() { ! shelf_running; }
received() { [[ -s "${LAB}/received.txt" ]]; }
win_box() { shelf '.windows[0] | "\(.x) \(.y) \(.width) \(.height)"'; }
height_now() { shelf '.windows[0].height'; }
# Ten disables and enables, each waited for: GNOME turns an extension off and on asynchronously (1 on; 2, or 6, off)
cycle10() {
    ev "globalThis._vmlabCycled = false; (async () => {
        const E = Main.extensionManager;
        const until = async on => { for (let n = 0; n < 50; n++) { const s = E.lookup('${UUID}').state; if (on ? s === 1 : s === 2 || s === 6) return; await new Promise(r => setTimeout(r, 100)); } };
        for (let i = 0; i < 10; i++) { E.disableExtension('${UUID}'); await until(false); E.enableExtension('${UUID}'); await until(true); }
        globalThis._vmlabCycled = true;
    })().catch(e => { globalThis._vmlabCycled = 'ERR ' + e.message; }); 'cycling'" >/dev/null
    cycled() { [[ "$(ev "String(globalThis._vmlabCycled)")" != false ]]; }
    wait_for 60 cycled
}
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "${HERE}/10-account.sh" >/dev/null
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
# The lab's window: drags a file list (as Files offers it), text (read from the file "mark") or a picture, and writes
# down what is dropped on it; "fake" is only a window that says it is the shelf
cat >"${LAB}/drag.js" <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
import Gdk from 'gi://Gdk?version=4.0';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
const [kind, dir] = ARGV;
const app = new Gtk.Application({application_id: kind === 'fake' ? 'org.amethystora.Shelf' : `org.vmlab.Drag${kind}`});
app.connect('activate', () => {
    const win = new Gtk.ApplicationWindow({application: app, title: `Drag ${kind}`, default_width: 500, default_height: 600});
    const box = new Gtk.Box({orientation: Gtk.Orientation.VERTICAL, spacing: 20});
    const from = new Gtk.Label({label: 'DRAG', vexpand: true, hexpand: true});
    const source = new Gtk.DragSource({actions: Gdk.DragAction.COPY});
    source.connect('prepare', () => {
        if (kind === 'text')
            return Gdk.ContentProvider.new_for_value(new TextDecoder().decode(GLib.file_get_contents(`${dir}/mark`)[1]).trim());
        if (kind === 'picture')
            return Gdk.ContentProvider.new_for_bytes('image/png', new GLib.Bytes(GLib.file_get_contents(`${dir}/pic.png`)[1]));
        const list = `file://${dir}/beach-2026.jpg\r\nfile://${dir}/invoice-october.pdf\r\n`;
        return Gdk.ContentProvider.new_for_bytes('text/uri-list', new GLib.Bytes(new TextEncoder().encode(list)));
    });
    from.add_controller(source);
    const to = new Gtk.Label({label: 'DROP', vexpand: true});
    const target = Gtk.DropTarget.new(GObject.TYPE_NONE, Gdk.DragAction.COPY);
    target.set_gtypes([Gdk.FileList.$gtype, Gdk.Texture.$gtype, GObject.TYPE_STRING]);
    target.connect('drop', (t, value) => {
        let text;
        if (value instanceof Gdk.Texture)
            text = `picture:${value.get_width()}x${value.get_height()}`;
        else if (typeof value === 'string')
            text = 'text';
        else
            text = `files:${value.get_files().map(f => f.get_path()).join(',')}`;
        GLib.file_set_contents(`${dir}/received.txt`, text);
        return true;
    });
    to.add_controller(target);
    box.append(from);
    box.append(to);
    win.set_child(box);
    win.present();
});
app.run([]);
EOF
echo beach >"${LAB}/beach-2026.jpg"
echo invoice >"${LAB}/invoice-october.pdf"
echo "Flight LH 1173 leaves at 9:40 ${MARK}" >"${LAB}/mark"
python3 - "${LAB}/pic.png" <<'EOF'
import struct, sys, zlib
w, h = 120, 80
rows = b''.join(b'\0' + bytes([240, 160 - y, 90 + y]) * w for y in range(h))
chunk = lambda t, d: struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
open(sys.argv[1], 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
    + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
THEME_BEFORE="$(cat "${HOME_DIR}/.config/amethystora/current/theme.name" 2>/dev/null)"
for key in shelf; do gs reset "${SHELF_SCHEMA}" "${key}"; done
for key in desks desks-learn; do gs reset "${DESKS_SCHEMA}" "${key}"; done
gs reset "${DOCK_SCHEMA}" edge
rm -f "${DESKS_FILE}" "${DESKS_FILE}.bak"
loginctl unlock-sessions
systemctl restart gdm
ready() { [[ "$(ev "typeof ${SH}?.state + typeof ${DK}?.state")" == functionfunction ]]; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "the shelf and desks modules did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
cleanup() {
    gs reset "${SHELF_SCHEMA}" shelf
    for key in desks desks-learn; do gs reset "${DESKS_SCHEMA}" "${key}"; done
    gs reset "${DOCK_SCHEMA}" edge
    gs reset "${FOCUS_SCHEMA}" profile
    gs reset "${FOCUS_SCHEMA}" work-power
    ev "${DK} && (${DK}._readDesk = null); 'ok'" >/dev/null 2>&1
    user_bus systemctl --user stop vmlab-dragfile vmlab-dragtext vmlab-dragpicture vmlab-dragfake >/dev/null 2>&1
    pkill -u "${TEST_USER}" -f '^/usr/lib/amethystora-shelf/amethystora-shelf' 2>/dev/null
    in_session "pactl unload-module module-null-sink" >/dev/null 2>&1
    [[ -n "${THEME_BEFORE}" ]] && in_session "ame desktop theme ${THEME_BEFORE}" >/dev/null 2>&1
    rm -f "${DESKS_FILE}" "${DESKS_FILE}.bak" "${HOME_DIR}/Downloads/beach-2026.jpg"
}
trap cleanup EXIT

# ===== The shelf =======================================================================================================

# --- 1-2: the key and the recipe; off costs nothing ----------------------------------------------------------------------
same "S1 the shelf is off for a new account" "$(gs get "${SHELF_SCHEMA}" shelf)" "'off'"
same "S1 ame desktop shelf says so" "$(in_session "ame desktop shelf")" "The shelf is off."
same "S1 a value it does not know exits 64" "$(in_session 'ame desktop shelf dock >/dev/null 2>&1; echo $?')" 64
same "S2 off, the shelf connects nothing" "$(shelf .connected)" false

# --- 3-5: files put down at the edge, through the shell --------------------------------------------------------------------
in_session "ame desktop shelf edge" >/dev/null
same "S3 ame desktop shelf edge turns it on" "$(shelf .connected)" true
lab_window file
read -r fx fy fw fh <<<"$(frame org.vmlab.Dragfile)"
read -r mx my mw mh <<<"$(ev "const m = Main.layoutManager.primaryMonitor; [m.x, m.y, m.width, m.height].join(' ')")"
read -r ax ay aw ah <<<"$(ev "const a = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex); [a.x, a.y, a.width, a.height].join(' ')")"
front org.vmlab.Dragfile
drag $((fx + fw / 2)) $((fy + fh / 4)) $((mx + mw - 10)) $((my + mh / 2)) 2500
sleep 1.6
glow="$(shelf .glow)"
same "S3 the glow is 26 px wide at the screen's right edge" "$(jq -r '"\(.width) \(.x + .width)"' <<<"${glow}")" "26 $((mx + mw))"
same "S3 the glow spans 120/560 to 440/560 of the screen's height" \
    "$(jq -r '"\(.y | floor) \(.height | floor)"' <<<"${glow}")" "$((my + (mh * 120 + 280) / 560)) $((mh - 2 * ((mh * 120 + 280) / 560)))"
same "S3 the drag over the glow is read once" "$(shelf .reading)" true
check "S4 the drop opened the shelf" wait_for 20 one_shelf
# The module takes the window when it is made and places it once it has a size: read it placed
shelf_placed() { [[ "$(shelf '.windows[0] | .width > 0 and .above')" == true ]]; }
wait_for 10 shelf_placed
win="$(shelf '.windows[0]')"
same "S4 the shelf is on every workspace and above" "$(jq -r '"\(.sticky) \(.above)"' <<<"${win}")" "true true"
same "S4 the shelf is 10 px in from the work area's edge" "$(jq -r '.x + .width' <<<"${win}")" "$((ax + aw - 10))"
centre="$(jq -r '.y + .height / 2 | floor' <<<"${win}")"
check "S4 the shelf is centred on the work area's height" test $((centre - (ay + ah / 2))) -ge -1 -a $((centre - (ay + ah / 2))) -le 1
same "S4 the keyboard is given back" "$(ev "global.display.focus_window?.get_gtk_application_id()")" org.vmlab.Dragfile
check "S4 the items are named after the files" wait_for 10 shelf_has beach-2026.jpg
check "S4 ...both of them" shelf_has invoice-october.pdf
SHELF_WINDOW="${WIN}.find(w => ${SH}.owns(w))"
same "S5 the dock lists no shelf" "$(ev "JSON.stringify(${UI}.get('dock').state().docks.flatMap(d => d.items.map(i => i.id))
    .filter(id => /shelf/i.test(id ?? '')))")" "[]"
same "S5 the tiling has not taken the shelf" "$(ev "${UI}.get('tiling').engine?.isTiled?.(${SHELF_WINDOW}) ?? false")" false

# --- 6: text and a picture straight onto the shelf ---------------------------------------------------------------------------
lab_window text
lab_window picture
for kind in text picture; do
    read -r tx ty tw th <<<"$(frame "org.vmlab.Drag${kind}")"
    front "org.vmlab.Drag${kind}"
    read -r wx wy ww wh <<<"$(win_box)"
    drag $((tx + tw / 2)) $((ty + th / 4)) $((wx + ww / 2)) $((wy + wh / 2)) 600
    sleep 3
done
check "S6 the text is named by its first words, in quotes" wait_for 10 shelf_has "“Flight LH 1173…”"
check "S6 the picture is named Picture" shelf_has Picture

# --- 7: dragging out is a copy; Ctrl keeps the item; a dock stack copies too --------------------------------------------------
read -r fx fy fw fh <<<"$(frame org.vmlab.Dragfile)"
front org.vmlab.Dragfile
read -r wx wy ww wh <<<"$(win_box)"
rm -f "${LAB}/received.txt"
# The first item (the beach picture's file) onto the lab window's drop half
drag $((wx + ww / 2)) $((wy + 10 + 22)) $((fx + fw / 2)) $((fy + fh * 3 / 4)) 600
wait_for 10 received
same "S7 the app was given the file" "$(cat "${LAB}/received.txt" 2>/dev/null)" "files:${LAB}/beach-2026.jpg"
check "S7 the file is still where it was" test -s "${LAB}/beach-2026.jpg"
shrunk() { (($(height_now) < wh)); }
check "S7 the item left the shelf" wait_for 10 shrunk
read -r wx wy ww wh <<<"$(win_box)"
rm -f "${LAB}/received.txt"
ctrl PRESSED
drag $((wx + ww / 2)) $((wy + 10 + 22)) $((fx + fw / 2)) $((fy + fh * 3 / 4)) 600
wait_for 10 received
ctrl RELEASED
sleep 1
same "S7 with Ctrl the item stays" "$(height_now)" "${wh}"
# The beach file again, then onto the dock's Downloads stack: Files copies it there, and it stays where it was
front org.vmlab.Dragfile
read -r wx wy ww wh <<<"$(win_box)"
drag $((fx + fw / 2)) $((fy + fh / 4)) $((wx + ww / 2)) $((wy + wh / 2)) 600
sleep 3
stack="$(ev "JSON.stringify(${UI}.get('dock').state().docks[0].items.find(i => i.kind === 'stack')?.rest ?? null)")"
if [[ "${stack}" != null ]]; then
    # The fourth item (invoice, text, picture, beach): 1 px of border, 10 of padding, 71 for each item above, 22 into it
    read -r wx wy ww wh <<<"$(win_box)"
    drag $((wx + ww / 2)) $((wy + 11 + 3 * 71 + 22)) "$(jq -r '.x + .width / 2 | floor' <<<"${stack}")" \
        "$(jq -r '.y + .height / 2 | floor' <<<"${stack}")" 900
    copied() { [[ -s "${HOME_DIR}/Downloads/beach-2026.jpg" ]]; }
    check "S7 a dock stack takes a copy of a shelf's file" wait_for 15 copied
    check "S7 ...and the file is still where it was" test -s "${LAB}/beach-2026.jpg"
else
    skip "S7 the dock has no stack to drop on"
fi

# --- 8: a held file deleted greys its item -----------------------------------------------------------------------------------
mv "${LAB}/invoice-october.pdf" "${LAB}/invoice.moved"
check "S8 a held file moved away greys its item" wait_for 5 shelf_has "invoice-october.pdf, moved or deleted"
mv "${LAB}/invoice.moved" "${LAB}/invoice-october.pdf"
check "S8 ...and back, it is itself again" wait_for 5 shelf_lacks "moved or deleted"

# --- M3: Remove from an item's menu, with other items held -------------------------------------------------------------------
read -r wx wy ww wh <<<"$(win_box)"
click $((wx + ww / 2)) $((wy + 10 + 22))
sleep 1
if user_bus python3 "${HERE}/atspi.py" amethystora-shelf has Remove >/dev/null 2>&1; then
    not_ok "S9 a left click on an item opens no menu"
else
    ok "S9 a left click on an item opens no menu"
fi
click $((wx + ww / 2)) $((wy + 10 + 22)) 3
sleep 1
user_bus python3 "${HERE}/atspi.py" amethystora-shelf click Remove >/dev/null 2>&1
shrunk() { (($(height_now) < wh)); }
check "S9 Remove from the item's menu takes the item away" wait_for 10 shrunk
check "S9 ...and the shelf keeps the rest" shelf_running
same "S9 ...with no GTK critical" \
    "$(journalctl -b "_UID=$(id -u "${TEST_USER}")" --since "${since}" --no-pager -o cat | grep -c 'amethystora-shelf.*CRITICAL')" 0

# --- 9: Clear ends the program ---------------------------------------------------------------------------------------------
read -r wx wy ww wh <<<"$(win_box)"
click $((wx + ww / 2)) $((wy + wh - 14))
check "S9 Clear closes the shelf and ends the program" wait_for 10 no_shelf_running

# --- 11: text dropped through the shell stays in memory --------------------------------------------------------------------------
read -r tx ty tw th <<<"$(frame org.vmlab.Dragtext)"
front org.vmlab.Dragtext
drag $((tx + tw / 2)) $((ty + th / 4)) $((mx + mw - 10)) $((my + mh / 2)) 1500
check "S11 a text dropped on the glow opens the shelf" wait_for 20 one_shelf
check "S11 ...holding the text" wait_for 10 shelf_has "Flight LH 1173"
leaks="$(grep -rlsF "${MARK}" "${HOME_DIR}" /tmp /var/tmp "/run/user/$(id -u "${TEST_USER}")" /var/lib/systemd/coredump 2>/dev/null |
    grep -v "^${LAB}/" | head -n 5)"
same "S11 the dropped text is in no file" "${leaks}" ""
# The mark reaches grep on a descriptor: on its command line, grep would find itself
same "S11 the dropped text is on no command line" \
    "$(grep -lsaF -f <(printf '%s\n' "${MARK}") /proc/[0-9]*/cmdline 2>/dev/null | head -n 3)" ""
same "S11 the dropped text is not in the journal" "$(journalctl -b --since "${since}" --no-pager -o cat | grep -cF "${MARK}")" 0
pkill -u "${TEST_USER}" -f '^/usr/lib/amethystora-shelf/amethystora-shelf' 2>/dev/null
wait_for 10 no_shelf_running

# --- M2: a window of another program that says it is the shelf is left alone -----------------------------------------------
lab_window fake
fake="$(ev "const w = ${WIN}.find(w => w.get_gtk_application_id() === 'org.amethystora.Shelf');
JSON.stringify({sticky: w.is_on_all_workspaces(), above: w.is_above(), owned: ${SH}.owns(w)})")"
same "S5 a look-alike window is not kept above, on every workspace or out of the dock" "${fake}" \
    '{"sticky":false,"above":false,"owned":false}'
user_bus systemctl --user stop vmlab-dragfake >/dev/null 2>&1

# --- 10: the shake ---------------------------------------------------------------------------------------------------------
in_session "ame desktop shelf shake" >/dev/null
read -r fx fy fw fh <<<"$(frame org.vmlab.Dragfile)"
front org.vmlab.Dragfile
cx=$((mx + mw * 3 / 4)) cy=$((my + mh / 2))
drag $((fx + fw / 2)) $((fy + fh / 4)) "${cx}" "${cy}" 2500 1
sleep 1.8
check "S10 a shake while dragging opens the card under the pointer" test "$(shelf .card)" != null
check "S10 the drop on the card opens the shelf" wait_for 20 one_shelf
# The window is taken in as it is made, before its first frame: placed once it has a size
on_drop() { [[ "$(shelf '.windows[0] | "\(.x + .width / 2 | floor) \(.y + .height / 2 | floor)"')" == "${cx} ${cy}" ]]; }
wait_for 10 on_drop
win="$(shelf '.windows[0]')"
same "S10 the shelf opens as a row" "$(jq -r .layout <<<"${win}")" row
same "S10 the row is centred on the drop" "$(jq -r '"\(.x + .width / 2 | floor) \(.y + .height / 2 | floor)"' <<<"${win}")" "${cx} ${cy}"

# --- 13: the light theme ---------------------------------------------------------------------------------------------------
in_session "ame desktop theme amethystora-light" >/dev/null 2>&1
light_rendered() { grep -q "amethystora-shelf-glow" "${HOME_DIR}/.config/amethystora/current/theme/shell-shelf.css"; }
check "S13 the light theme renders the shelf's colours" wait_for 20 light_rendered
check "S13 ...with no placeholder left" bash -c "! grep -q '{{' '${HOME_DIR}/.config/amethystora/current/theme/shell-shelf.css'"

# --- 12: ten cycles --------------------------------------------------------------------------------------------------------
cycle10
sleep 2
same "S12 after ten cycles the shelf is still sticky and above" "$(shelf '.windows[0] | "\(.sticky) \(.above)"')" "true true"
same "S12 ...and where it was" "$(shelf '.windows[0] | "\(.x + .width / 2 | floor) \(.y + .height / 2 | floor)"')" "${cx} ${cy}"
same "S12 nothing of the shelf is left in the shell" \
    "$(ev "Main.layoutManager.uiGroup.get_children().filter(a => /amethystora-shelf/.test(a.style_class ?? '')).length")" 0
pkill -u "${TEST_USER}" -f '^/usr/lib/amethystora-shelf/amethystora-shelf' 2>/dev/null
in_session "ame desktop shelf off" >/dev/null
same "S2 off again, the shelf connects nothing" "$(shelf .connected)" false

# ===== Desks ===========================================================================================================

same "D1 desks are off for a new account" "$(gs get "${DESKS_SCHEMA}" desks) $(gs get "${DESKS_SCHEMA}" desks-learn)" "false true"
same "D2 off, desks hold nothing" "$(desks '"\(.on) \(.timer) \(.fileMonitor) \(.learning)"')" "false false false false"
# Desks stood in for: two screens with their serial numbers, the laptop alone, and a set of screens nobody sets up
ev "globalThis._vmlabA = () => ({id: 'aaaaaaaaaaaa', base: 'aaaaaaaaaaaa', screens: [{vendor: 'DEL', product: 'DELL U2720Q', serial: 'X1', connector: 'DP-1', name: 'Dell 27'}, {vendor: 'DEL', product: 'DELL U2720Q', serial: 'X2', connector: 'DP-2', name: 'Dell 27'}], thunderbolt: 0, builtin: true, lidShut: true});
globalThis._vmlabL = () => ({id: 'bbbbbbbbbbbb', base: 'bbbbbbbbbbbb', screens: [], thunderbolt: 0, builtin: true, lidShut: false});
globalThis._vmlabU = () => ({id: 'cccccccccccc', base: 'cccccccccccc', screens: [{vendor: 'LG', product: 'LG TV', serial: '', connector: 'HDMI-1', name: 'LG TV'}], thunderbolt: 0, builtin: true, lidShut: false}); 'ok'" >/dev/null
at() { ev "${DK}._readDesk = _vmlab$1; ${DK}._queue('change'); 'queued'" >/dev/null; sleep 3; }
unlock() { ev "${DK}._stop({turnedOff: false}); ${DK}._start(); 'unlocked'" >/dev/null; sleep 3; }
card_up() { [[ "$(desks .card)" == true ]]; }
no_card() { [[ "$(desks .card)" == false ]]; }
in_session "ame desktop desks on" >/dev/null
ev "${DK}._readDesk = _vmlabA; 'ok'" >/dev/null
check "D3 the first arrival shows the card" wait_for 10 card_up
ev "${DK}.card.buttons[1].emit('clicked', 1); 'not now'" >/dev/null
same "D3 Not now keeps no desk" "$(jq -c '.desks | length' "${DESKS_FILE}")" 0
same "D3 ...but remembers the answer" "$(jq -c '.declined' "${DESKS_FILE}")" '["aaaaaaaaaaaa"]'
at L
at A
check "D3 the next arrival at that desk does not ask again" no_card
in_session "ame desktop desks ask" >/dev/null
at L
at A
check "D3 after ame desktop desks ask, it asks again" wait_for 10 card_up
ev "${DK}.card.buttons[0].emit('clicked', 1); 'set up'" >/dev/null
sleep 1
same "D4 Set up keeps the desk with its screens" "$(jq -c '[.desks[] | select(.id == "aaaaaaaaaaaa") | .screens[].serial]' "${DESKS_FILE}")" '["X1","X2"]'
same "D4 Focus off is not recorded as the desk's" "$(jq -c '.desks[] | select(.id == "aaaaaaaaaaaa") | .set | has("focus")' "${DESKS_FILE}")" false
same "D4 the copy beside it says the same" "$(cmp -s "${DESKS_FILE}" "${DESKS_FILE}.bak" && echo same)" same
same "D4 the file is the account's alone" "$(stat -c %a "${DESKS_FILE}")" 600
# M6: a card left unanswered does not come back at an unlock at the same desk
at U
check "D3 a new set of screens asks" wait_for 10 card_up
ev "${DK}._closeCard(); 'left unanswered'" >/dev/null
unlock
check "D3 an unlock at the same desk asks nothing" no_card
at L
ev "${DK}.card.buttons[0].emit('clicked', 1); 'set up'" >/dev/null
sleep 6
gs set "${DOCK_SCHEMA}" edge right
learn_asked() { [[ "$(desks .learnQuestion)" == true ]]; }
check "D5 moving the dock at a desk asks to remember it" wait_for 10 learn_asked
ev "${DK}._learnQuestion.actions[1].activate(); 'not now'" >/dev/null
gs set "${DOCK_SCHEMA}" edge left
sleep 1
gs set "${DOCK_SCHEMA}" edge right
sleep 2
same "D5 Not now to the same change is not asked again" "$(desks .learnQuestion)" false
gs set "${DOCK_SCHEMA}" edge left
check "D5 another change is asked about" wait_for 10 learn_asked
ev "${DK}._learnQuestion.actions[0].activate(); 'remembered'" >/dev/null
sleep 1
same "D5 Remember writes it into the desk" "$(jq -r '.desks[] | select(.id == "bbbbbbbbbbbb") | .set.dock.edge' "${DESKS_FILE}")" left
at A
same "D6 arriving at a known desk sets what differs" "$(gs get "${DOCK_SCHEMA}" edge)" "'bottom'"
same "D6 a toast says so" "$(desks .toast)" "Set for 2 screens: the dock"
ev "${DK}.toast.undo.emit('clicked', 1); 'undo'" >/dev/null
sleep 1
same "D6 Undo puts it back" "$(gs get "${DOCK_SCHEMA}" edge)" "'left'"
at L
at A
# Leaving for a set of screens no desk is: only putting back changes the dock
at U
same "D6 leaving puts back what the desk set" "$(gs get "${DOCK_SCHEMA}" edge)" "'left'"
ev "${DK}._closeCard(); 'ok'" >/dev/null
# Sound, Focus and the power mode for desk A, as the desk recorded them
in_session "pactl load-module module-null-sink sink_name=vmlab_desk sink_properties=device.description=VmlabDesk" >/dev/null
sleep 2
SINK_BEFORE="$(in_session "pactl get-default-sink")"
gs set "${FOCUS_SCHEMA}" work-power performance
in_session "jq -c '(.desks[] | select(.id == \"aaaaaaaaaaaa\") | .set) += {\"focus\": \"work\", \"power\": \"power-saver\",
    \"sound\": {\"sink\": \"vmlab_desk\", \"port\": \"\", \"label\": \"VmlabDesk\"}}' '${DESKS_FILE}' >'${DESKS_FILE}.new' &&
    mv '${DESKS_FILE}.new' '${DESKS_FILE}'"
sleep 2
at A
sleep 2
same "D7 the sound output comes with the desk" "$(in_session "pactl get-default-sink")" vmlab_desk
same "D7 Focus comes with the desk" "$(ev "${UI}.get('focus').state().hand")" work
profiles() {
    gdbus call --system --dest org.freedesktop.UPower.PowerProfiles --object-path /org/freedesktop/UPower/PowerProfiles \
        --method org.freedesktop.DBus.Properties.Get org.freedesktop.UPower.PowerProfiles "$1" 2>/dev/null
}
if profiles Profiles | grep -q "'performance'"; then
    same "D8 Work's own power hold is kept over the desk's mode" "$(ev "${UI}.get('focus').state().power.held?.profile ?? 'none'")" performance
    check "D8 ...as the daemon has it" bash -c "$(declare -f profiles); profiles ActiveProfileHolds | grep -q performance"
else
    skip "D8 this machine has no performance mode for Work to hold"
fi
at U
same "D7 leaving puts the sound output back" "$(in_session "pactl get-default-sink")" "${SINK_BEFORE}"
same "D7 leaving ends Focus" "$(ev "${UI}.get('focus').state().hand")" off
same "D8 leaving puts the power mode back" "$(profiles ActiveProfile)" "(<'balanced'>,)"
ev "${DK}._closeCard(); 'ok'" >/dev/null
in_session "ame desktop desks rename aaaaaaaaaaaa Office desk" >/dev/null
same "D9 a name with a right-to-left override is refused" \
    "$(in_session "ame desktop desks rename aaaaaaaaaaaa \$'a‮b' >/dev/null 2>&1; echo \$?")" 64
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "D9 Control lists the desk by its new name" "$(jq -r '.items[] | select(.id == "desk-aaaaaaaaaaaa") | .title' <<<"${control}")" "Office desk"
# The serials as words (Steam's entry says X11)
serials="$(grep -oE '"[^"]*\bX[12]\b[^"]*"' <<<"${control}" | sort -u | paste -sd ' ')"
same "D9 Control shows no serial number" "${serials}" ""
in_session "ame desktop desks forget bbbbbbbbbbbb" >/dev/null
sleep 2
same "D9 forget takes it away, from the copy too" "$(jq -c '[.desks[].id]' "${DESKS_FILE}.bak")" '["aaaaaaaaaaaa"]'
echo garbage >"${DESKS_FILE}"
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "D10 Control reads the copy while the file is damaged" "$(jq -r '[.items[] | select(.id == "desk-aaaaaaaaaaaa")] | length' <<<"${control}")" 1
unlock
same "D10 a damaged file is read from its copy and written again" "$(jq -c '[.desks[].id]' "${DESKS_FILE}")" '["aaaaaaaaaaaa"]'
echo garbage >"${DESKS_FILE}"
echo junk >"${DESKS_FILE}.bak"
unlock
same "D10 both damaged: no desk, and nothing thrown" "$(desks '.desks | length')" 0
# A card and a toast up through ten cycles
at A
check "D11 a card is up" wait_for 10 card_up
ev "${DK}.card.buttons[0].emit('clicked', 1); 'set up'" >/dev/null
gs set "${DOCK_SCHEMA}" edge right
in_session "jq -c '(.desks[0].set.dock) = {\"edge\": \"bottom\", \"screens\": \"every\"}' '${DESKS_FILE}' >'${DESKS_FILE}.new' &&
    mv '${DESKS_FILE}.new' '${DESKS_FILE}'"
sleep 2
at U
at A
toast_up() { [[ "$(desks .toast)" != null ]]; }
check "D11 a toast is up" toast_up
cycle10
sleep 1
# Left over: a card or toast the running module does not hold. The module started again reads this machine's own
# screens (D13), a desk it has not seen, and may ask about it with a card of its own
same "D11 nothing of desks is left in the shell" \
    "$(ev "Main.layoutManager.uiGroup.get_children().filter(a => /amethystora-desks/.test(a.style_class ?? '') && a !== ${DK}.card && a !== ${DK}.toast).map(a => a.style_class).join(', ')")" ""
sleep 3
same "D11 after the cycles desks are on again, no timer left" "$(desks '"\(.on) \(.fileMonitor) \(.timer)"')" "true true false"
# The cycles started the module again without its stand-in: it read this machine's own screens, as Mutter has them
same "D13 the screens of this machine make a desk with an id" "$(desks '.here.id | test("^[0-9a-f]{12}$")')" true
in_session "ame setup save" >/dev/null 2>&1
same "D12 the setup file carries desks on" "$(jq -c '.settings.desks' "${HOME_DIR}/.config/amethystora/setup.json")" '{"desks":true}'
same "D12 and no serial number" "$(grep -cE '\bX[12]\b|serial' "${HOME_DIR}/.config/amethystora/setup.json")" 0
in_session "ame desktop desks off" >/dev/null
same "D2 off again, desks hold nothing" "$(desks '"\(.on) \(.timer) \(.fileMonitor)"')" "false false false"

# --- and nothing threw ---------------------------------------------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: the (shelf|desks) module|Amethystora UI: (shelf|desks)" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of the shelf or desks threw"; else not_ok "the shelf or desks threw" "${errors}"; fi
finish
