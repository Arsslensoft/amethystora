#!/usr/bin/bash
# vmlab-vm: saved sets
# Saved workspace sets (plans/beyond/spec-phase7.md, part A, suite 31), in the test account's session, asked of GNOME
# Shell itself through Eval, which a lab-only extension of the test account opens (never in the image), and through
# the workspace menu as a person uses it (keys.py's right click, atspi.py):
#   - ame desktop sets takes Sets out of the menu and puts it back, at once;
#   - Save this workspace keeps the apps and their places in sets.json and its copy, 0600 in a 0700 folder, and no
#     window's title; the row says the set's name and apps;
#   - opening a set puts each window back at its place, on the workspace you are on when empty, else the next empty
#     one, names an unnamed workspace, uses the windows of its apps already there, brings over a one-window app's
#     window, leaves out an app that is gone, says when an app cannot start, gives up after 30 s, and places steps
#     aside for it;
#   - under the tiling the strip comes back in its order and widths, also when the person switched away meanwhile,
#     with two windows of one app around another, and from a set saved in the classic layout;
#   - a window of a set rises into its place (animations forced on: the lab draws without a GPU), and only fades in
#     with Reduce motion;
#   - twelve sets at most, sixteen windows a set, broken files and their copy, the cross that forgets, nothing
#     connected between openings, ten disable/enable cycles (one in the middle of an opening), the setup file;
#   - nothing of Amethystora UI throws meanwhile.
# Not covered here: several screens (a set's places name screens by connector, which one lab screen cannot vary).
# It leaves the account as it found it, however it ends: every key reset, sets.json as it was, its windows closed.
# vmlab-paths: @ui/extension.js @ui/*sets* @ui/*workspaces* @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HOME_DIR="/var/home/${TEST_USER}"
STATE="${HOME_DIR}/.local/state/amethystora"
APPS="${HOME_DIR}/.local/share/applications"
WORKSPACES=org.gnome.shell.extensions.amethystora-ui.workspaces
WORK=/run/vmlab-31-spaces
HERE="$(dirname "$0")"
keys() { python3 "${HERE}/keys.py" "$@"; }
atspi() { user_bus python3 "${HERE}/atspi.py" gnome-shell "$@"; }
# Eval's answer is JSON.stringify of the value, which gdbus prints as a GVariant string: decoded once here, a string
# printed as itself and anything else as compact JSON (sed left the quotes of a JSON value escaped). A throw is printed
# as gdbus gave it.
EV_DECODE='
import ast, json, re, sys
text = sys.stdin.read().strip()
answer = re.fullmatch(r"\((true|false), (.*)\)", text, re.S)
if not answer or answer.group(1) == "false":
    print(text)
    sys.exit(0)
value = ast.literal_eval(answer.group(2))
try:
    value = json.loads(value)
except ValueError:
    pass
print(value if isinstance(value, str) else json.dumps(value, separators=(",", ":"), ensure_ascii=False))'
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -I -c "${EV_DECODE}"
}
# Each argument quoted for the shell in_session starts: a value such as "['Writing']" lost its quotes there, and
# gsettings refused it without a word
gs() { in_session "gsettings $(printf '%q ' "$@")" 2>/dev/null; }
ame_() { in_session "ame $(printf '%q ' "$@")" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
is_true() { [[ "$(ev "$1")" == true ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
WS="${UI}.get('workspaces')"
SETS="${WS}.sets"
# The windows that count, as [class, workspace, x, y, width, height]
WINS="global.display.list_all_windows().filter(w => w.get_window_type() === 0)"
module_up() { [[ "$(ev "typeof ${WS}?.claim")" == function ]]; }
# Amethystora UI off and on again, one step at a time: GNOME enables it again from its enabled-extensions setting, and an
# enable sent right behind the disable could leave it off for the rest of the suite (seen on GNOME 50.6)
reload_ui() {
    ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
    wait_for 10 is_true "Main.extensionManager.lookup('${UUID}').state !== 1"
    ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
    wait_for 30 module_up
}
opening_over() { [[ "$(ev "JSON.stringify(${SETS}?.snapshot().opening ?? null)")" == null ]]; }
count_class() { ev "${WINS}.filter(w => w.get_wm_class() === '$1').length"; }
close_all() {
    ev "for (const w of ${WINS}) w.delete(global.get_current_time()); 1" >/dev/null
    for _ in $(seq 20); do
        [[ "$(ev "${WINS}.length")" == 0 ]] && break
        sleep 1
    done
}
go_to() { ev "global.workspace_manager.get_workspace_by_index($1).activate(global.get_current_time()); 1" >/dev/null; sleep 1; }
names() { gs get org.gnome.desktop.wm.preferences workspace-names; }
active() { ev "global.workspace_manager.get_active_workspace_index()"; }
# A frame as [x, y, width, height] relative to its screen
frames() {
    ev "JSON.stringify(${WINS}.filter(w => w.get_wm_class() === '$1').map(w => { const f = w.get_frame_rect();
        const g = Main.layoutManager.monitors[w.get_monitor()]; return [w.get_workspace()?.index() ?? -1, f.x - g.x,
        f.y - g.y, f.width, f.height]; }))"
}
near() { # A B: two [ws, x, y, w, h] lists within 2 px
    python3 -c "import json,sys; a=json.loads(sys.argv[1]); b=json.loads(sys.argv[2]);
sys.exit(0 if len(a)==len(b) and all(p[0]==q[0] and all(abs(x-y)<=2 for x,y in zip(p[1:],q[1:])) for p,q in zip(a,b)) else 1)" "$1" "$2"
}
open_menu() {
    local where
    where="$(ev "(() => { const l = ${WS}._labels[${1:-0}]; const [x, y] = l.get_transformed_position(); const [w, h] = l.get_transformed_size();
        return ((x + w / 2) / global.stage.width).toFixed(4) + ',' + ((y + h / 2) / global.stage.height).toFixed(4); })()")"
    keys "right:${where}" >/dev/null 2>&1
    sleep 1
}
close_menu() { ev "${WS}._menu?.close(); 1" >/dev/null; sleep 0.5; }
start() { # UNIT COMMAND...
    local unit="$1"
    shift
    user_bus systemd-run --user --collect --unit="vmlab-31-${unit}-$$-${RANDOM}" "$@" >/dev/null
}
three_up() {
    [[ "$(count_class org.gnome.TextEditor)" -ge 1 && "$(count_class org.gnome.Nautilus)" -ge 1 && "$(count_class kitty)" -ge 1 ]]
}
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "${HERE}/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi

# The lab's Eval (as the motion's suite has it)
if [[ "$(ev 1)" != 1 ]]; then
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
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session
fi
wait_for 120 module_up || { not_ok "Amethystora UI's workspaces are not running"; finish; }

# Stand-ins: a one-window app, an app whose window comes after 35 s, one that cannot start, one with 17 windows
install -d -m 0755 "${WORK}"
cat >"${WORK}/one.js" <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: 'org.vmlab.One'});
app.connect('activate', () => (app.get_active_window() ?? new Gtk.ApplicationWindow({application: app, title: 'One',
    default_width: 420, default_height: 300})).present());
