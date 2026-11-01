#!/usr/bin/bash
# vmlab-vm: r5-tools
# Rounds 5 and 6's tools group (plans/ui-ideas/spec-tools.md, section 9), in the test account's session, asked of GNOME
# Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - the tools module's keys and place;
#   - the colour picker (Super+Alt+C, through a virtual keyboard): the frozen screen and the loupe, a real click copying
#     the colour under it as hex, Shift as rgb() and Ctrl as hsl(), Esc nothing, the colours in the search's colour:,
#     and not while the whole screen is shared (sharecast.py);
#   - the microphone (Super+Alt+M) on a null sink's virtual source: muted, the mark, a source chosen next muted too, the
#     privacy list's Mute;
#   - What is using this?: the helper naming the account's own process and never another account's, quit checking the
#     process, the dialog over D-Bus and its Quit, Files' entry written by the hook;
#   - Convert in Files: the switch writing and taking away its six entries, a person's own file kept, every kind
#     converted beside the original;
#   - the clipboard's text actions and the QR code, hidden while the screen is shared;
#   - Share this Wi-Fi: the WIFI: text escaped, and the row only where there is Wi-Fi;
#   - ten disables and enables leaving nothing behind.
# vmlab-paths: @ui/extension.js @ui/*tools* @ui/modules/pill/privacy.js @ui/modules/search/clipboard.js @ui/modules/dock/places.js @sf/*themed/shell-tools* @sf/usr/libexec/amethystora-what-uses @sf/usr/libexec/amethystora-convert @sf/*24-files-menu.sh @sf/*just/desktop.just @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/usr/share/gnome-shell/extensions/amethystora-clipboard@*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-59
TOOLS=org.gnome.shell.extensions.amethystora-ui.tools
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
pa() { in_session "pactl $*" 2>/dev/null; }
# JSON compared without its spaces: the expressions stringify compactly, the expectations are written with ", "
same() { if [[ "${2//, /,}" == "${3//, /,}" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
T="${UI}.get('tools')"
# One field of the tools module's state: a string as it is, anything else as JSON
st() { ev "(v => typeof v === 'string' ? v : JSON.stringify(v))(${T}.state()$1)"; }
# A virtual pointer and keyboard of the shell's own seat
INPUT="const C = imports.gi.Clutter, GLib = imports.gi.GLib;
const seat = global.stage.context.get_backend().get_default_seat();
globalThis.__vmp ??= seat.create_virtual_device(C.InputDeviceType.POINTER_DEVICE);
globalThis.__vmk ??= seat.create_virtual_device(C.InputDeviceType.KEYBOARD_DEVICE);
const now = () => GLib.get_monotonic_time();"
move() { ev "${INPUT} __vmp.notify_absolute_motion(now(), $1, $2); 'moved'" >/dev/null; sleep 0.4; }
click() {
    ev "${INPUT} __vmp.notify_absolute_motion(now(), $1, $2); GLib.timeout_add(0, 120, () => {
        __vmp.notify_button(now(), C.BUTTON_PRIMARY, C.ButtonState.PRESSED);
        GLib.timeout_add(0, 120, () => { __vmp.notify_button(now(), C.BUTTON_PRIMARY, C.ButtonState.RELEASED); return false; });
        return false; }); 'clicked'" >/dev/null
    sleep 1
}
# Keys pressed in order and let go in the other order: keys Super_L Alt_L c
keys() {
    ev "${INPUT} const k = [$(printf 'C.KEY_%s,' "$@")]; k.forEach((v, i) => __vmk.notify_keyval(now() + i * 1000, v, C.KeyState.PRESSED));
        [...k].reverse().forEach((v, i) => __vmk.notify_keyval(now() + (k.length + i) * 1000, v, C.KeyState.RELEASED)); 'keys'" >/dev/null
    sleep 1
}
key_down() { ev "${INPUT} __vmk.notify_keyval(now(), C.KEY_$1, C.KeyState.PRESSED); 'down'" >/dev/null; sleep 0.2; }
key_up() { ev "${INPUT} __vmk.notify_keyval(now(), C.KEY_$1, C.KeyState.RELEASED); 'up'" >/dev/null; sleep 0.2; }
# What the clipboard holds, read back as text
clip() {
    ev "imports.gi.St.Clipboard.get_default().get_text(imports.gi.St.ClipboardType.CLIPBOARD, (c, t) => { globalThis.__vmclip = t; }); 'asked'" >/dev/null
    sleep 0.5
    ev "String(globalThis.__vmclip ?? '')"
}
share() { # UNIT ARGS...
    local unit="$1"
    shift
    rm -f "${WORK}/${unit}.log"
    user_bus systemd-run --user --collect --unit="${unit}" --property=StandardOutput=file:"${WORK}/${unit}.log" "$@" >/dev/null
}
started() { grep -q started "${WORK}/$1.log" 2>/dev/null; }

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
cp "${HERE}/sharecast.py" "${WORK}/"
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell" "${WORK}"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
for key in $(in_session "gsettings list-keys ${TOOLS}"); do gs reset "${TOOLS}" "${key}"; done
loginctl unlock-sessions
systemctl restart gdm
tools_ready() { [[ "$(ev "typeof ${T}?.state")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 tools_ready; then
    not_ok "the tools module did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
cleanup() {
    for key in $(in_session "gsettings list-keys ${TOOLS}" 2>/dev/null); do gs reset "${TOOLS}" "${key}"; done
    in_session "/usr/libexec/amethystora-convert menu off" >/dev/null 2>&1
    user_bus systemctl --user stop vmlab-share vmlab-hold vmlab-rec >/dev/null 2>&1
    pa unload-module module-null-sink >/dev/null 2>&1
    [[ -n "${ROOT_PID:-}" ]] && kill "${ROOT_PID}" 2>/dev/null
    ev "${T}?.picker?.close(); 'closed'" >/dev/null 2>&1
    loginctl unlock-sessions
}
trap cleanup EXIT

# --- 1: the keys, the place -----------------------------------------------------------------------------------------
same "1 the schema's keys and defaults" \
    "$(GSETTINGS_BACKEND=memory gsettings list-recursively "${TOOLS}" | cut -d' ' -f2- | LC_ALL=C sort | paste -sd ';')" \
    "colour-picker ['<Super><Alt>c'];convert-in-files false;mute-microphone ['<Super><Alt>m']"
order="$(ev "[...${UI}.keys()].join(' ')")"
[[ " ${order} " =~ \ privacy\ (.*\ )?tools\ (.*\ )?focus\ $ ]] && ok "1 tools runs after privacy and before focus, focus last" ||
    not_ok "1 the modules' order" "${order}"
same "1 both keys bound, the picker closed, nothing held" "$(st ".bound.sort().join(',')") $(st ".picker") $(st ".qr")" \
    "colour-picker,mute-microphone null null"

# --- 2-3: the colour picker -----------------------------------------------------------------------------------------
ev "Main.layoutManager.uiGroup.add_child(globalThis.__vmswatch = new imports.gi.St.Widget({x: 300, y: 300, width: 200, height: 200,
    style: 'background-color: #3a7bd5;'})); 'painted'" >/dev/null
sleep 1
picker_open() { [[ "$(st ".picker?.open")" == true ]]; }
picker_closed() { [[ "$(st ".picker")" == null ]]; }
move 400 400
keys Super_L Alt_L c
if wait_for 10 picker_open; then
    ok "2 Super+Alt+C opens the loupe"
    move 401 401
    same "2 the loupe names the colour under the pointer" "$(st ".picker.colour")" "#3A7BD5"
    same "2 the screen is held: the shell is modal" "$(ev "Main.modalCount > 0")" true
    click 400 400
    wait_for 10 picker_closed
    same "2 a click copies it as hex" "$(clip)" "#3A7BD5"
    same "2 ...and the picker closes, the grab given back" "$(st ".picker") $(ev "Main.modalCount")" "null 0"
    keys Super_L Alt_L c
    wait_for 10 picker_open
    key_down Shift_L
    click 400 400
    key_up Shift_L
    wait_for 10 picker_closed
    same "2 Shift+click copies rgb()" "$(clip)" "rgb(58, 123, 213)"
    keys Super_L Alt_L c
    wait_for 10 picker_open
    key_down Control_L
    click 400 400
    key_up Control_L
    wait_for 10 picker_closed
    same "2 Ctrl+click copies hsl()" "$(clip)" "hsl(215, 65%, 53%)"
    ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.CLIPBOARD, 'untouched'); 'set'" >/dev/null
    keys Super_L Alt_L c
    wait_for 10 picker_open
    keys Escape
    wait_for 10 picker_closed
    same "2 Esc copies nothing" "$(clip) $(st ".picker")" "untouched null"
    same "2 the last colours, newest first, each once" "$(st ".colours.join('|')")" "hsl(215, 65%, 53%)|rgb(58, 123, 213)|#3A7BD5"
    ev "(() => { const p = ${UI}.get('search')._providers.get('clipboard'); globalThis.__vmids = null;
        p.getInitialResultSet(['colour:']).then(ids => { globalThis.__vmids = ids; }); return 'asked'; })()" >/dev/null
    sleep 1
    same "2 the search's colour: lists them" "$(ev "globalThis.__vmids.join('|')")" \
        "colour:hsl(215, 65%, 53%)|colour:rgb(58, 123, 213)|colour:#3A7BD5"
