#!/usr/bin/bash
# vmlab-vm: screens
# Four screens side by side, as the tiling has them (modules/tiling/screens.js), asked of the session itself through
# GNOME Shell's Eval, which a lab-only extension of the test account opens (never in the image):
#   - GNOME has the image's six fixed workspaces (GNOME Shell's own override sets dynamic ones for the GNOME desktop
#     alone, which once left two workspaces and no third screen any);
#   - each screen shows a workspace of its own, also for an account that set dynamic workspaces, which stay dynamic:
#     there is one more than screens, and a workspace a screen shows is kept when its last window leaves;
#   - a Files window goes from screen to screen with Super+Shift+Ctrl+Right and Left, and dragged by its title bar
#     across a screen to the next, onto the workspace each shows;
#   - the search box stays open while the pointer crosses a screen with a window;
#   - a window on a screen without the keyboard is drawn on glass, and the top bar is solid while a window of the
#     workspace its screen shows reaches it;
#   - nothing of Amethystora UI throws meanwhile.
# vmlab-paths: @ui/extension.js @ui/*tiling* @ui/*glass* @ui/*bars* @ui/*workspaces*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
# The value Eval gives back, a string as the expression made it
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
# A session ended while its screen is locked takes GNOME Shell down with it in the unlock dialog (GNOME's own), and
# the next one cannot start until it has gone: unlocked first, and tried again once
restart_session() {
    local try
    for try in 1 2; do
        loginctl unlock-sessions
        sleep 2
        systemctl restart gdm
        wait_for 240 has_session && wait_for 120 four_screens && break
    done
    sleep 10
}
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
# Each screen's workspace, by the screen's index: "0 1 2 3"
shown() { ev "Main.layoutManager.monitors.map((m, i) => ${UI}.get('tiling').engine.screens.shownOn(i).index()).join(' ')"; }
# The Files window's workspace and screen: "2 2"
files_at() { ev "(() => { const w = global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus'); return w ? w.get_workspace().index() + ' ' + w.get_monitor() : 'none'; })()"; }
# The workspace screen N shows and N, as files_at says it
screen_pair() { echo "$(shown | cut -d' ' -f$(($1 + 1))) $1"; }
files_on() { [[ "$(files_at)" == "$(screen_pair "$1")" ]]; }
files_open() { [[ "$(files_at)" != none ]]; }
focus_on() { [[ "$(ev 'global.display.focus_window?.get_monitor()')" == "$1" ]]; }
four_screens() { [[ "$(ev 'Main.layoutManager.monitors.length')" == 4 ]]; }
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
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
restart_session

check "GNOME has four screens side by side" test "$(ev "Main.layoutManager.monitors.map(m => m.x + ',' + m.y).join(' ')")" == "0,0 1280,0 2560,0 3840,0"
check "the session has the image's six fixed workspaces" test \
    "$(ev "imports.gi.Meta.prefs_get_dynamic_workspaces() + ' ' + global.workspace_manager.n_workspaces")" == "false 6"
check "each screen shows a workspace of its own" test "$(shown | tr ' ' '\n' | sort -u | wc -l)" == 4

# A window, from screen to screen by the keyboard and back
user_bus systemd-run --user --collect --unit="vmlab-files-$$" nautilus --new-window >/dev/null
wait_for 60 files_open
# The middle of screen 1 (a quarter of the way is the edge between screens 1 and 2)
keys move:0.125,0.5
ev "Main.activateWindow(global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus'))" >/dev/null
for screen in 1 2 3; do
    keys super+shift+ctrl+right
    check "Super+Shift+Ctrl+Right takes the window to screen $((screen + 1)) and its workspace" \
        wait_for 10 files_on "${screen}"
done
keys super+shift+ctrl+left super+shift+ctrl+left super+shift+ctrl+left
check "Super+Shift+Ctrl+Left three times brings it back to screen 1" wait_for 10 files_on 0

