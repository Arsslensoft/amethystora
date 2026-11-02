#!/usr/bin/bash
# vmlab-vm: windows
# The windows module of Amethystora UI (plans/beyond/spec-windows.md, section 7), in the classic layout unless a
# check says otherwise, asked of the session through GNOME Shell's Eval (the lab-only vmlab-eval@lab extension, never
# in the image) and driven as a person would, with keys.py's keyboard and pointers and touchpad.py's touchpad:
#   15.0  the schema's keys and their defaults (rounds 5 and 6 added eleven: suite 57), the module running;
#   15.1  snap layouts: Super+Z and its thirteen places, to the pixel; a drag to the top and onto a place; the old size
#         back when dragged away; GNOME's maximise at the very top; nothing in the tiling layout or with snap off;
#   15.2  the switcher: the grid, typing (which keeps it open), Alt+Tab's quick switch, Escape, no shortcut firing on
#         the typed letters, GNOME's own in the tiling layout, a title that is never markup;
#   15.3  holding Super: the card and the dock's numbers after a second, no overview on letting go, a tap still opens
#         it, a short hold or a shortcut shows nothing, the card's rows as this machine has them, the delay key;
#   15.4  hot corners: barriers only for corners in use, a push fires and a slow drift does not, each action;
#   15.5  show desktop: slivers of exactly 28 px, and every way back;
#   15.6  touchpad gestures, on a uinput touchpad, or through the recogniser's own entry when libinput takes none;
#   15.7  places: saved, restored in the first frame, a bad file, the caps, nothing in the tiling layout, forget;
#   15.8  a copy kept in view: on every workspace, its corner, a drag, a click, a close, a lock, a disable;
#   15.9  dimming: the strengths, the pixels at two thirds, nothing in the overview;
#   15.10 the cost: nothing runs while nothing happens, ten disables leave nothing behind, frames under 1 ms;
#   and nothing of Amethystora UI throws meanwhile.
# vmlab-paths: @ui/extension.js @ui/*windows* @sf/*themed/shell-windows* @sf/*just/desktop.just @ui/*desks*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
STATE_DIR="${HOME_DIR}/.local/state/amethystora"
SCHEMA=org.gnome.shell.extensions.amethystora-ui.windows
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
touchpad() { python3 "$(dirname "$0")/touchpad.py" "$@"; }
# Eval's answer decoded once: gdbus prints a GVariant string holding JSON.stringify of the value, so a string comes
# back as itself, a switch as true or false, a number as digits and anything else as compact JSON; a throw prints its
# message and fails
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c '
import ast, json, sys
out = sys.stdin.read().strip()
if not (out.startswith("(") and "," in out):
    print(out)
    sys.exit(1)
text = ast.literal_eval(out[out.index(",") + 1:-1].strip())
if not out.startswith("(true,"):
    print(text)
    sys.exit(1)
try:
    value = json.loads(text)
except ValueError:
    print(text)
    sys.exit(0)
if isinstance(value, bool):
    print("true" if value else "false")
elif value is None:
    print("null")
elif isinstance(value, (str, int, float)):
    print(value)
else:
    print(json.dumps(value, separators=(",", ":"), ensure_ascii=False))