app.run([]);
EOF
cat >"${WORK}/slow.js" <<'EOF'
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: 'org.vmlab.Slow'});
app.connect('activate', () => {
    app.hold();
    GLib.timeout_add_seconds(0, 35, () => {
        new Gtk.ApplicationWindow({application: app, title: 'Slow', default_width: 400, default_height: 300}).present();
        app.release();
        return false;
    });
});
app.run([]);
EOF
cat >"${WORK}/many.js" <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: 'org.vmlab.Many'});
app.connect('activate', () => {
    for (let i = 0; i < 17; i++)
        new Gtk.ApplicationWindow({application: app, title: `Many ${i}`, default_width: 300, default_height: 200}).present();
});
app.run([]);
EOF
cat >"${WORK}/stand.js" <<'EOF'
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
Gtk.init();
const loop = new GLib.MainLoop(null, false);
const win = new Gtk.Window({title: `vmlab-31-stand-${ARGV[0] ?? ''}`, default_width: 300, default_height: 200});
win.connect('close-request', () => loop.quit());
win.present();
loop.run();
EOF
printf '#!/bin/sh\nexit 0\n' >"${WORK}/broken"
chmod 0755 "${WORK}/broken"
chmod 0644 "${WORK}"/*.js
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${APPS}"
desktop() { # ID NAME EXEC [EXTRA]
    printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s\nIcon=application-x-executable\nStartupWMClass=%s\n%s' \
        "$2" "$3" "${1%.desktop}" "${4:-}" >"${APPS}/$1"
    chown "${TEST_USER}:" "${APPS}/$1"
}
desktop org.vmlab.One.desktop One "gjs -m ${WORK}/one.js" 'SingleMainWindow=true'
desktop org.vmlab.Slow.desktop Slow "gjs -m ${WORK}/slow.js"
desktop org.vmlab.Many.desktop Many "gjs -m ${WORK}/many.js"
desktop vmlab-broken.desktop Broken "${WORK}/broken"
known() { [[ "$(ev "['org.vmlab.One.desktop', 'org.vmlab.Slow.desktop', 'org.vmlab.Many.desktop', 'vmlab-broken.desktop']
    .every(id => !!Shell.AppSystem.get_default().lookup_app(id))")" == true ]]; }
wait_for 30 known || not_ok "the stand-in apps did not reach GNOME's list of apps"

# Put back whatever the suite changed, however it ends
cp -a "${STATE}/sets.json" "${WORK}/sets.json.before" 2>/dev/null
# The account's own values, as gsettings prints them, put back as they were
KEPT=("${WORKSPACES} sets" "org.gnome.desktop.wm.preferences workspace-names"
    "org.gnome.desktop.a11y.interface reduced-motion" "org.gnome.shell.extensions.amethystora-ui.windows places")
declare -A WAS
for key in "${KEPT[@]}"; do
    WAS["${key}"]="$(gs get ${key})"
done
LAYOUT_WAS="$([[ "$(gs get org.gnome.shell.extensions.amethystora-ui.tiling enabled)" == false ]] && echo classic || echo tiling)"
cleanup() {
    ev "global.force_animations = true; 1" >/dev/null 2>&1
    close_all
    local key
    for key in "${KEPT[@]}"; do
        [[ -n "${WAS[${key}]}" ]] && in_session "gsettings set ${key} $(printf '%q' "${WAS[${key}]}")" >/dev/null 2>&1
    done
    ame_ desktop layout "${LAYOUT_WAS}" >/dev/null
    rm -f "${APPS}"/org.vmlab.{One,Slow,Many}.desktop "${APPS}/vmlab-broken.desktop"
    if [[ -e "${WORK}/sets.json.before" ]]; then
        install -m 0600 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}/sets.json.before" "${STATE}/sets.json"
        install -m 0600 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}/sets.json.before" "${STATE}/sets.json.bak"
    else
        rm -f "${STATE}/sets.json" "${STATE}/sets.json.bak"
    fi
    reload_ui
    user_bus pkill -f "${WORK}/" 2>/dev/null
}
trap cleanup EXIT
fresh_sets() { # The file empty and the module reading it again
    rm -f "${STATE}/sets.json" "${STATE}/sets.json.bak"
    reload_ui
    wait_for 30 module_up
}
fresh_sets
close_all
gs reset org.gnome.desktop.wm.preferences workspace-names
# A session started again by the suite before (a restart of GDM) opens in the overview, whose grab takes the menus
ev "Main.overview.hide(); 'hidden'" >/dev/null

# --- 1 The switch ---------------------------------------------------------------------------------------------------
same "1 sets is on as the image ships" "$(gs get "${WORKSPACES}" sets)" true
ame_ desktop sets off >/dev/null
same "1 ame desktop sets off turns it off" "$(gs get "${WORKSPACES}" sets)" false
open_menu
if atspi has "Save this workspace" >/dev/null 2>&1; then not_ok "1 the menu has no Save this workspace while off"; else ok "1 the menu has no Save this workspace while off"; fi
# Picking up after a restart (resume, on by default) keeps the sets object alive: with it off too, nothing is held
gs set "${WORKSPACES}" resume false
sleep 1
check "1 nothing of the sets is held while off" is_true "${WS}.sets === null && ${WS}._setsMenu === null"
gs set "${WORKSPACES}" resume true
close_menu
ame_ desktop sets on >/dev/null
open_menu
check "1 ame desktop sets on puts it back without logging out" atspi wait 5 "Nothing to save here"
close_menu

# --- 2 Saving, in the classic layout ---------------------------------------------------------------------------------
ame_ desktop layout classic >/dev/null
sleep 2
go_to 0
start editor flatpak run org.gnome.TextEditor --new-window
start files nautilus --new-window
start kitty kitty
check "2 Text Editor, Files and kitty are open" wait_for 60 three_up
sleep 3
ev "(() => { const a = Main.layoutManager.getWorkAreaForMonitor(0); const r = {
        'org.gnome.TextEditor': [0.03, 0.04, 0.42, 0.55], 'org.gnome.Nautilus': [0.5, 0.05, 0.4, 0.45],
        kitty: [0.2, 0.45, 0.45, 0.45]};
    for (const w of ${WINS}) { const p = r[w.get_wm_class()]; if (!p) continue;
        if (w.get_maximized?.() || w.maximized_horizontally) w.unmaximize();
        w.move_resize_frame(false, Math.round(a.x + p[0] * a.width), Math.round(a.y + p[1] * a.height),
            Math.round(p[2] * a.width), Math.round(p[3] * a.height)); } return 1; })()" >/dev/null
sleep 2
SAVED_EDITOR="$(frames org.gnome.TextEditor)"
SAVED_FILES="$(frames org.gnome.Nautilus)"
SAVED_KITTY="$(frames kitty)"
TITLES="$(ev "JSON.stringify(${WINS}.map(w => w.get_title()).filter(t => t && t.length > 2))")"
gs set org.gnome.desktop.wm.preferences workspace-names "['Writing']"
open_menu
check "2 Save this workspace is offered" atspi wait 5 "Save this workspace"
clicked="$(atspi click "Save this workspace" 2>&1)"
check "2 the button then reads Saved as Writing" atspi wait 5 "Saved as Writing"
echo "# after the click (${clicked:-no answer}): $(ev "JSON.stringify([${WS}._setsMenu?._saveLabel?.text, ${WS}.sets?.list?.length, global.workspace_manager.get_active_workspace().index(), ${WS}._setsMenu?._workspace?.index?.()])")"
sleep 1.5
check "2 sets.json and its copy, 0600, in a 0700 folder" bash -c "[[ \$(stat -c %a '${STATE}/sets.json') == 600 &&
    \$(stat -c %a '${STATE}/sets.json.bak') == 600 && \$(stat -c %a '${STATE}') == 700 ]]"
same "2 the file holds Writing: three windows, classic" \
    "$(jq -r '[(.sets | length), .sets[0].name, (.sets[0].windows | length), .sets[0].layout] | join(" ")' "${STATE}/sets.json")" \
    "1 Writing 3 classic"
check "2 its copy reads the same" cmp "${STATE}/sets.json" "${STATE}/sets.json.bak"
leaked="$(python3 -c "import json,sys; t=json.loads(sys.argv[1]); s=open(sys.argv[2]).read(); print([x for x in t if x in s])" "${TITLES}" "${STATE}/sets.json")"
same "2 no window's title is in the file" "${leaked}" "[]"

# --- 3 The row ------------------------------------------------------------------------------------------------------
check "3 the Sets heading" atspi has "Sets"
check "3 a row that opens Writing" atspi has "Open the set Writing"
ROW="${WS}._setsMenu._rows.get_children()[0]"
same "3 its description names the three apps in stacking order" "$(ev "${ROW}.get_children()[0].accessible_description")" \
    "$(ev "new Intl.ListFormat(undefined, {style: 'short', type: 'unit'}).format(${SETS}.windowsOf(global.workspace_manager.get_workspace_by_index(0)).map(w => Shell.WindowTracker.get_default().get_window_app(w).get_name()))")"
same "3 three icons" "$(ev "${ROW}.get_children()[0].get_child().get_children()[0].get_n_children()")" 3
close_menu

# --- 4 Opening puts each window back --------------------------------------------------------------------------------
close_all
open_menu
atspi click "Open the set Writing" >/dev/null 2>&1
check "4 the three windows are back within 20 s" wait_for 20 three_up
wait_for 20 opening_over
sleep 1
check "4 Text Editor at its place on workspace 1" near "$(frames org.gnome.TextEditor)" "${SAVED_EDITOR}"
check "4 Files at its place" near "$(frames org.gnome.Nautilus)" "${SAVED_FILES}"
check "4 kitty at its place" near "$(frames kitty)" "${SAVED_KITTY}"
check "4 the opening holds nothing once every window is placed" is_true \
    "${SETS}.snapshot().opening === null && ${SETS}.connections === 0"

# --- 5a The next empty workspace, 6 its name ------------------------------------------------------------------------
close_all
start editor2 flatpak run org.gnome.TextEditor --new-window
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.gnome.TextEditor')"
ev "${WINS}.forEach(w => w.change_workspace_by_index(1, false)); 1" >/dev/null
go_to 1
BEFORE="$(frames org.gnome.TextEditor)"
editors="$(count_class org.gnome.TextEditor)"
gs set org.gnome.desktop.wm.preferences workspace-names "['Writing']"
ev "${WS}.openSet('Writing'); 1" >/dev/null
wait_for 30 opening_over
sleep 1
same "5a GNOME went to the next empty workspace (3)" "$(active)" 2
same "5a the Text Editor window on 2 did not move" "$(frames org.gnome.TextEditor | python3 -c "import json,sys; print(json.dumps([f for f in json.load(sys.stdin) if f[0]==1], separators=(',', ':')))")" "${BEFORE}"
same "5a one Text Editor window more, on 3" "$(count_class org.gnome.TextEditor)" "$((editors + 1))"
check "5a Files and kitty on 3" is_true "${WINS}.filter(w => ['org.gnome.Nautilus', 'kitty'].includes(w.get_wm_class())).every(w => w.get_workspace().index() === 2)"
same "6 the unnamed workspace took the set's name" "$(names)" "['Writing', '', 'Writing']"

# --- 5b Windows already there are used; 6 a named workspace keeps its name ------------------------------------------
close_all
for n in 1 2 3 4 5; do
    start "stand${n}" gjs -m "${WORK}/stand.js" "${n}"
done
stands_up() { [[ "$(ev "${WINS}.filter(w => w.get_title()?.startsWith('vmlab-31-stand-')).length")" == 5 ]]; }
wait_for 30 stands_up
ev "${WINS}.filter(w => w.get_title()?.startsWith('vmlab-31-stand-')).forEach(w =>
    w.change_workspace_by_index(Number(w.get_title().slice(-1)), false)); 1" >/dev/null
start files2 nautilus --new-window
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.gnome.Nautilus')"
ev "${WINS}.filter(w => w.get_wm_class() === 'org.gnome.Nautilus').forEach(w => { w.change_workspace_by_index(0, false);
    w.move_resize_frame(false, 300, 300, 500, 400); }); 1" >/dev/null
go_to 0
gs set org.gnome.desktop.wm.preferences workspace-names "['Desk']"
files="$(count_class org.gnome.Nautilus)"
ev "${WS}.openSet('Writing'); 1" >/dev/null
wait_for 40 opening_over
sleep 1
same "5b no workspace was empty: the set opened where you are" "$(active)" 0
check "5b the Files window went to its saved place" near "$(frames org.gnome.Nautilus)" "${SAVED_FILES}"
same "5b and no second Files window was started" "$(count_class org.gnome.Nautilus)" "${files}"
check "5b Text Editor and kitty were started on 1" is_true \
    "${WINS}.filter(w => ['org.gnome.TextEditor', 'kitty'].includes(w.get_wm_class())).map(w => w.get_workspace().index()).join() === '0,0'"
same "6 a workspace with a name keeps it" "$(names)" "['Desk']"

# --- 7 A one-window app is brought over -----------------------------------------------------------------------------
close_all
gs reset org.gnome.desktop.wm.preferences workspace-names
go_to 3
start one gjs -m "${WORK}/one.js"
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.vmlab.One')"
gs set org.gnome.desktop.wm.preferences workspace-names "['', '', '', 'One']"
ev "${SETS}.save(global.workspace_manager.get_workspace_by_index(3)); 1" >/dev/null
ev "${WINS}.filter(w => w.get_wm_class() === 'org.vmlab.One').forEach(w => w.change_workspace_by_index(2, false)); 1" >/dev/null
go_to 4
ev "${WS}.openSet('One'); 1" >/dev/null
wait_for 20 opening_over
same "7 its window moved to the set's workspace" "$(ev "${WINS}.filter(w => w.get_wm_class() === 'org.vmlab.One').map(w => w.get_workspace().index()).join()")" 4
same "7 and no second one was started" "$(pgrep -u "${TEST_USER}" -fc "${WORK}/one.js")" 1

# --- 8 An app that is gone, 9 one that cannot start -----------------------------------------------------------------
close_all
gs reset org.gnome.desktop.wm.preferences workspace-names
chmod 0644 "${WORK}/broken"
for f in sets.json sets.json.bak; do
    jq '(.sets[] | select(.name == "Writing") | .windows) += [
        {"app": "vmlab-gone.desktop", "rank": 0, "place": {"m": .sets[0].windows[0].place.m, "x": 10, "y": 10, "w": 300, "h": 300, "max": false}},
        {"app": "vmlab-broken.desktop", "rank": 0, "place": {"m": .sets[0].windows[0].place.m, "x": 20, "y": 20, "w": 300, "h": 300, "max": false}}]' \
        "${STATE}/${f}" >"${WORK}/${f}" && install -m 0600 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}/${f}" "${STATE}/${f}"
done
reload_ui
wait_for 30 module_up
go_to 0
open_menu
same "8 the gone app shows the dimmed stand-in icon" "$(ev "${WS}._setsMenu._rows.get_children().map(r => r.get_children()[0].get_child().get_children()[0].get_children()).find(icons => icons.length === 4)?.filter(i => i.icon_name === 'application-x-executable-symbolic' && i.opacity === 128).length")" 1
close_menu
NOTES="Main.messageTray.getSources().flatMap(s => s.notifications.map(n => n.title))"
# Counted from here: the list keeps what earlier runs in this session said
broken_before="$(ev "${NOTES}.filter(t => t === 'Broken could not be started').length")"
ev "${WS}.openSet('Writing'); 1" >/dev/null
check "8 the rest started" wait_for 40 three_up
wait_for 40 opening_over
same "9 one notification for the app that could not start" "$(($(ev "${NOTES}.filter(t => t === 'Broken could not be started').length") - broken_before))" 1
same "8 none for the app that is gone" "$(ev "${NOTES}.filter(t => t.includes('vmlab-gone')).length")" 0

# --- 10 Windows later than 30 s are not waited for -------------------------------------------------------------------
close_all
for f in sets.json sets.json.bak; do
    jq '.sets = [{"name": "Slow", "saved": 1, "layout": "classic", "windows": [{"app": "org.vmlab.Slow.desktop", "rank": 0,
        "place": {"m": .sets[0].windows[0].place.m, "x": 37, "y": 91, "w": 333, "h": 222, "max": false}}]}] + .sets' \
        "${STATE}/${f}" >"${WORK}/${f}" && install -m 0600 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}/${f}" "${STATE}/${f}"
done
reload_ui
wait_for 30 module_up
go_to 0
ev "${WS}.openSet('Slow'); 1" >/dev/null
sleep 25
check "10 still waiting at 25 s" is_true "${SETS}.snapshot().opening !== null"
sleep 7
check "10 given up after 30 s, nothing connected" is_true "${SETS}.snapshot().opening === null && ${SETS}.connections === 0"
check "10 the late window did appear" wait_for 20 is_true "${WINS}.some(w => w.get_wm_class() === 'org.vmlab.Slow')"
sleep 1
slow="$(frames org.vmlab.Slow)"
if [[ "${slow}" == "[]" || -z "${slow}" ]]; then not_ok "10 the late window was left where GNOME put it" "no window"
elif near "${slow}" '[[0, 37, 91, 333, 222]]'; then not_ok "10 the late window was left where GNOME put it" "${slow}"; else ok "10 the late window was left where GNOME put it"; fi

# --- 10b The set's workspace removed while it opens -----------------------------------------------------------------
# With dynamic workspaces an empty workspace can go at any time: the opening ends, and nothing touches the workspace
close_all
user_bus pkill -f "${WORK}/slow.js" 2>/dev/null
go_to 0
ev "${WS}.openSet('Slow'); globalThis.vmlab31 = {target: ${SETS}._opening?.target ?? null}; 1" >/dev/null
go_to 1
check "10b the set's workspace can be removed" is_true "(() => { const t = globalThis.vmlab31.target; if (!t) return false;
    global.workspace_manager.remove_workspace(t, global.get_current_time()); return true; })()"
sleep 1
check "10b the opening ended with it, nothing connected" is_true \
    "${SETS}.snapshot().opening === null && ${SETS}.connections === 0"
ev "delete globalThis.vmlab31; while (global.workspace_manager.n_workspaces < 6)
    global.workspace_manager.append_new_workspace(false, global.get_current_time()); 1" >/dev/null
user_bus pkill -f "${WORK}/slow.js" 2>/dev/null

# --- 10c Two sets back to back ----------------------------------------------------------------------------------------
# The first set's apps are still on their way: the second does not open on its workspace
close_all
go_to 0
ev "${WS}.openSet('Slow'); 1" >/dev/null
sleep 1
ev "${WS}.openSet('Writing'); 1" >/dev/null
same "10c the second set went to the next empty workspace" "$(active)" 1
wait_for 40 opening_over
check "10c and its windows are there" is_true "${WINS}.filter(w => ['org.gnome.TextEditor', 'org.gnome.Nautilus', 'kitty']
    .includes(w.get_wm_class())).every(w => w.get_workspace().index() === 1)"
user_bus pkill -f "${WORK}/slow.js" 2>/dev/null

# --- 10d A window of a set's app opened elsewhere meanwhile stays where it was opened --------------------------------
close_all
go_to 2
ev "${WS}.openSet('Writing'); 1" >/dev/null
go_to 5
start files-elsewhere nautilus --new-window
wait_for 40 opening_over
sleep 2
check "10d the Files window opened on workspace 6 is still there" is_true \
    "${WINS}.some(w => w.get_wm_class() === 'org.gnome.Nautilus' && w.get_workspace().index() === 5)"

# --- 11 Places step aside -------------------------------------------------------------------------------------------
close_all
# On workspace 1, where the set was saved (10d left GNOME on 6); the place is compared without the workspace, which
# 10c's Slow, still on its way for its 30 s, may keep the set from
go_to 0
gs set org.gnome.shell.extensions.amethystora-ui.windows places true
sleep 1
start editor3 flatpak run org.gnome.TextEditor --new-window
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.gnome.TextEditor')"
sleep 2
ev "${WINS}.filter(w => w.get_wm_class() === 'org.gnome.TextEditor').forEach(w => w.move_resize_frame(false, 600, 400, 520, 380)); 1" >/dev/null
sleep 3
close_all
sleep 3
PLACES="${UI}.get('windows').places"
# Places holds a place of its own for Text Editor first, so that its stepping aside means something
check "11 places holds a place of its own for Text Editor" is_true "Object.values(${PLACES}?.data?.arrangements ?? {}).some(a =>
    (a.apps['org.gnome.TextEditor.desktop']?.places ?? []).some(p => p && Math.abs(p.x - JSON.parse('${SAVED_EDITOR}')[0][1]) > 2))"
before="$(ev "JSON.stringify(${PLACES}?.lastApplied ?? null)")"
ev "${WS}.openSet('Writing'); 1" >/dev/null
wait_for 40 opening_over
sleep 1
landed="$(frames org.gnome.TextEditor | python3 -c "import json,sys; print(json.dumps([[0] + f[1:] for f in json.load(sys.stdin)]))")"
if near "${landed}" "${SAVED_EDITOR}"; then ok "11 Text Editor landed at the set's place, not places'"
else not_ok "11 Text Editor landed at the set's place, not places'" "expected: ${SAVED_EDITOR}" "found: ${landed}"; fi
same "11 places did not place it" "$(ev "JSON.stringify(${PLACES}?.lastApplied ?? null)")" "${before}"
gs reset org.gnome.shell.extensions.amethystora-ui.windows places

# --- 12 The tiling ---------------------------------------------------------------------------------------------------
close_all
ame_ desktop layout tiling >/dev/null
sleep 2
go_to 1
STRIP="(() => { const e = ${UI}.get('tiling').engine; const s = e.strips().find(s => s.workspace.index() === WSX && s.columns.length);
    return JSON.stringify(s ? s.columns.map(c => c.map(w => [w.get_wm_class(), e.info(w).width])) : []); })()"
strip() { ev "${STRIP//WSX/$1}"; }
start editor4 flatpak run org.gnome.TextEditor --new-window
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.gnome.TextEditor')"
sleep 2
start files4 nautilus --new-window
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.gnome.Nautilus')"
sleep 2
start kitty4 kitty
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'kitty')"
sleep 2
ev "(() => { const e = ${UI}.get('tiling').engine; const s = e.strips().find(s => s.workspace.index() === 1);
    const order = ['org.gnome.TextEditor', 'org.gnome.Nautilus', 'kitty']; const width = [700, 500, 600];
    s.columns.sort((a, b) => order.indexOf(a[0].get_wm_class()) - order.indexOf(b[0].get_wm_class()));
    s.columns.forEach((c, i) => (e.info(c[0]).width = width[i])); e.layout(s, {animate: false}); return 1; })()" >/dev/null
sleep 1
gs set org.gnome.desktop.wm.preferences workspace-names "['', 'Code']"
ev "${SETS}.save(global.workspace_manager.get_workspace_by_index(1)); 1" >/dev/null
AVAILABLE="$(ev "(() => { const a = Main.layoutManager.getWorkAreaForMonitor(0); return a.width; })()")"
SAVED_STRIP="$(strip 1)"
close_all
go_to 1
ev "${WS}.openSet('Code'); imports.gi.GLib.timeout_add(0, 200, () => {
    global.workspace_manager.get_workspace_by_index(4).activate(global.get_current_time()); return false; }); 1" >/dev/null
wait_for 40 opening_over
sleep 2
same "12 the strip came back in its order and widths, on its workspace" "$(strip 1)" "${SAVED_STRIP}"
same "12 the workspace switched to meanwhile stays" "$(active)" 4

# --- 13 Two windows of one app around another -----------------------------------------------------------------------
close_all
go_to 1
start kitty5 kitty
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'kitty')"
sleep 2
start files5 nautilus --new-window
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.gnome.Nautilus')"
sleep 2
start kitty6 kitty
wait_for 30 is_true "${WINS}.filter(w => w.get_wm_class() === 'kitty').length === 2"
sleep 2
ev "(() => { const e = ${UI}.get('tiling').engine; const s = e.strips().find(s => s.workspace.index() === 1);
    const all = s.columns.flat(); const k = all.filter(w => w.get_wm_class() === 'kitty').sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence());
    const f = all.find(w => w.get_wm_class() === 'org.gnome.Nautilus'); s.columns = [[k[0]], [f], [k[1]]];
    e.info(k[0]).width = 650; e.info(f).width = 600; e.info(k[1]).width = 550; e.layout(s, {animate: false}); return 1; })()" >/dev/null
gs set org.gnome.desktop.wm.preferences workspace-names "['', 'Two']"
ev "${SETS}.save(global.workspace_manager.get_workspace_by_index(1)); 1" >/dev/null
TWO="$(strip 1)"
close_all
go_to 1
ev "${WS}.openSet('Two'); 1" >/dev/null
wait_for 40 opening_over
sleep 2
same "13 kitty, Files, kitty again, each at its width" "$(strip 1)" "${TWO}"

# --- 14 A classic set under the tiling ------------------------------------------------------------------------------
close_all
go_to 0
ev "${WS}.openSet('Writing'); 1" >/dev/null
wait_for 40 opening_over
sleep 2
expected="$(jq -c '[.sets[] | select(.name == "Writing") | .windows[] | select(.app | test("^vmlab") | not)] |
    sort_by(.place.x + .place.w / 2) | map(.app)' "${STATE}/sets.json")"
same "14 the columns follow the saved places' centres" \
    "$(ev "(() => { const e = ${UI}.get('tiling').engine; const s = e.strips().find(s => s.workspace.index() === 0 && s.columns.length);
        return JSON.stringify(s.columns.map(c => Shell.WindowTracker.get_default().get_window_app(c[0]).get_id())); })()")" "${expected}"

# --- 15 The rise ------------------------------------------------------------------------------------------------------
close_all
go_to 0
SAMPLE="globalThis.vmlab31 = {log: []}; global.window_manager.connectObject('map', (wm, a) => {
    const at = () => globalThis.vmlab31?.log.push([a.meta_window?.get_wm_class(), a.scale_x, a.translation_y, a.opacity]);
    imports.gi.GLib.timeout_add(0, 60, () => { at(); return false; });
    imports.gi.GLib.timeout_add(0, 660, () => { at(); return false; }); }, globalThis);"
ev "global.force_animations = true; ${SAMPLE} ${WS}.openSet('Two'); 1" >/dev/null
wait_for 40 opening_over
sleep 2
rise="$(ev "JSON.stringify(globalThis.vmlab31.log)")"
echo "# the rise sampled: ${rise}"
check "15 within 100 ms the window is a little smaller and lower" python3 -c "import json,sys; l=json.loads(sys.argv[1]);
first=[x for x in l if x[0]==l[0][0]][0]; sys.exit(0 if 0.89 < first[1] < 1 and first[2] > 0 else 1)" "${rise}"
check "15 600 ms later it is in place" python3 -c "import json,sys; l=json.loads(sys.argv[1]);
last=[x for x in l if x[0]==l[0][0]][1]; sys.exit(0 if last[1] == 1 and abs(last[2]) < 0.5 and last[3] == 255 else 1)" "${rise}"
same "15 it rests where the strip draws it" "$(ev "(() => { const e = ${UI}.get('tiling').engine; const windows = e.strips().flatMap(s => s.windows).filter(w => w.get_workspace() === global.workspace_manager.get_active_workspace()); return windows.length > 0 && windows.every(w => { const f = w.get_frame_rect(), a = w.get_compositor_private(), v = e.info(w).vis; return Math.abs(f.x + a.translation_x - v.x) <= 1 && Math.abs(f.y + a.translation_y - v.y) <= 1; }); })()")" true
ev "global.window_manager.disconnectObject(globalThis); 1" >/dev/null
close_all
gs set org.gnome.desktop.a11y.interface reduced-motion "'reduce'"
sleep 1
ev "${SAMPLE} ${WS}.openSet('Two'); 1" >/dev/null
wait_for 40 opening_over
sleep 2
rise="$(ev "JSON.stringify(globalThis.vmlab31.log)")"
echo "# the rise sampled: ${rise}"
check "15 with Reduce motion it only fades in" python3 -c "import json,sys; l=json.loads(sys.argv[1]);
s=[x for x in l if x[0]==l[0][0]]; sys.exit(0 if all(x[1] == 1 and x[2] == 0 for x in s) and any(x[3] < 255 for x in s) else 1)" "${rise}"
ev "global.window_manager.disconnectObject(globalThis); delete globalThis.vmlab31; global.force_animations = true; 1" >/dev/null
gs reset org.gnome.desktop.a11y.interface reduced-motion

# --- 16 Limits -------------------------------------------------------------------------------------------------------
close_all
ame_ desktop layout classic >/dev/null
go_to 5
start many gjs -m "${WORK}/many.js"
wait_for 40 is_true "global.workspace_manager.get_workspace_by_index(5).list_windows().length === 17"
gs set org.gnome.desktop.wm.preferences workspace-names "['', '', '', '', '', 'Many']"
open_menu 5
atspi click "Save this workspace" >/dev/null 2>&1
check "16 17 windows: the button says 16 were saved" atspi wait 5 "Saved as Many (16 windows)"
close_menu
same "16 the set holds 16" "$(ev "${SETS}.list.find(s => s.name === 'Many').windows.length")" 16
for n in $(seq 1 13); do
    gs set org.gnome.desktop.wm.preferences workspace-names "['', '', '', '', '', 'L${n}']"
    ev "${SETS}.save(global.workspace_manager.get_workspace_by_index(5)); 1" >/dev/null
done
same "16 twelve sets are kept, the oldest gone" "$(ev "${SETS}.list.map(s => s.name).join(' ')")" \
    "L13 L12 L11 L10 L9 L8 L7 L6 L5 L4 L3 L2"
close_all

# --- 17 Broken files -------------------------------------------------------------------------------------------------
sleep 1
cp "${STATE}/sets.json" "${WORK}/good.json"
printf '{"version":' >"${STATE}/sets.json"
reload_ui
wait_for 30 module_up
same "17 a broken file: the sets are its copy's" "$(ev "${SETS}.list.length")" 12
printf 'garbage' >"${STATE}/sets.json"
printf '[1,2' >"${STATE}/sets.json.bak"
reload_ui
wait_for 30 module_up
same "17 both broken: no sets" "$(ev "${SETS}.list.length")" 0
go_to 0
start files7 nautilus --new-window
wait_for 30 is_true "${WINS}.some(w => w.get_wm_class() === 'org.gnome.Nautilus')"
gs set org.gnome.desktop.wm.preferences workspace-names "['Again']"
ev "${SETS}.save(global.workspace_manager.get_workspace_by_index(0)); 1" >/dev/null
sleep 1.5
check "17 the next save writes both again" bash -c "jq -e '.sets[0].name == \"Again\"' '${STATE}/sets.json' &&
    jq -e '.sets[0].name == \"Again\"' '${STATE}/sets.json.bak'"

# --- 18 Forget ------------------------------------------------------------------------------------------------------
open_menu
ev "${WS}._setsMenu._rows.get_children()[0].get_children()[0].grab_key_focus(); 1" >/dev/null
atspi click "Forget the set Again" >/dev/null 2>&1 ||
    ev "${WS}._setsMenu._rows.get_children()[0].get_children()[1].emit('clicked', 1); 1" >/dev/null
sleep 1.5
if atspi has "Open the set Again" >/dev/null 2>&1; then not_ok "18 the cross took the set out of the menu"; else ok "18 the cross took the set out of the menu"; fi
same "18 and out of the file" "$(jq '.sets | length' "${STATE}/sets.json")" 0
close_menu

# --- 19 Idle, and ten disable/enable cycles ---------------------------------------------------------------------------
check "19 between openings nothing is connected" is_true "${SETS}.snapshot().opening === null && ${SETS}.connections === 0"
gs set org.gnome.desktop.wm.preferences workspace-names "['Again']"
ev "${SETS}.save(global.workspace_manager.get_workspace_by_index(0)); 1" >/dev/null
close_all
ITEMS="${WS}._menu._getMenuItems().length"
items="$(ev "${ITEMS}")"
for round in 1 2 3 4 5 6 7 8 9 10; do
    # The fifth goes in the middle of an opening
    [[ ${round} == 5 ]] && ev "${WS}.openSet('Again'); 1" >/dev/null && sleep 0.3
    ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
    sleep 0.3
    ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
    sleep 0.5
done
wait_for 30 module_up
same "19 after ten cycles the menu has the same items, the Sets part once" "$(ev "${ITEMS}")" "${items}"
check "19 and nothing is connected" is_true "${SETS}.connections === 0 && ${SETS}.snapshot().opening === null"
same "19 the set survived" "$(ev "${SETS}.list.map(s => s.name).join()")" Again

# --- 20 The setup file ------------------------------------------------------------------------------------------------
ame_ desktop sets off >/dev/null
ame_ setup save >/dev/null
SETUP_FILE="${HOME_DIR}/.config/amethystora/setup.json"
same "20 the setup file carries sets: false" "$(jq -c '.settings.workspaces.sets' "${SETUP_FILE}")" false
same "20 and none of the sets themselves" "$(grep -c '"windows"\|sets.json\|"Again"' "${SETUP_FILE}")" 0
ame_ desktop sets on >/dev/null

# --- Leave the account as it was ------------------------------------------------------------------------------------
cleanup
trap - EXIT
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    awk -v uuid="${UUID}" '/JS ERROR/ { block = $0; next } block && /@(resource|file):\/\// { block = block "\n" $0; next }
        block { if (index(block, uuid)) print block; block = "" } /Amethystora UI: .*(failed|threw|did not)/ { print }
        END { if (block && index(block, uuid)) print block }' | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of Amethystora UI threw"; else not_ok "Amethystora UI threw" "${errors}"; fi
finish