else
    not_ok "2 Super+Alt+C did not open the loupe" "$(st)"
fi
share vmlab-share python3 "${WORK}/sharecast.py" 60
if wait_for 20 started vmlab-share; then
    keys Super_L Alt_L c
    sleep 2
    same "3 while the screen is shared the picker does not open" "$(st ".picker") $(ev "Main.modalCount")" "null 0"
else
    not_ok "3 the stand-in share did not start" "$(cat "${WORK}/vmlab-share.log" 2>&1)"
fi
user_bus systemctl --user stop vmlab-share >/dev/null 2>&1
ev "globalThis.__vmswatch?.destroy(); 'gone'" >/dev/null

# --- 4-5: the microphone --------------------------------------------------------------------------------------------
pa load-module module-null-sink sink_name=vmlab-mic media.class=Audio/Source/Virtual >/dev/null
pa set-default-source vmlab-mic
sleep 2
muted() { [[ "$(pa get-source-mute @DEFAULT_SOURCE@)" == *yes* ]]; }
unmuted() { [[ "$(pa get-source-mute @DEFAULT_SOURCE@)" == *no* ]]; }
pa set-source-mute @DEFAULT_SOURCE@ 0
keys Super_L Alt_L m
check "4 Super+Alt+M mutes the default input" wait_for 10 muted
mark() { [[ "$(st ".microphone.mark")" == true ]]; }
check "4 Mic off shows in the bar while it is muted" wait_for 10 mark
same "4 the mark sits in the right box, before the system menu" \
    "$(ev "Main.panel._rightBox.contains(Main.panel.statusArea['amethystora-tools-mic']?.container ?? null)")" true