'
}
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
W="${UI}.get('windows')"
gs() { in_session gsettings "$@"; }
ame_() { in_session ame "$@"; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3"$'\n'"got: $2"; fi; }
is_true() { [[ "$(ev "$1")" == true ]]; }
is_false() { [[ "$(ev "$1")" == false ]]; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

# The windows by their class, and the one in use
WIN="global.get_window_actors().map(a => a.meta_window)"
files() { echo "${WIN}.find(w => w.get_wm_class() === 'org.gnome.Nautilus')"; }
rect_of() { ev "(() => { const r = $1.get_frame_rect(); return [r.x, r.y, r.width, r.height].join(','); })()"; }
focused() { ev "global.display.focus_window?.get_wm_class()"; }
count_class() { ev "${WIN}.filter(w => w.get_wm_class() === '$1').length"; }
at_least() { (($(count_class "$1") >= $2)); }
# The primary work area, and fractions of the whole screen for keys.py
AREA="Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex)"
frac() { ev "(() => { const x = $1, y = $2; return (x / global.screen_width).toFixed(5) + ',' + (y / global.screen_height).toFixed(5); })()"; }
# A cell's rect as the spec computes it: X + round(W·a) .. X + round(W·b), likewise down
cell_rect() {
    ev "(() => { const a = ${AREA}; const [p, q, r, s] = [$1]; const x1 = a.x + Math.round(a.width * p), x2 = a.x + Math.round(a.width * q), y1 = a.y + Math.round(a.height * r), y2 = a.y + Math.round(a.height * s); return [x1, y1, x2 - x1, y2 - y1].join(','); })()"
}
open_files() {
    user_bus systemd-run --user --collect --unit="vmlab-files-$$-${RANDOM}" nautilus --new-window >/dev/null
}
open_manual() {
    user_bus systemd-run --user --collect --unit="vmlab-manual-$$" amethystora-manual >/dev/null
}
activate_files() { ev "Main.activateWindow($(files))" >/dev/null; sleep 1; }
classic() { ame_ desktop layout classic >/dev/null; sleep 2; }
tiling() { ame_ desktop layout tiling >/dev/null; sleep 2; }

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
# The windows an earlier suite of the same lane left open (Weather, a test app) would be counted by the switcher: only
# the Manual, Files and the lab's own stay
ev "global.get_window_actors().map(a => a.meta_window).filter(w => !['org.amethystora.Manual', 'org.gnome.Nautilus'].includes(w.get_wm_class()) && !w.get_wm_class()?.startsWith('org.vmlab.')).forEach(w => w.delete(global.get_current_time())); 'closed'" >/dev/null
# The lab renders in software, where GNOME turns animations off: the animated paths are what is tested here
ev "global.force_animations = true; 'forced'" >/dev/null
# Put back however the suite ends
restore_animations() { ev "global.force_animations = true; 'unforced'" >/dev/null 2>&1; }
trap restore_animations EXIT
was_tiling="$(gs get org.gnome.shell.extensions.amethystora-ui.tiling enabled)"
classic

# --- 15.0 Schema and defaults ---------------------------------------------------------------------------------------
same "the windows schema has the spec's keys and defaults" \
    "$(gs list-recursively "${SCHEMA}" | cut -d' ' -f2- | LC_ALL=C sort | paste -sd ';')" \
    "corner-bottom-left 'none';corner-bottom-right 'none';corner-top-left 'none';corner-top-right 'none';dim false;drop-down true;drop-down-key ['<Super><Alt>Return'];frame false;gesture-down true;gesture-pinch true;gesture-spread true;gesture-today true;hold-super true;hold-super-delay 1000;keep-on-top true;keep-on-top-key ['<Super><Alt>p'];places true;recording-sizes true;reopen true;reopen-key ['<Super><Alt>t'];scratchpad true;see-through false;see-through-apps @a{si} {};show-desktop false;snap true;snap-chooser ['<Super>z'];switcher true"
check "the windows module runs" is_true "${UI}.has('windows')"

# A plain window with no size of its own to defend (Files will not shrink to a quarter of the lab's 1280 x 800), for
# the snap layouts, the places and the pixels: an app of its own, any number of windows at once
in_session "cat >/tmp/vmlab-plain.js <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: 'org.vmlab.Plain', flags: 4});
app.connect('activate', () => { const w = new Gtk.ApplicationWindow({application: app, title: 'Plain', default_width: 640, default_height: 400}); w.present(); });
app.run([]);
EOF"
open_plain() { user_bus systemd-run --user --collect --unit="vmlab-plain-$-${RANDOM}" gjs -m /tmp/vmlab-plain.js >/dev/null; }
plain() { echo "${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain').sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence())[0]"; }
activate_plain() { ev "Main.activateWindow($(plain))" >/dev/null; sleep 1; }
open_plain
wait_for 60 at_least org.vmlab.Plain 1
open_files
wait_for 60 at_least org.gnome.Nautilus 1
open_manual
wait_for 60 at_least org.amethystora.Manual 1
sleep 2
activate_plain

# --- 15.1 Snap layouts ----------------------------------------------------------------------------------------------
chooser() { is_true "${W}.snap.chooserVisible"; }
no_chooser() { is_false "${W}.snap.chooserVisible"; }
keys super+z
check "Super+Z shows the snap layouts" wait_for 4 chooser
keys esc
check "Escape closes them" wait_for 4 no_chooser

keys super+z right return
sleep 1
same "Super+Z, Right, Return puts the window on the right half, to the pixel" "$(rect_of "$(plain)")" "$(cell_rect '1/2, 1, 0, 1')"

# Every place by the keyboard: the chooser opens on the first; Right steps through the thirteen in reading order
FRACTIONS=('0, 1/2, 0, 1' '1/2, 1, 0, 1' '0, 2/3, 0, 1' '2/3, 1, 0, 1' '0, 1/3, 0, 1' '1/3, 2/3, 0, 1' '2/3, 1, 0, 1'
    '0, 1/2, 0, 1/2' '1/2, 1, 0, 1/2' '0, 1/2, 1/2, 1' '1/2, 1, 1/2, 1' '1/4, 3/4, 0, 1')
for i in "${!FRACTIONS[@]}"; do
    steps=(super+z)
    for ((j = 0; j < i; j++)); do steps+=(right); done
    keys "${steps[@]}" return
    sleep 1
    same "place $((i + 1)) of 13 by the keyboard is its cell, to the pixel" "$(rect_of "$(plain)")" "$(cell_rect "${FRACTIONS[$i]}")"
done
steps=(super+z)
for ((j = 0; j < 12; j++)); do steps+=(right); done
keys "${steps[@]}" return
check "place 13, the whole screen, maximises" wait_for 4 is_true "$(plain).is_maximized()"
ev "$(plain).unmaximize(); $(plain).move_resize_frame(true, 200, 200, 900, 600); 'placed'" >/dev/null
sleep 1

# A drag by the title bar to the top shows them, and dropping on the middle third puts it there
before="$(rect_of "$(plain)")"
title="$(frac 650 215)"
top="$(frac "${AREA}.x + ${AREA}.width / 2" "${AREA}.y + 30")"
middle="$(ev "(() => { const e = ${W}.snap._targets[5].actor.get_transformed_extents(); return ((e.origin.x + e.size.width / 2) / global.screen_width).toFixed(5) + ',' + ((e.origin.y + e.size.height / 2) / global.screen_height).toFixed(5); })()")"
snapmoves="$(ev "${W}.counters.snapMove ?? 0")"
keys "move:${title}" btn:down "glide:${title},${top},600" "glide:${top},${middle},400" btn:up
sleep 1
same "a drag to the top and onto the middle third puts it there, to the pixel" "$(rect_of "$(plain)")" "$(cell_rect '1/3, 2/3, 0, 1')"
check "the drag moved the chooser's pointer test" test "$(ev "${W}.counters.snapMove ?? 0")" -gt "${snapmoves}"
check "the chooser is gone after the drop" no_chooser

# Dragged away, it has its old size back
snapped="$(rect_of "$(plain)")"
grab="$(frac "$(cut -d, -f1 <<<"${snapped}") + 200" "$(cut -d, -f2 <<<"${snapped}") + 15")"
away="$(frac 900 500)"
keys "move:${grab}" btn:down "glide:${grab},${away},500" btn:up
sleep 1
size_back() { [[ "$(rect_of "$(plain)" | cut -d, -f3,4)" == "$(cut -d, -f3,4 <<<"${before}")" ]]; }
check "dragged away, it has its size from before the snap again" wait_for 3 size_back

# The very top, inside the bar, is GNOME's maximise
here="$(frac "$(rect_of "$(plain)" | cut -d, -f1) + 200" "$(rect_of "$(plain)" | cut -d, -f2) + 15")"
very_top="$(frac 960 2)"
keys "move:${here}" btn:down "glide:${here},${very_top},600" sleep:600 btn:up
check "a drop at the very top still maximises, as GNOME does" wait_for 4 is_true "$(plain).is_maximized()"
check "and the chooser is hidden" no_chooser
ev "$(plain).unmaximize(); 'ok'" >/dev/null

tiling
keys super+z
sleep 1
check "in the tiling layout Super+Z shows no snap layouts" no_chooser
classic
activate_plain
keys super+z
check "back in classic, Super+Z works again" wait_for 4 chooser
keys esc
ame_ desktop windows snap off >/dev/null
sleep 1
keys super+z
sleep 1
check "with snap off, Super+Z does nothing" no_chooser
ame_ desktop windows snap on >/dev/null

# --- 15.2 The switcher ----------------------------------------------------------------------------------------------
in_session "cat >/tmp/vmlab-bold.js <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: 'org.vmlab.Bold'});
app.connect('activate', () => { const w = new Gtk.ApplicationWindow({application: app, title: '<b>bold</b>'}); w.present(); });
app.run([]);
EOF"
user_bus systemd-run --user --collect --unit="vmlab-bold-$$" gjs -m /tmp/vmlab-bold.js >/dev/null
wait_for 60 at_least org.vmlab.Bold 1
activate_files
switcher_open() { is_true "${W}.switcher.visible"; }
switcher_shut() { is_false "${W}.switcher.visible"; }
# Super is held by a keys.py of its own in the background while the checks run: a keyboard that goes lets go of its
# keys, so a held key has to stay in one process
keys down:super tab sleep:12000 up:super &
held=$!
check "Super+Tab shows the grid" wait_for 8 switcher_open
same "it lists the four windows" "$(ev "${W}.switcher.titles().length")" 4
keys text:fil
same "typing fil keeps Files' tile only" "$(ev "${W}.switcher.titles().length + ' ' + ${W}.switcher.titles()[0].startsWith('Files')")" "1 true"
same "and the count reads 1 of 4" "$(ev "${W}.switcher.countText")" "1 of 4"
wait "${held}"
sleep 1
check "letting go of Super after typing leaves the grid open" switcher_open
keys return
check "Enter closes it" wait_for 3 switcher_shut
same "and Files has the keyboard" "$(focused)" org.gnome.Nautilus

ev "Main.activateWindow(${WIN}.find(w => w.get_wm_class() === 'org.vmlab.Bold'))" >/dev/null
sleep 1
activate_files
keys down:alt tab up:alt sleep:800
same "Alt+Tab released at once switches to the window used before" "$(focused)" org.vmlab.Bold
check "with no grid left" switcher_shut
activate_files
keys down:super tab sleep:600 esc up:super sleep:600
same "Super+Tab then Escape leaves the focus where it was" "$(focused)" org.gnome.Nautilus

# The typed letters fire none of their shortcuts
windows_before="$(ev "global.get_window_actors().length")"
keys down:super tab sleep:15000 up:super &
held=$!
wait_for 8 switcher_open
keys text:elasvwhn
same "every letter went into the query" "$(ev "${W}.switcher.query")" elasvwhn
keys esc
wait "${held}"
same "no window was opened by a letter" "$(ev "global.get_window_actors().length")" "${windows_before}"
check "the screen did not lock" test "$(loginctl show-session "$(loginctl show-user "${TEST_USER}" -p Display --value)" -p LockedHint --value)" == no
check "the overview did not open" is_false "Main.overview.visible"

# The title shown as it is
ev "${W}.switcher.open('switch-windows'); 'open'" >/dev/null
sleep 1
bold="$(ev "(() => { const t = ${W}.switcher._tiles.find(t => t.window.get_wm_class() === 'org.vmlab.Bold'); return t.name.get_text() + '|' + t.name.clutter_text.use_markup; })()")"
ev "${W}.switcher.close(true); 'closed'" >/dev/null
if [[ "${bold}" == *"<b>bold</b>|false" ]]; then
    ok "a window titled <b>bold</b> shows those very letters, never as markup"
else
    not_ok "a window titled <b>bold</b> shows those very letters, never as markup" "${bold}"
fi

tiling
keys down:super tab sleep:800
check "in the tiling layout Super+Tab is not the grid" switcher_shut
keys esc up:super
classic

# --- 15.3 Holding Super ---------------------------------------------------------------------------------------------
shown_count() { ev "${W}.hold.shownCount"; }
card() { is_true "${W}.hold.cardVisible"; }
no_card() { is_false "${W}.hold.cardVisible"; }
activate_files
keys down:super sleep:5000 up:super &
held=$!
check "after holding Super alone for a second the card shows" wait_for 8 card
check "and the dock numbers its apps" is_true "${UI}.get('dock')?.numbersShown"
wait "${held}"
check "letting go hides the card" wait_for 2 no_card
check "and the dock's numbers" is_false "${UI}.get('dock')?.numbersShown"
check "and does not open the overview" is_false "Main.overview.visible"
keys super
check "a tap of Super still opens the overview" wait_for 3 is_true "Main.overview.visible"
keys esc
wait_for 5 is_false "Main.overview.visible"
n="$(shown_count)"
keys down:super sleep:500 up:super sleep:1000
same "a half-second hold shows nothing" "$(shown_count)" "${n}"
ev "Main.overview.hide(); 'h'" >/dev/null
sleep 1
keys down:super sleep:300 e sleep:1200 up:super
sleep 2
same "Super+E typed during a hold shows nothing" "$(shown_count)" "${n}"
ev "Main.overview.hide(); global.get_window_actors().map(a => a.meta_window).filter(w => w.get_wm_class() === 'org.gnome.Nautilus').slice(1).forEach(w => w.delete(0)); 'h'" >/dev/null
same "the card's rows are this machine's shortcuts" \
    "$(ev "${W}.hold.rows().map(([n, k]) => n + '=' + k.join('+')).join(';')")" \
    "Search=Super+Space;Terminal=Super+Return;Manual=Super+F1;Emoji=Super+.;Clipboard=Super+Shift+V;Window layouts=Super+Z;Screenshot=Print;Lock the screen=Super+L;Control=Super+Ctrl+Shift+C"
check "its footer names Alt" test "$(ev "${W}.hold.footerText")" == "Alt+1 … 9"
# Quoted once more for the shell in_session runs it in
gs set org.gnome.shell.extensions.amethystora-clipboard toggle-menu "\"['<Super><Shift>c']\""
sleep 1
check "a key someone changed shows as they set it" test \
    "$(ev "${W}.hold.rows().find(([n]) => n === 'Clipboard')[1].join('+')")" == "Super+Shift+C"
gs reset org.gnome.shell.extensions.amethystora-clipboard toggle-menu
ame_ desktop windows hold-super 2000 >/dev/null
n="$(shown_count)"
keys down:super sleep:1300 up:super sleep:800
same "with a delay of two seconds, 1.3 s shows nothing" "$(shown_count)" "${n}"
ev "Main.overview.hide(); 'h'" >/dev/null
sleep 1
keys down:super sleep:2300 up:super sleep:500
same "and 2.3 s shows the card" "$(shown_count)" "$((n + 1))"
gs reset "${SCHEMA}" hold-super-delay
# A hold that showed the card, then a shortcut inside it (Super+2, a workspace): no overlay-key comes at the release,
# and the overview's toggle must not stay swallowed for whatever calls it next (a hot corner, the logo menu)
ev "Main.overview.hide(); 'h'" >/dev/null
sleep 1
n="$(shown_count)"
keys down:super sleep:1300 2 up:super sleep:600
same "the card showed during that hold" "$(shown_count)" "$((n + 1))"
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); Main.overview.toggle(); 't'" >/dev/null
check "after a shortcut typed inside a hold, the overview still opens when asked" wait_for 3 is_true "Main.overview.visible"
ev "Main.overview.hide(); 'h'" >/dev/null
sleep 1
ev "Main.overview.show(); 'o'" >/dev/null
sleep 2
n="$(shown_count)"
keys down:super sleep:1300 up:super sleep:800
same "with the overview open, a hold shows no card" "$(shown_count)" "${n}"
ev "Main.overview.hide(); 'h'" >/dev/null
sleep 1

# --- 15.4 Hot corners -----------------------------------------------------------------------------------------------
barriers() { ev "${W}.corners.barrierCount ?? 0"; }
same "with every corner at none there is no barrier" "$(barriers)" 0
ame_ desktop windows corner top-right overview >/dev/null
sleep 1
same "one corner in use on one screen is two barriers" "$(barriers)" 2
push=()
for i in $(seq 10); do push+=("rel:20,-20"); done
keys move:0.99,0.01 "${push[@]}" sleep:500
check "a push into the top right corner opens the overview" wait_for 3 is_true "Main.overview.visible"
keys esc
wait_for 5 is_false "Main.overview.visible"
keys move:0.9,0.3
slow=()
for i in $(seq 10); do slow+=("rel:2,-2" "sleep:200"); done
keys move:0.99,0.01 "${slow[@]}" sleep:500
check "a slow drift into it does not" is_false "Main.overview.visible"
ame_ desktop windows corner bottom-left notifications >/dev/null
sleep 1
push=()
for i in $(seq 10); do push+=("rel:-20,20"); done
# Through the middle first: the drift above left the pointer held at the top right corner's barrier, and the lab's
# absolute pointer jumping from there straight across the screen is stopped part way (a person's never jumps)
keys move:0.5,0.5 sleep:300 move:0.01,0.99 "${push[@]}" sleep:500
check "a push into the bottom left corner opens the notifications" wait_for 3 is_true "Main.panel.statusArea.dateMenu.menu.isOpen"
keys esc
ame_ desktop windows corner bottom-right lock >/dev/null
sleep 1
push=()
for i in $(seq 10); do push+=("rel:20,20"); done
keys move:0.5,0.5 sleep:300 move:0.99,0.99 "${push[@]}" sleep:500
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"
locked() { [[ "$(loginctl show-session "${SESSION}" -p LockedHint --value)" == yes ]]; }
check "a push into the bottom right corner locks the screen" wait_for 10 locked
loginctl unlock-session "${SESSION}"
wait_for 60 sh -c "[ \"\$(loginctl show-session ${SESSION} -p LockedHint --value)\" = no ]"
sleep 5
ame_ desktop windows corner top-left desktop >/dev/null
sleep 1
push=()
for i in $(seq 10); do push+=("rel:-20,-20"); done
keys move:0.5,0.5 sleep:300 move:0.01,0.01 "${push[@]}" sleep:800
if wait_for 3 is_true "${W}.desktop.out"; then ok "a push into the top left corner shows the desktop"; else
    not_ok "a push into the top left corner shows the desktop" \
        "$(ev "JSON.stringify({classic: ${W}.classic, running: [...${W}._running], barriers: ${W}.corners.barrierCount, overview: Main.overview.visible, pointer: global.get_pointer().slice(0, 2)})")"
fi
fired="$(ev "${W}.counters.cornerFired ?? 0")"
keys move:0.3,0.3 sleep:500 move:0.01,0.01 sleep:300 "${push[@]}" sleep:800
check "the same push brings the windows back" wait_for 3 is_false "${W}.desktop.out"
same "and the corner fired once more for it" "$(ev "${W}.counters.cornerFired ?? 0")" "$((fired + 1))"
for where in top-left top-right bottom-left bottom-right; do ame_ desktop windows corner "${where}" none >/dev/null; done
sleep 1
same "every corner back at none, no barrier is left" "$(barriers)" 0

# --- 15.5 Show desktop ----------------------------------------------------------------------------------------------
ame_ desktop windows show-desktop on >/dev/null
sleep 1
# A point of wallpaper no window covers, near a corner of the work area
# Every window against the right edge first, so that the lab's small screen keeps some wallpaper free. The Manual is
# put away for this part: it is never narrower than 1264 px, nearly the whole of the lab's screen
ev "${WIN}.filter(w => w.get_wm_class() === 'org.amethystora.Manual').forEach(w => w.minimize()); 'min'" >/dev/null
ev "global.get_window_actors().map(a => a.meta_window).filter(w => !w.minimized).forEach(w => { const a = ${AREA}; const f = w.get_frame_rect(); w.move_frame(true, a.x + a.width - f.width, a.y); }); 'moved'" >/dev/null
sleep 1
empty="$(ev "(() => { const a = ${AREA}; const pts = []; for (let y = a.y + 20; y < a.y + a.height - 10; y += 40) for (let x = a.x + 20; x < a.x + a.width - 10; x += 40) pts.push([x, y]); const free = pts.find(([x, y]) => !global.get_window_actors().some(w => { const r = w.meta_window.get_frame_rect(); return !w.meta_window.minimized && x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height; })); return free ? (free[0] / global.screen_width).toFixed(5) + ',' + (free[1] / global.screen_height).toFixed(5) : 'none'; })()")"
out() { is_true "${W}.desktop.out"; }
back() { is_false "${W}.desktop.out" && [[ "$(ev "${W}.desktop.layerCount")" == 0 ]]; }
slivers() {
    ev "(() => { const a = ${AREA}; return ${W}.desktop.windows().every(w => { const f = w.frame; const L = f.x + w.tx, T = f.y + w.ty; const vw = Math.min(L + f.width, a.x + a.width) - Math.max(L, a.x); const vh = Math.min(T + f.height, a.y + a.height) - Math.max(T, a.y); return (w.tx !== 0 ? vw : vh) === 28; }); })()"
}
if [[ "${empty}" == none ]]; then
    not_ok "no point of the wallpaper is free of windows" "$(ev "JSON.stringify(global.get_window_actors().map(a => { const w = a.meta_window; const f = w.get_frame_rect(); return [w.get_wm_class(), w.get_window_type(), w.get_workspace()?.index(), w.minimized, [f.x, f.y, f.width, f.height].join(String.fromCharCode(44))]; }))")"
else
    keys "click:${empty}"
    check "a click on empty wallpaper slides the windows out" wait_for 3 out
    sleep 1
    check "each leaves exactly 28 px inside the work area" test "$(slivers)" == true
    check "and the hint says how to bring them back" is_true "${W}.desktop._hint?.visible"
    keys esc
    check "Escape brings them back and takes the layer away" wait_for 3 back
    same "every window at its place" "$(ev "global.get_window_actors().every(a => a.translation_x === 0 && a.translation_y === 0)")" true
    # A double click on the wallpaper: the first brings them back, the second, while they glide home, sends them out
    # again, and never without the layer that keeps clicks from the windows nobody can see
    keys "click:${empty}"
    wait_for 3 out
    keys "tap:${empty}" sleep:120 "tap:${empty}" sleep:600
    guarded() { is_false "${W}.desktop.out" || (($(ev "${W}.desktop.layerCount") > 0)); }
    check "a double click never leaves windows out without the layer" guarded
    if is_true "${W}.desktop.out"; then keys esc; fi
    wait_for 3 back
    # Another window in use before the windows go out: a change of focus while they are out brings them back
    ev "Main.activateWindow(${WIN}.find(w => w.get_wm_class() === 'org.vmlab.Bold')); 'b'" >/dev/null
    keys "click:${empty}"
    wait_for 3 out
    sliver="$(ev "(() => { const w = ${W}.desktop.windows().find(w => w.title && ${WIN}.find(m => m.get_title() === w.title)?.get_wm_class() === 'org.gnome.Nautilus'); if (!w) return 'none'; const f = w.frame; const a = ${AREA}; const L = Math.max(f.x + w.tx, a.x), T = Math.max(f.y + w.ty, a.y), R = Math.min(f.x + w.tx + f.width, a.x + a.width), B = Math.min(f.y + w.ty + f.height, a.y + a.height); return (((L + R) / 2) / global.screen_width).toFixed(5) + ',' + (((T + B) / 2) / global.screen_height).toFixed(5); })()")"
    if [[ "${sliver}" != none ]]; then
        keys "click:${sliver}"
        check "a click on Files' sliver brings every window back" wait_for 3 back
        same "and Files to the front" "$(focused)" org.gnome.Nautilus
    else
        not_ok "Files was not out"
    fi
    keys "click:${empty}"
    wait_for 3 out
    keys super
    check "the overview brings them back" wait_for 3 back
    check "with no layer left in the shell" test "$(ev "Main.layoutManager.uiGroup.get_children().some(c => c.name === 'amethystoraWindowsDesktopLayer')")" == false
    keys esc
    wait_for 5 is_false "Main.overview.visible"
    keys "click:${empty}"
    wait_for 3 out
    open_files
    check "a new window brings them back" wait_for 30 back
    sleep 3
    ev "${WIN}.filter(w => w.get_wm_class() === 'org.gnome.Nautilus').slice(1).forEach(w => w.delete(0)); 'x'" >/dev/null
    sleep 1
    keys "click:${empty}"
    wait_for 3 out
    ame_ desktop windows show-desktop off >/dev/null
    check "show-desktop off brings them back" wait_for 3 back
    tiling
    ame_ desktop windows show-desktop on >/dev/null
    keys "click:${empty}"
    sleep 1
    check "in the tiling layout a wallpaper click does nothing" is_false "${W}.desktop.out"
    classic
fi
ame_ desktop windows show-desktop off >/dev/null

# --- 15.6 Touchpad gestures -----------------------------------------------------------------------------------------
for name in pinch spread down today; do ame_ desktop windows gesture "${name}" off >/dev/null; done
sleep 1
check "with every gesture off, nothing listens to the touchpad" is_false "${W}.gestures.connected"
for name in pinch spread down today; do ame_ desktop windows gesture "${name}" on >/dev/null; done
sleep 1
events="$(ev "${W}.counters.gestureEvent ?? 0")"
touchpad pinch:3,0.9 >/dev/null 2>&1
sleep 1
if (($(ev "${W}.counters.gestureEvent ?? 0") > events)); then
    ok "libinput takes the uinput touchpad as one with gestures"
    keys esc
    wait_for 5 is_false "Main.overview.visible"
    touchpad pinch:3,0.5
    check "a three-finger pinch opens the app grid" wait_for 3 is_true "Main.overview.dash.showAppsButton.checked && Main.overview.visible"
    keys esc esc
    wait_for 5 is_false "Main.overview.visible"
    touchpad pinch:3,0.9
    sleep 1
    check "a pinch not far enough leaves the overview hidden" is_false "Main.overview.visible"
    touchpad pinch:3,1.6
    check "three fingers spread show the desktop" wait_for 3 is_true "${W}.desktop.out"
    touchpad pinch:3,0.5
    check "a pinch brings them back" wait_for 3 is_false "${W}.desktop.out"
    check "and opens no app grid" is_false "Main.overview.visible"
    open_files
    wait_for 30 at_least org.gnome.Nautilus 2
    activate_files
    touchpad swipe:3,0,40
    check "three fingers down fan Files' windows out from the dock" wait_for 3 is_true "${UI}.get('dock').state().fan.open"
    keys esc
    touchpad swipe:4,-40,0
    check "four fingers to the left open Today" wait_for 3 is_true "Main.panel.statusArea.dateMenu.menu.isOpen"
    touchpad swipe:4,40,0
    check "and to the right close it" wait_for 3 is_false "Main.panel.statusArea.dateMenu.menu.isOpen"
    ws="$(ev "global.workspace_manager.get_active_workspace_index()")"
    touchpad swipe:3,40,0
    ws_moved() { [[ "$(ev "global.workspace_manager.get_active_workspace_index()")" != "${ws}" ]]; }
    check "three fingers sideways are still GNOME's workspaces" wait_for 3 ws_moved
    ev "global.workspace_manager.get_workspace_by_index(${ws}).activate(global.get_current_time()); 'w'" >/dev/null
    touchpad swipe:3,0,-40
    check "and three fingers up GNOME's overview" wait_for 3 is_true "Main.overview.visible"
    keys esc
else
    skip "libinput did not take the uinput touchpad: 15.6.2-6 by the recogniser's own entry instead"
    SEQ="const seq = (kind, f, steps) => [{kind, fingers: f, phase: 'begin', dx: 0, dy: 0, scale: 1}, ...steps.map(s => ({kind, fingers: f, phase: 'update', ...s})), {kind, fingers: f, phase: 'end', dx: 0, dy: 0, scale: steps.at(-1).scale ?? 1}]; const pinch = (f, to) => seq('pinch', f, Array.from({length: 10}, (_, i) => ({dx: 0, dy: 0, scale: 1 + (to - 1) * (i + 1) / 10}))); const swipe = (f, dx, dy) => seq('swipe', f, Array.from({length: 20}, () => ({dx: dx / 20, dy: dy / 20})));"
    ev "${SEQ} ${W}.gestures.feed(pinch(3, 0.5)); 'fed'" >/dev/null
    check "a three-finger pinch opens the app grid" wait_for 3 is_true "Main.overview.dash.showAppsButton.checked && Main.overview.visible"
    ev "Main.overview.hide(); 'h'" >/dev/null
    wait_for 5 is_false "Main.overview.visible"
    ev "${SEQ} ${W}.gestures.feed(pinch(3, 0.9)); 'fed'" >/dev/null
    sleep 1
    check "a pinch not far enough leaves the overview hidden" is_false "Main.overview.visible"
    ev "${SEQ} ${W}.gestures.feed(pinch(3, 1.6)); 'fed'" >/dev/null
    check "three fingers spread show the desktop" wait_for 3 is_true "${W}.desktop.out"
    ev "${SEQ} ${W}.gestures.feed(pinch(3, 0.5)); 'fed'" >/dev/null
    check "a pinch brings them back, with no app grid" wait_for 3 is_false "${W}.desktop.out || Main.overview.visible"
    open_files
    wait_for 30 at_least org.gnome.Nautilus 2
    activate_files
    ev "${SEQ} ${W}.gestures.feed(swipe(3, 0, 100)); 'fed'" >/dev/null
    check "three fingers down fan Files' windows out from the dock" wait_for 3 is_true "${UI}.get('dock').state().fan.open"
    keys esc
    ev "${SEQ} ${W}.gestures.feed(swipe(4, -100, 0)); 'fed'" >/dev/null
    check "four fingers to the left open Today" wait_for 3 is_true "Main.panel.statusArea.dateMenu.menu.isOpen"
    ev "${SEQ} ${W}.gestures.feed(swipe(4, 100, 0)); 'fed'" >/dev/null
    check "and to the right close it" wait_for 3 is_false "Main.panel.statusArea.dateMenu.menu.isOpen"
    same "three fingers up and sideways are never taken" \
        "$(ev "${SEQ} [...${W}.gestures.feed(swipe(3, 0, -100)), ...${W}.gestures.feed(swipe(3, 100, 0))].every(r => r === false)")" true
fi
ev "${WIN}.filter(w => w.get_wm_class() === 'org.gnome.Nautilus').slice(1).forEach(w => w.delete(0)); Main.overview.hide(); 'x'" >/dev/null
sleep 2

# --- 15.7 Places ----------------------------------------------------------------------------------------------------
PLACES="${STATE_DIR}/places.json"
ame_ desktop windows places on >/dev/null
ev "$(plain).move_resize_frame(true, 100, 120, 900, 600); 'm'" >/dev/null
sleep 1
echo "# before closing: the plain window at $(rect_of "$(plain)"), desktop out $(ev "${W}.desktop.out"), kept $(ev "${W}.keep.copy !== null"), maximized $(ev "$(plain).get_maximize_flags?.() ?? $(plain).is_maximized()"), places $(ev "(() => { const p = ${W}.places; const w = $(plain); const t = imports.gi.GLib.get_monotonic_time(); return JSON.stringify({grabbed: p._grabbed === w, other: p._grabbed ? p._grabbed.get_title() : null, unplaced: p._unplaced.has(w), quiet: p._quietUntil - t, moved: (p._moved.get(w) ?? 0) - t, kept: ${W}.isKept?.(w), watched: p._watched.has(w), siblings: p._siblings(w, 'org.vmlab.Plain').length}); })()")"
ev "$(plain).delete(0); 'closed'" >/dev/null
check "places.json is written within 3 s" wait_for 4 test -s "${PLACES}"
# The file is there from the steps before: what counts is the place the window had as it closed, which the module
# writes a moment later (once its records settle), so the file is read again until it holds that place, 3 s at most
monitor_xy="$(ev "Main.layoutManager.primaryMonitor.x + ',' + Main.layoutManager.primaryMonitor.y")"
want_place="[\"$(ev "global.backend.get_monitor_manager().get_logical_monitors()[0].get_monitors()[0].get_connector()")\",[$((100 - ${monitor_xy%,*})),$((120 - ${monitor_xy#*,})),900,600]]"
place_now() {
    jq -c '.arrangements | to_entries[0] | [.key, (.value.apps["org.vmlab.Plain"].places[0] | [.x, .y, .w, .h])]' "${PLACES}" 2>/dev/null |
        sed 's/"\([^:]*\):.*",/"\1",/'
}
place_kept() { [[ "$(place_now)" == "${want_place}" ]]; }
wait_for 4 place_kept
same "it holds this arrangement and the window's place" "$(place_now)" "${want_place}"
check "and its copy is the same" cmp -s "${PLACES}" "${PLACES}.bak"
open_plain
wait_for 30 at_least org.vmlab.Plain 1
sleep 2
same "The window opens at its place again, to the pixel" "$(rect_of "$(plain)")" "100,120,900,600"
applied="$(ev "JSON.stringify(${W}.places.lastApplied)")"
# Mutter takes the place given in configure, or (GNOME 51) only the size, and the module moves it as it shows
first="$(jq -r '.shown | join(",")' <<<"${applied}" 2>/dev/null)"
if [[ "${first}" == "100,120,900,600" ]]; then
    ok "and was there in its first frame"
else
    same "shown elsewhere first (${first}), it was moved to its place as it showed" \
        "$(jq -r '.moved // false' <<<"${applied}" 2>/dev/null)" true
fi
ok "the place was applied through $(jq -r .path <<<"${applied}" 2>/dev/null) (15.7.3)"
# Where Mutter put it before the move is nobody's choice, and is never kept as its place
sleep 3
same "the place kept is still the one it was given" \
    "$(jq -c '.arrangements[].apps["org.vmlab.Plain"].places[0] | [.x, .y, .w, .h]' "${PLACES}" 2>/dev/null | head -1)" \
    "[$((100 - ${monitor_xy%,*})),$((120 - ${monitor_xy#*,})),900,600]"
# A second window gets the second place
ev "$(plain).move_resize_frame(true, 300, 200, 700, 500); 'm'" >/dev/null
open_plain
wait_for 30 at_least org.vmlab.Plain 2
sleep 1
ev "${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain').sort((a, b) => b.get_stable_sequence() - a.get_stable_sequence())[0].move_resize_frame(true, 640, 260, 600, 450); 'm'" >/dev/null
sleep 3
ev "${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain').sort((a, b) => b.get_stable_sequence() - a.get_stable_sequence())[0].delete(0); 'c'" >/dev/null
sleep 2
open_plain
wait_for 30 at_least org.vmlab.Plain 2
sleep 2
same "a second window of the app opens at the second place" \
    "$(ev "(() => { const w = ${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain').sort((a, b) => b.get_stable_sequence() - a.get_stable_sequence())[0]; const r = w.get_frame_rect(); return [r.x, r.y, r.width, r.height].join(','); })()")" \
    "640,260,600,450"
ev "${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain').slice(1).forEach(w => w.delete(0)); 'x'" >/dev/null
# 15.7.5 Another arrangement (the screen at another size, through DisplayConfig, temporarily): a window placed there
# is remembered under it, and back at the first size the window returns to its place there
display() {
    user_bus python3 - "$1" <<'PY'
import sys
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
def call(method, args, reply):
    return bus.call_sync("org.gnome.Mutter.DisplayConfig", "/org/gnome/Mutter/DisplayConfig",
                         "org.gnome.Mutter.DisplayConfig", method, args, GLib.VariantType(reply) if reply else None,
                         Gio.DBusCallFlags.NONE, -1, None)
serial, monitors, logical, props = call("GetCurrentState", None, "(ua((ssss)a(siiddada{sv})a{sv})a(iiduba(ssss)a{sv})a{sv})").unpack()
(connector, *_), modes, _ = monitors[0]
current = next(m for m in modes if m[6].get("is-current"))
if sys.argv[1] == "current":
    print(current[0])
    sys.exit(0)
if sys.argv[1] == "smaller":
    smaller = [m for m in modes if m[1] < current[1] and m[2] < current[2]]
    mode = max(smaller, key=lambda m: m[1] * m[2])[0] if smaller else current[0]
else:
    mode = sys.argv[1]
call("ApplyMonitorsConfig", GLib.Variant("(uua(iiduba(ssa{sv}))a{sv})",
     (serial, 1, [(0, 0, 1.0, 0, True, [(connector, mode, {})])], {})), None)
PY
}
first_mode="$(display current)"
ev "${WIN}.filter(w => w.get_wm_class() === 'org.vmlab.Plain').slice(1).forEach(w => w.delete(0)); $(plain).move_resize_frame(true, 120, 140, 700, 450); 'm'" >/dev/null
sleep 3
display smaller
sleep 3
ev "$(plain).move_resize_frame(true, 60, 70, 500, 350); 'm'" >/dev/null
sleep 3
same "the new arrangement is remembered beside the first" "$(jq '.arrangements | length >= 2' "${PLACES}")" true
display "${first_mode}"
back_at_first() { [[ "$(rect_of "$(plain)")" == "120,140,700,450" ]]; }
check "back at the first size, the window returns to its place there within 2 s" wait_for 3 back_at_first
display smaller
again_second() { [[ "$(rect_of "$(plain)")" == "60,70,500,350" ]]; }
check "and at the second, to its place there" wait_for 3 again_second
display "${first_mode}"
sleep 3

# A file of 20 arrangements is cut to 8 at the next save
jq '.arrangements |= (to_entries + [range(20) | {key: "X-\(.):1x1@0,0", value: {used: ., apps: {}}}] | from_entries)' "${PLACES}" >"${PLACES}.many"
cp "${PLACES}.many" "${PLACES}" && cp "${PLACES}.many" "${PLACES}.bak" && rm -f "${PLACES}.many"
chown "${TEST_USER}:" "${PLACES}" "${PLACES}.bak"
# The module read its file at enable: a disable and enable reads the planted one
ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
sleep 2
ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
sleep 5
ev "$(plain).move_resize_frame(true, 110, 130, 900, 600); 'm'" >/dev/null
sleep 4
same "after the next save there are 8 arrangements" "$(jq '.arrangements | length' "${PLACES}")" 8
# A broken file: the copy stands in, and the file is written again
echo '{nope' >"${PLACES}"
ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
sleep 2
echo '{nope' >"${PLACES}"
ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
sleep 5
check "a broken places.json is read from its copy and written again" jq -e '.version == 1' "${PLACES}"
echo '{nope' >"${PLACES}"
echo '{nope' >"${PLACES}.bak"
ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
sleep 2
echo '{nope' >"${PLACES}"
echo '{nope' >"${PLACES}.bak"
ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
sleep 5
check "with both broken the module still runs" is_true "${UI}.has('windows') && ${W}.running('places')"
tiling
ev "$(plain).move_resize_frame(true, 150, 150, 800, 500); 'm'" >/dev/null 2>&1
mtime="$(stat -c %Y "${PLACES}" 2>/dev/null || echo none)"
sleep 4
same "in the tiling layout nothing is saved" "$(stat -c %Y "${PLACES}" 2>/dev/null || echo none)" "${mtime}"
classic
ame_ desktop windows places forget >/dev/null
check "places forget takes both files away" test ! -e "${PLACES}" -a ! -e "${PLACES}.bak"
same "and places is on again" "$(gs get "${SCHEMA}" places)" true

# --- 15.8 Keep a copy in view ---------------------------------------------------------------------------------------
ev "${WIN}.filter(w => w.get_wm_class() === 'org.gnome.Nautilus').slice(1).forEach(w => w.delete(0)); 'x'" >/dev/null
activate_files
item="$(ev "(() => { const w = $(files); Main.wm._windowMenuManager.showWindowMenuForWindow(w, imports.gi.Meta.WindowMenuType.WM, w.get_frame_rect()); const m = Main.wm._windowMenuManager._manager.activeMenu; const it = m?._getMenuItems().find(i => i.label?.text === 'Keep a copy in view'); if (!it) return 'none'; it.activate(imports.gi.Clutter.get_current_event()); return 'chosen'; })()")"
same "the window menu has Keep a copy in view" "${item}" chosen
kept() { is_true "$(files).minimized && ${W}.keep.copy instanceof imports.gi.Clutter.Clone && ${W}.keep.copy.source === $(files).get_compositor_private() && ${W}.keep.copy.mapped"; }
check "choosing it puts Files out of sight and a live copy of it in view" wait_for 3 kept
keys super+2
sleep 1
check "the copy is on workspace 2" is_true "${W}.keep.copy.mapped && Main.layoutManager.uiGroup.contains(${W}.keep.frame)"
keys super+3
sleep 1
check "and on workspace 3" is_true "${W}.keep.copy.mapped"
keys super+1
sleep 1
corner_gap() {
    ev "(() => { const e = ${W}.keep.frame.get_transformed_extents(); const a = ${AREA}; return [Math.round(e.size.width), Math.round(a.x + a.width - e.origin.x - e.size.width), Math.round(e.origin.y - a.y), Math.round(e.origin.x - a.x), Math.round(a.y + a.height - e.origin.y - e.size.height)].join(','); })()"
}
gap="$(corner_gap)"
check "it is 320 wide (or its clamp), 14 px from the top and right of the work area" \
    test "$(cut -d, -f2,3 <<<"${gap}")" == "14,14"
centre="$(ev "(() => { const e = ${W}.keep.frame.get_transformed_extents(); return ((e.origin.x + e.size.width / 2) / global.screen_width).toFixed(5) + ',' + ((e.origin.y + e.size.height / 2) / global.screen_height).toFixed(5); })()")"
keys "drag:${centre},0.2,0.8"
sleep 1.5
gap="$(corner_gap)"
check "dragged to the bottom left, it settles 14 px from those edges" test "$(cut -d, -f4,5 <<<"${gap}")" == "14,14"
centre="$(ev "(() => { const e = ${W}.keep.frame.get_transformed_extents(); return ((e.origin.x + e.size.width / 2) / global.screen_width).toFixed(5) + ',' + ((e.origin.y + e.size.height / 2) / global.screen_height).toFixed(5); })()")"
keys "click:${centre}"
check "a click brings Files back with the keyboard, and the copy goes" wait_for 3 is_true "!$(files).minimized && !${W}.keep.copy && global.display.focus_window === $(files)"
ev "${W}.keep.keep($(files)); 'k'" >/dev/null
sleep 1
ev "$(files).delete(0); 'c'" >/dev/null
check "closing the window takes the copy away" wait_for 3 is_true "!${W}.keep.copy"
open_files
wait_for 30 at_least org.gnome.Nautilus 1
sleep 2
ev "${W}.keep.keep($(files)); 'k'" >/dev/null
sleep 1
loginctl lock-session "${SESSION}"
wait_for 20 locked
loginctl unlock-session "${SESSION}"
wait_for 60 sh -c "[ \"\$(loginctl show-session ${SESSION} -p LockedHint --value)\" = no ]"
sleep 5
check "across a screen lock the copy comes back" wait_for 10 kept
ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
sleep 2
check "disabling Amethystora UI gives the window back" is_false "$(files).minimized"
ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
sleep 5

# --- 15.9 Dimming ---------------------------------------------------------------------------------------------------
dims() { ev "global.get_window_actors().filter(a => a.get_effect('amethystora-dim')).length"; }
same "with dim off, no window carries the effect" "$(dims)" 0
ame_ desktop windows dim on >/dev/null
ev "Main.activateWindow(${WIN}.find(w => w.get_wm_class() === 'org.amethystora.Manual')); $(files).move_resize_frame(true, 40, 80, 700, 500); 'f'" >/dev/null
strength() { ev "($1).get_compositor_private().get_effect('amethystora-dim')?.strength ?? 0"; }
MANUAL="${WIN}.find(w => w.get_wm_class() === 'org.amethystora.Manual')"
strength_is() { [[ "$(strength "$1")" == "$2" ]]; }
check "Files, not in use, is dimmed" wait_for 3 strength_is "$(files)" 1
check "the Manual, in use, is not" wait_for 3 strength_is "${MANUAL}" 0
activate_files
check "using Files swaps them" wait_for 3 strength_is "${MANUAL}" 1
# Two thirds of the brightness, in pixels: one spot of the Manual's window, with dim off and on. pixel NAME POINT,
# POINT a JavaScript expression giving [x, y] on the stage (the window's middle when left out)
# The plain window, whose middle is one colour: bottom left, clear of every other window, and not in use. The Manual
# is put away first: never narrower than 1264 px, it would cover the plain window wherever it went (and a window wholly
# behind another is rightly not dimmed)
ev "${WIN}.filter(w => w.get_wm_class() === 'org.amethystora.Manual').forEach(w => w.minimize()); 'min'" >/dev/null
PLAIN="$(plain)"
ev "(() => { const a = ${AREA}; global.get_window_actors().map(a => a.meta_window).filter(w => !w.minimized && w.get_wm_class() !== 'org.vmlab.Plain').forEach(w => { const f = w.get_frame_rect(); w.move_frame(true, a.x + a.width - f.width, a.y); }); ${PLAIN}.move_resize_frame(true, a.x, a.y + a.height - 200, 300, 200); return 'placed'; })()" >/dev/null
activate_files
check "the plain window, not in use, is dimmed" wait_for 3 strength_is "${PLAIN}" 1
MIDDLE="(() => { const r = ${PLAIN}.get_frame_rect(); return [r.x + Math.round(r.width / 2), r.y + Math.round(r.height / 2) + 20]; })()"
pixel() {
    local file="/tmp/vmlab-dim-$1.png" point="${2:-${MIDDLE}}"
    ev "(() => { const [x, y] = ${point}; const s = new imports.gi.Shell.Screenshot(); const f = imports.gi.Gio.File.new_for_path('${file}'); const o = f.replace(null, false, 0, null); s.screenshot_area(Math.round(x), Math.round(y), 4, 4, o, () => o.close(null)); return 'shot'; })()" >/dev/null
    sleep 2
    python3 - "${file}" <<'PY'
import struct, sys, zlib
data = open(sys.argv[1], "rb").read()
pos, idat, width, height, kind = 8, b"", 0, 0, 0
while pos < len(data):
    length, tag = struct.unpack(">I4s", data[pos:pos + 8])
    chunk = data[pos + 8:pos + 8 + length]
    if tag == b"IHDR":
        width, height, _, kind = struct.unpack(">IIBB", chunk[:10])
    elif tag == b"IDAT":
        idat += chunk
    pos += 12 + length
raw = zlib.decompress(idat)
bpp = 4 if kind == 6 else 3
row = raw[1:1 + width * bpp] if raw[0] == 0 else None
# The first pixel of the first row is enough when the row is unfiltered; otherwise undo the Sub and Up filters
if row is None:
    stride = width * bpp
    line = bytearray(raw[1:1 + stride])
    # The first row has no row above it: Up adds nothing, Average half the pixel to the left, Paeth that pixel
    for i in range(stride):
        left = line[i - bpp] if i >= bpp else 0
        if raw[0] in (1, 4):
            line[i] = (line[i] + left) & 255
        elif raw[0] == 3:
            line[i] = (line[i] + left // 2) & 255
    row = bytes(line)
print(sum(row[:3]) / 3)
PY
}
light="$(ame_ desktop windows dim off >/dev/null; sleep 1; pixel off)"
ame_ desktop windows dim on >/dev/null
sleep 1
dark="$(pixel on)"
# Dim darkens the window's own picture to 0.66; an unfocused window is drawn over the glass, and where its pane is
# not quite opaque the glass that shows through stays as bright (0.71 measured in the lab)
if [[ "$(gnome-shell --version | grep -oE '[0-9]+' | head -n 1)" -ge 51 && "${dark}" == "${light}" ]]; then
    # GNOME 51's area screenshot draws the windows without the effects on them (the screen itself shows the window
    # dimmed, seen in the lab's own screen dump on 2026-11-01): the strength above is what is held there
    skip "a dimmed window's pixels: GNOME 51's screenshot leaves the dim out (${dark} of ${light})"
else
    check "a dimmed window's pixels are between 0.63 and 0.75 of their brightness (${dark} of ${light})" python3 -c \
        "import sys; r = ${dark:-0} / max(${light:-1}, 1); sys.exit(0 if 0.63 <= r <= 0.75 else 1)"
fi
# In the overview, the Manual's picture (a clone of its window) is as bright with dim on as with dim off
PREVIEW="(() => { const find = a => { if (a.metaWindow === ${PLAIN}) return a; for (const c of a.get_children()) { const f = find(c); if (f) return f; } return null; }; const p = find(Main.layoutManager.overviewGroup); const e = p.get_transformed_extents(); return [e.origin.x + e.size.width / 2, e.origin.y + e.size.height / 2]; })()"
ev "Main.overview.show(); 'o'" >/dev/null
sleep 3
# The window itself is dimmed meanwhile, so that only its picture can be the undimmed one
check "with the overview open, the plain window is still dimmed" strength_is "${PLAIN}" 1
clone_dim="$(pixel clone-on "${PREVIEW}")"
ev "Main.overview.hide(); 'h'" >/dev/null
sleep 2
ame_ desktop windows dim off >/dev/null
sleep 1
ev "Main.overview.show(); 'o'" >/dev/null
sleep 3
clone_plain="$(pixel clone-off "${PREVIEW}")"
ev "Main.overview.hide(); 'h'" >/dev/null
sleep 2
check "in the overview, the window's picture is not dimmed (within 0.03 of undimmed)" python3 -c     "import sys; r = ${clone_dim:-0} / max(${clone_plain:-1}, 1); sys.exit(0 if abs(r - 1) <= 0.03 else 1)"
ame_ desktop windows dim on >/dev/null
ame_ desktop windows dim off >/dev/null
no_dims() { [[ "$(dims)" == 0 ]]; }
check "dim off takes every effect away" wait_for 3 no_dims

# --- 15.10 Cost and cleanliness -------------------------------------------------------------------------------------
c1="$(ev "JSON.stringify(Object.fromEntries(Object.entries(${W}.counters).filter(([k]) => k !== 'hold.keymapEvents')))")"
sleep 30
c2="$(ev "JSON.stringify(Object.fromEntries(Object.entries(${W}.counters).filter(([k]) => k !== 'hold.keymapEvents')))")"
same "with nothing happening for 30 s, no handler of the module runs" "${c2}" "${c1}"
# The class as GNOME made it, to compare with after the cycles (an import the Eval takes only as an expression)
ev "(import('resource:///org/gnome/shell/ui/windowMenu.js')).then(m => { globalThis.vmlabWindowMenu = m.WindowMenu; }); 'k'" >/dev/null
# GNOME 50 turns an extension on and off through its settings, a moment later, and later still in a busy lab: each
# step waits for the state it asked for (1 on; 2 or 6 off) rather than a fixed time
ext_state_is() { [[ " $2 " == *" $(ev "Main.extensionManager.lookup('${UUID}').state") "* ]]; }
for i in $(seq 10); do
    ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
    wait_for 30 ext_state_is "${UUID}" "2 6"
    ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
    wait_for 30 ext_state_is "${UUID}" "1"
done
ev "Main.extensionManager.disableExtension('${UUID}'); 'd'" >/dev/null
wait_for 30 ext_state_is "${UUID}" "2 6"
sleep 1
left="$(ev "(() => { const walk = (a, acc) => { for (const c of a.get_children()) { const s = c.get_style_class_name?.() || ''; if (/amethystora-windows/.test(s) || /amethystoraWindows/.test(c.name || '')) acc.push(s || c.name); walk(c, acc); } return acc; }; return walk(Main.layoutManager.uiGroup, []).length + walk(global.window_group, []).length; })()")"
same "ten disables leave no actor of the module behind" "${left}" 0
check "Main.overview.toggle is GNOME's own" is_true "!Object.getOwnPropertyDescriptor(Main.overview, 'toggle')"
same "the window menu is GNOME's own (the class wrapped, the instance wrapped)" \
    "$(ev "[globalThis.vmlabWindowMenu.prototype._buildMenu.toString().includes('_addItem'), Object.hasOwn(Main.wm._windowMenuManager, 'showWindowMenuForWindow')].join(' ')")" "false false"
check "Super+Z is unbound while it is off" test "$(ev "Main.wm._allowedKeybindings['snap-chooser'] ?? 0")" == 0
ev "Main.extensionManager.enableExtension('${UUID}'); 'e'" >/dev/null
sleep 5
ev "${W}.timings.maxFrameUs = 0; ${W}.desktop.toggle(); 'out'" >/dev/null
sleep 1
ev "${W}.desktop.toggle(); 'back'" >/dev/null
sleep 1
desktop_us="$(ev "${W}.timings.maxFrameUs")"
# The grid is built as it opens, which is no frame of motion: the selection is timed from then on
ev "${W}.switcher.open('switch-windows'); ${W}.timings.maxFrameUs = 0; 'x'" >/dev/null
keys right right left esc
switcher_us="$(ev "${W}.timings.maxFrameUs")"
check "the show-desktop timeline stays under 1 ms a frame (${desktop_us} µs)" test "${desktop_us:-99999}" -lt 1000
check "the switcher's selection stays under 1 ms (${switcher_us} µs)" test "${switcher_us:-99999}" -lt 1000

# Back as it was
[[ "${was_tiling}" == true ]] && tiling
ev "global.force_animations = true; 'unforced'" >/dev/null
user_systemctl stop "vmlab-bold-$$" "vmlab-manual-$$" 2>/dev/null
ev "${WIN}.filter(w => w.get_wm_class() === 'org.gnome.Nautilus').forEach(w => w.delete(0)); 'x'" >/dev/null

errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    awk -v uuid="${UUID}" '/JS ERROR/ { block = $0; next } block && /@(resource|file):\/\// { block = block "\n" $0; next }
        block { if (index(block, uuid)) print block; block = "" } /Amethystora UI: .*(failed|threw|did not)/ { print }
        END { if (block && index(block, uuid)) print block }' | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of Amethystora UI threw"; else not_ok "Amethystora UI threw" "${errors}"; fi
finish