# Dragged by its title bar from screen 1, across screen 2, onto screen 3
frame="$(ev "(() => { const r = global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus').get_frame_rect(); return (r.x + 100) / 5120 + ',' + (r.y + 20) / 800; })()")"
keys "drag:${frame},0.625,0.5"
check "the window dragged across a screen lands on the screen it is dropped on" \
    wait_for 10 files_on 2

# The search box over a screen with a window
keys move:0.125,0.5 super+space
search_open() { [[ "$(ev "${UI}.get('search').open")" == true ]]; }
if wait_for 10 search_open; then
    keys move:0.62,0.6
    check "the search box stays open while the pointer crosses a screen with a window" search_open
    keys esc esc move:0.63,0.55
    check "once it is closed, the screen under the pointer has the keyboard" \
        wait_for 10 focus_on 2
else
    not_ok "Super+Space did not open the search box"
fi

# Screen 3 drawn as a picture while screen 1 has the keyboard: its window on glass
keys move:0.12,0.45
mirror_glass() {
    [[ "$(ev "(() => { const m = ${UI}.get('tiling').engine.screens._mirrors.get(2); const g = ${UI}.get('glass'); return !!m && m.actor.get_children().some(c => c instanceof imports.gi.Clutter.Clone && g.has(c)); })()")" == true ]]
}
check "a window on a screen without the keyboard is on glass" wait_for 10 mirror_glass

# The top bar: a window of screen 1's workspace moved up against it, the keyboard on screen 3
user_bus systemd-run --user --collect --unit="vmlab-zenity-$$" zenity --info --text=vmlab >/dev/null
zenity_up() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class()?.toLowerCase().includes('zenity'))")" == true ]]; }
if wait_for 30 zenity_up; then
    ev "global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class()?.toLowerCase().includes('zenity')).move_frame(true, 300, 20)" >/dev/null
    solid() { [[ "$(ev "Main.panel.has_style_class_name('amethystora-glass-clear')")" == false ]]; }
    check "the top bar is solid under a window that reaches it" wait_for 10 solid
    keys move:0.64,0.4
    check "and stays so while that workspace is drawn as a picture" wait_for 10 solid
    user_systemctl stop "vmlab-zenity-$$"
else
    not_ok "zenity's window did not come"
fi

# An account that set dynamic workspaces itself: they stay dynamic, and GNOME takes away no workspace a screen shows,
# empty ones included (a new session's are)
user_bus gsettings set org.gnome.mutter dynamic-workspaces true
restart_session
check "with dynamic workspaces set by the account, they stay dynamic" test \
    "$(ev "imports.gi.Meta.prefs_get_dynamic_workspaces()")" == true
# A new session's workspaces come with its screens, a moment after it starts
four_shown() { [[ "$(shown | tr ' ' '\n' | sort -u | wc -l)" == 4 ]]; }
five_or_more() { (($(ev 'global.workspace_manager.n_workspaces') >= 5)); }
check "and each screen still shows a workspace of its own" wait_for 20 four_shown
check "and there is one workspace more than screens at least" wait_for 20 five_or_more
check "no workspace had to be added with a word" test "$(ev "Main.messageTray.getSources().flatMap(s => s.notifications).filter(n => n.title === 'Workspaces added').length")" == 0
# A window opened on screen 2's workspace and taken to screen 3's: screen 2's, empty again, stays
# The session starts in the overview, where the keyboard does not follow the pointer; two moves, since the lab's
# tablet drops a position the same as the last it sent
ev "Main.overview.hide(); 'hidden'" >/dev/null
no_overview() { [[ "$(ev 'Main.overview.visible')" == false ]]; }
wait_for 10 no_overview
keys move:0.36,0.45 move:0.375,0.5
on_screen_2() { [[ "$(ev "global.workspace_manager.get_active_workspace() === ${UI}.get('tiling').engine.screens.shownOn(1)")" == true ]]; }
wait_for 10 on_screen_2
user_bus systemd-run --user --collect --unit="vmlab-files2-$$" nautilus --new-window >/dev/null
wait_for 60 files_open
ev "Main.activateWindow(global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus'))" >/dev/null
check "a window opens on screen 2's workspace" wait_for 10 files_on 1
ev "globalThis.vmlabShown = ${UI}.get('tiling').engine.screens.shownOn(1); 'kept'" >/dev/null
keys super+shift+ctrl+right
check "it goes to screen 3 with Super+Shift+Ctrl+Right" wait_for 10 files_on 2
sleep 3
check "screen 2 still shows its workspace, empty now" test "$(ev "(() => { const m = global.workspace_manager; return Array.from({length: m.n_workspaces}, (u, i) => m.get_workspace_by_index(i)).includes(globalThis.vmlabShown) && ${UI}.get('tiling').engine.screens.shownOn(1) === globalThis.vmlabShown; })()")" == true
check "and every screen still shows a workspace of its own" test "$(shown | tr ' ' '\n' | sort -u | wc -l)" == 4
user_systemctl stop "vmlab-files2-$$"
user_bus gsettings reset org.gnome.mutter dynamic-workspaces