pa load-module module-null-sink sink_name=vmlab-mic2 media.class=Audio/Source/Virtual >/dev/null
pa set-default-source vmlab-mic2
sleep 2
check "4 a microphone chosen while muted from here is muted too" wait_for 10 muted
keys Super_L Alt_L m
check "4 Super+Alt+M again turns it back on" wait_for 10 unmuted
no_mark() { [[ "$(st ".microphone.mark")" == false ]]; }
check "4 ...and the mark goes" wait_for 10 no_mark
pa set-default-source vmlab-mic
pa set-source-mute @DEFAULT_SOURCE@ 0
user_bus systemd-run --user --collect --unit=vmlab-rec -- parecord --device=vmlab-mic /dev/null >/dev/null
row_mute() { [[ "$(ev "(() => { const p = ${UI}.get('pill').privacy; p?._button?.menu.open(); return String(!!p?._listRows?.microphone?.mute?.visible); })()")" == true ]]; }
check "5 the privacy list's microphone row has Mute while it records" wait_for 20 row_mute
ev "(() => { const p = ${UI}.get('pill').privacy; p._listRows.microphone.mute.emit('clicked', 1); p._button.menu.close(); return 'clicked'; })()" >/dev/null
check "5 ...which mutes every app" wait_for 10 muted
keys Super_L Alt_L m
user_bus systemctl --user stop vmlab-rec >/dev/null 2>&1

# --- 6: What is using this? -----------------------------------------------------------------------------------------
STICK="${WORK}/stick"
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${STICK}/photos"
user_bus systemd-run --user --collect --unit=vmlab-hold --working-directory="${STICK}/photos" -- sleep 600 >/dev/null
( cd "${STICK}" && exec sleep 600 ) &
ROOT_PID=$!
sleep 2
HOLD_PID="$(user_bus systemctl --user show -p MainPID --value vmlab-hold)"
LIST="$(as_user "/usr/libexec/amethystora-what-uses list --json -- ${STICK}")"
same "6 the helper names the account's own program and why" \
    "$(jq -c --argjson p "${HOLD_PID}" '[.holders[] | select(.pid == $p) | [.name, .reasons[0].kind, .reasons[0].path]]' <<<"${LIST}")" \
    "[[\"sleep\",\"cwd\",\"${STICK}/photos\"]]"
same "6 ...and never another account's" "$(jq --argjson p "${ROOT_PID}" '[.holders[] | select(.pid == $p)] | length' <<<"${LIST}")" 0
START="$(awk '{print $22}' "/proc/${HOLD_PID}/stat")"
same "6 quit refuses a process whose start time is not the one listed" \
    "$(as_user "/usr/libexec/amethystora-what-uses quit ${HOLD_PID} $((START + 1))"; echo $?)" 1
same "6 ...and another account's" "$(as_user "/usr/libexec/amethystora-what-uses quit ${ROOT_PID} $(awk '{print $22}' "/proc/${ROOT_PID}/stat")"; echo $?)" 1
user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/amethystora/Tools \
    --method org.amethystora.Tools.ShowUsers "['${STICK}']" >/dev/null
rows() { [[ "$(st ".users.rows?.length ?? -1")" -ge 1 ]]; }
if wait_for 20 rows; then
    same "6 ShowUsers opens the dialog, naming it and what it holds" "$(st ".users.rows.map(r => r.name + ': ' + r.detail).join('|')")" \
        "sleep: its folder is open here"
    ev "(() => { const l = ${T}.users._dialog.list; l._quit(l.rows[0]); return 'quit'; })()" >/dev/null
    gone() { ! kill -0 "${HOLD_PID}" 2>/dev/null; }
    check "6 its Quit ends the program (checked by the helper)" wait_for 10 gone
    empty() { [[ "$(st ".users.rows?.length")" == 0 ]]; }
    check "6 ...and the list, read again, is empty" wait_for 10 empty
    ev "${T}.users._dialog?.close(); 'closed'" >/dev/null
else
    not_ok "6 ShowUsers opened no dialog" "$(st ".users")"
fi
same "6 a path that is not absolute is refused" "$(user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/amethystora/Tools \
    --method org.amethystora.Tools.ShowUsers "['relative']" >/dev/null; sleep 1; st ".users.dialog")" false
# GNOME's own busy dialog, as Files asks the shell for it when an eject finds the drive in use
user_bus systemd-run --user --collect --unit=vmlab-busy -- gdbus call --session --timeout 60 \
    --dest org.gtk.MountOperationHandler --object-path /org/gtk/MountOperationHandler \
    --method org.Gtk.MountOperationHandler.ShowProcesses "lab" "Volume is busy" "" "[]" "['Unmount Anyway', 'Cancel']" >/dev/null
BUSY="Main.layoutManager.modalDialogGroup.get_children().find(c => c.dialogLayout?._dialog?.has_style_class_name?.('processes-dialog'))"
busy_line() { [[ "$(ev "String(${BUSY}?._amethystoraUsers?.label ?? '')")" == "Show what is using it" ]]; }
check "6 GNOME's busy dialog gains Show what is using it" wait_for 15 busy_line
ev "${BUSY}?._amethystoraUsers?.emit('clicked', 1); 'shown'" >/dev/null
sleep 3
same "6 ...which lists, in the dialog, what holds the removable drives (none in the lab)" \
    "$(ev "JSON.stringify(${BUSY}?._amethystoraUsers?.rows?.length ?? -1)")" 0
ev "${BUSY}?.emit('response', -1); 'closed'" >/dev/null
kill "${ROOT_PID}" 2>/dev/null
ROOT_PID=""
SCRIPTS="${HOME_DIR}/.local/share/nautilus/scripts"
check "6 Files' Scripts menu has What is using this?, as the hook writes it" \
    grep -qx 'exec /usr/libexec/amethystora-what-uses show -- "$@"' "${SCRIPTS}/What is using this?"

# --- 7-8: Convert in Files ------------------------------------------------------------------------------------------
in_session "ame desktop files-convert on" >/dev/null
same "7 on: six entries under Convert" "$(ls "${SCRIPTS}/Convert" 2>/dev/null | LC_ALL=C sort | paste -sd ',')" \
    "1920 pixels wide,As JPEG,As PNG,As WebP,Half the size,Into one PDF"