# The wheel on the top bar steps the bar's own screen, notch after notch: no other screen changes, and the pointer
# stays where it is (a workspace another screen showed once took the pointer there, off the bar, with every later notch)
primary="$(ev 'Main.layoutManager.primaryIndex')"
bar_at="$(ev "(() => { const m = Main.layoutManager.primaryMonitor, s = global.stage; return ((m.x + m.width / 2) / s.width).toFixed(3) + ',' + ((m.y + 5) / s.height).toFixed(3); })()")"
keys "move:${bar_at}"
sleep 1
notch() {
    ev "${UI}.get('workspaces')._scrolled({get_scroll_direction: () => imports.gi.Clutter.ScrollDirection.$1}); true" >/dev/null
    sleep 1
}
others() { shown | tr ' ' '\n' | sed "$((primary + 1))d" | paste -sd ' '; }
bar_shows() { shown | cut -d' ' -f$((primary + 1)); }
pointer_on_bar() { [[ "$(ev 'global.display.get_current_monitor()')" == "${primary}" ]]; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
restore="$(bar_shows)"
before="$(others)"
# The workspaces no other screen shows, from the one the bar's screen is on to the last: the wheel stops at the end
list="$(ev "${UI}.get('tiling').engine.screens.workspacesFor(${primary}, false).map(w => w.index()).join(' ')")"
rest="${list#*"${restore}"}"
steps="$(wc -w <<<"${rest}")"
seen="${restore}"
for ((i = 0; i <= steps; i++)); do
    notch DOWN
    seen+=" $(bar_shows)"
done
same "the wheel steps the bar's screen through its own workspaces, and stops at the end" "${seen}" "${restore}${rest} ${list##* }"
same "...and no other screen changes" "$(others)" "${before}"
check "...and the pointer stays on the bar's screen" pointer_on_bar
for ((i = 0; i < steps; i++)); do
    notch UP
done
same "...and back up to where it was" "$(bar_shows)" "${restore}"
# The main bar lights what its own screen shows, also while the keyboard is on another screen
other_at="$(ev "(() => { const m = Main.layoutManager.monitors.find((m, i) => i !== ${primary}), s = global.stage; return ((m.x + m.width / 2) / s.width).toFixed(3) + ',' + ((m.y + m.height / 2) / s.height).toFixed(3); })()")"
keys "move:${other_at}"
sleep 1
same "...and the main bar lights its own screen's workspace with the keyboard elsewhere" "$(ev "${UI}.get('workspaces')._labels.findIndex(l => l.has_style_class_name('amethystora-workspaces-active'))")" "$(bar_shows)"
keys "move:${bar_at}"
sleep 1

# With animations off, a switch to a workspace no screen shows: GNOME's slide would end inside its loop over the screens,
# and an "already disposed" fails this suite
user_bus gsettings set org.gnome.desktop.interface enable-animations false
sleep 1
ev "(() => { const e = ${UI}.get('tiling').engine; const n = global.workspace_manager.n_workspaces; for (let i = n - 1; i >= 0; i--) { const w = global.workspace_manager.get_workspace_by_index(i); if (!e.screens.wasShown(w)) { w.activate(global.get_current_time()); return i; } } return -1; })()" >/dev/null
sleep 1
same "with animations off a switch leaves no slide behind" "$(ev "Main.wm._workspaceAnimation._switchData === null")" true
user_bus gsettings reset org.gnome.desktop.interface enable-animations
keys "move:${bar_at}"
sleep 1

# A JS error whose stack goes through the extension, or the extension's own word that a module failed (GNOME's own
# errors while the session is ended for a restart are not its)
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    awk -v uuid="${UUID}" '/JS ERROR/ { block = $0; next } block && /@(resource|file):\/\// { block = block "\n" $0; next }
        block { if (index(block, uuid)) print block; block = "" } /Amethystora UI: .*(failed|threw)/ { print }
        END { if (block && index(block, uuid)) print block }' | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of Amethystora UI threw"; else not_ok "Amethystora UI threw" "${errors}"; fi
finish