same "7 ...each handing Files' arguments on after --" "$(sed -n 3p "${SCRIPTS}/Convert/As WebP")" \
    'exec /usr/libexec/amethystora-convert webp -- "$@"'
same "7 the key says so, and Control" "$(gs get "${TOOLS}" convert-in-files) $(in_session "/usr/libexec/amethystora-control-list --json" |
    jq -r '.. | objects | select(.id? == "files-convert") | .state')" "true on"
as_user "printf 'mine\n' >'${SCRIPTS}/Convert/Mine'"
in_session "ame desktop files-convert off" >/dev/null
same "7 off takes its six away and keeps a script of the person's own" "$(ls "${SCRIPTS}/Convert" 2>/dev/null | paste -sd ',')" "Mine"
rm -rf "${SCRIPTS}/Convert"
gs set "${TOOLS}" convert-in-files true
written() { [[ -x "${SCRIPTS}/Convert/As PNG" ]]; }
check "7 the key alone (as the setup file sets it) has the module write the menu" wait_for 20 written
gs set "${TOOLS}" convert-in-files false
taken() { [[ ! -e "${SCRIPTS}/Convert" ]]; }
check "7 ...and take it away" wait_for 20 taken
PICS="${WORK}/pictures"
rm -rf "${PICS}"
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${PICS}"
as_user "cd '${PICS}' && python3 -c '
import gi; gi.require_version(\"GdkPixbuf\", \"2.0\"); from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, True, 8, 2400, 1600); p.fill(0x9148F080); p.savev(\"wide.png\", \"png\", [], [])
q = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, False, 8, 800, 1200); q.fill(0x20A040FF); q.savev(\"tall.jpg\", \"jpeg\", [\"quality\"], [\"90\"])
'"
for kind in png jpeg webp half 1920 pdf; do
    as_user "cd '${PICS}' && /usr/libexec/amethystora-convert ${kind} -- wide.png tall.jpg" >/dev/null 2>&1
done
size() { as_user "python3 -c 'import gi, sys; gi.require_version(\"GdkPixbuf\", \"2.0\"); from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1]); print(p.get_width(), p.get_height(), p.get_has_alpha())' '$1'" 2>/dev/null; }
same "8 every kind written beside the originals, and only those" "$(ls "${PICS}" | LC_ALL=C sort | paste -sd ',')" \
    "tall (half size).jpg,tall.jpg,tall.png,tall.webp,wide (1920 px).png,wide (half size).png,wide.jpg,wide.pdf,wide.png,wide.webp"
same "8 a JPEG over white, a WebP with its alpha, half and 1920 px wide" \
    "$(size "${PICS}/wide.jpg") / $(size "${PICS}/wide.webp") / $(size "${PICS}/wide (half size).png") / $(size "${PICS}/wide (1920 px).png")" \
    "2400 1600 False / 2400 1600 True / 1200 800 True / 1920 1280 True"
same "8 the PDF a page each, the photo as a JPEG" "$(pdfinfo "${PICS}/wide.pdf" | awk '/^Pages:/ {print $2}') $(grep -ac DCTDecode "${PICS}/wide.pdf")" "2 1"
as_user "cd '${PICS}' && /usr/libexec/amethystora-convert jpeg -- wide.png" >/dev/null 2>&1
same "8 a name already taken is never written over" "$(ls "${PICS}" | grep -c '^wide.*jpg$') $(size "${PICS}/wide 2.jpg")" "2 2400 1600 False"

# --- 9: the clipboard's text actions and the QR code ------------------------------------------------------------------
CI="Main.panel.statusArea.amethystoraClipboard"
ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.CLIPBOARD, '  hello   lab world  '); 'set'" >/dev/null
recorded() { [[ "$(ev "${CI}.clipItemsRadioGroup.some(i => i.clipContents === '  hello   lab world  ')")" == true ]]; }
if wait_for 20 recorded; then
    ev "(() => { ${CI}.clipItemsRadioGroup.find(i => i.clipContents === '  hello   lab world  ').actionsBtn.emit('clicked', 1); return 'opened'; })()" >/dev/null
    sleep 1
    same "9 Make a new copy offers the actions that change it, and the QR code" \
        "$(ev "${CI}._textActionsDialog.textActions.map(a => a.id).join(' ') + ' ' + !!${CI}._textActionsDialog.qr")" \
        "upper title trim line true"
    ev "(() => { const d = ${CI}._textActionsDialog; d._copy(d.textActions.find(a => a.id === 'upper').result); return 'copied'; })()" >/dev/null
    sleep 1
    same "9 UPPER CASE is a new item at the top, what the clipboard holds, the original kept" \
        "$(ev "${CI}.clipItemsRadioGroup.at(-1).clipContents + '|' + ${CI}.clipItemsRadioGroup.some(i => i.clipContents === '  hello   lab world  ')")|$(clip)" \
        "  HELLO   LAB WORLD  |true|  HELLO   LAB WORLD  "
    ev "${T}.showQrCode('https://amethystora.org/manual'); 'shown'" >/dev/null
    qr_shown() { [[ "$(st ".qr?.shown")" == true ]]; }
    check "9 the QR code is drawn" wait_for 10 qr_shown
    share vmlab-share python3 "${WORK}/sharecast.py" 60
    qr_hidden() { [[ "$(st ".qr?.shown") $(st ".qr?.covered")" == "false true" ]]; }
    if wait_for 20 started vmlab-share; then
        check "9 a share that starts hides it at once" wait_for 10 qr_hidden
    else
        not_ok "9 the stand-in share did not start"
    fi
    user_bus systemctl --user stop vmlab-share >/dev/null 2>&1
    ev "${T}._qr?.close(); 'closed'" >/dev/null
else
    not_ok "9 the clipboard did not record the copy"
fi

# --- 10: Share this Wi-Fi ---------------------------------------------------------------------------------------------
ev "import('file:///usr/share/gnome-shell/extensions/${UUID}/modules/tools/wifi.js').then(m => {
    globalThis.__vmwifi = [m.wifiText({ssid: 'Caf;e \"x\"', kind: 'WPA', password: 'pa:ss,'}),
        m.wifiText({ssid: 'Open', kind: 'nopass', hidden: true})]; }); 'asked'" >/dev/null
sleep 1
same "10 the WIFI: text, every special character escaped" "$(ev "globalThis.__vmwifi.join(' | ')")" \
    'WIFI:T:WPA;S:Caf\;e \"x\";P:pa\:ss\,;; | WIFI:T:nopass;S:Open;H:true;;'
if nmcli -t -f TYPE,STATE device 2>/dev/null | grep -q '^wifi:connected'; then
    same "10 the Wi-Fi menu offers Share this Wi-Fi" "$(ev "(() => { const w = ${T}.wifi; w._sync(); return String(w.state.offered); })()")" true
else
    skip "10 Share this Wi-Fi in the menu: the lab's machine has no Wi-Fi"
    same "10 ...where there is none, the row is there and hidden" "$(st ".wifi.row") $(st ".wifi.offered")" "true false"
fi

# --- 11: ten disables and enables -------------------------------------------------------------------------------------
for _ in $(seq 10); do
    # GNOME 51 carries a disable out only after the Eval that asked has returned: enable in a second one
    ev "Main.extensionManager.disableExtension('${UUID}'); 'down'" >/dev/null
    sleep 1
    ev "Main.extensionManager.enableExtension('${UUID}'); 'up'" >/dev/null
    sleep 1
done
wait_for 60 tools_ready
same "11 after ten: the keys bound once, one D-Bus object, no picker, no mark, one Share row" \
    "$(ev "JSON.stringify([${T}.state().bound.length, Main.panel._rightBox.get_children().filter(c => c.get_child?.()?.has_style_class_name?.('amethystora-tools-mic-mark')).length,
        Main.panel.statusArea.quickSettings._network?._wirelessToggle?.menu._getMenuItems().filter(i => i.has_style_class_name?.('amethystora-tools-wifi-share')).length ?? 1])") $(st ".picker")" \
    "[2, 0, 1] null"
ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
sleep 2
same "11 off: the D-Bus object is gone, the keys unbound, Share gone from the Wi-Fi menu" \
    "$(user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/amethystora/Tools \
        --method org.amethystora.Tools.ShowUsers "['/tmp']" >/dev/null 2>&1 && echo there || echo gone) $(ev "String((Main.wm._allowedKeybindings?.['colour-picker'] ?? 0) === 0)") $(ev "String(Main.panel.statusArea.quickSettings._network?._wirelessToggle?.menu._getMenuItems().some(i => i.has_style_class_name?.('amethystora-tools-wifi-share')) ?? false)")" \
    "gone true false"
ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
wait_for 60 tools_ready

finish
