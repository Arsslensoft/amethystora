#!/usr/bin/bash
# vmlab-vm: screens
# A top bar on every screen and the bar over fullscreen (plans/beyond/spec-quick.md 1.2, 1.3, criteria of 7
# "19-top-bars.sh"), on four screens side by side, asked of the session through GNOME Shell's Eval, which a lab-only
# extension of the test account opens (never in the image):
#   - three secondary bars, each at its screen's top and as tall as the main bar, with the workspace that screen shows
#     lit and the main bar's time, and nothing else of the main bar;
#   - a click on a label shows that workspace on its screen, and one another screen shows swaps with it;
#   - a maximised window starts under its screen's bar; ame desktop top-bar screens off takes the bars away;
#   - pushing the pointer against the top of a screen with a fullscreen window brings that screen's bar down;
#   - Super+Ctrl+B hides the bar of the screen the pointer is on, and shows it again;
#   - nothing of Amethystora UI throws, and ten disable/enable cycles leave no bar behind.
# The VM draws in software, where GNOME turns its animations off: the peek is checked by its state, not its slide.
# vmlab-paths: @ui/extension.js @ui/*bars* @sf/*themed/shell-bars* @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
ev_is() { [[ "$(ev "$1")" == "$2" ]]; }
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
B="${UI}.get('bars')"
four_screens() { [[ "$(ev 'Main.layoutManager.monitors.length')" == 4 ]]; }
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
ev "Main.overview.hide(); 1" >/dev/null

check "GNOME has four screens" four_screens
check "the bars module runs" ev_is "${UI}.has('bars')" true
check "the tiling runs with a workspace per screen" ev_is "Boolean(${UI}.get('tiling')?.engine?.screens?.separate)" true
check "as the image ships: the main screen's bar alone" ev_is "${B}.bars().length" 0
in_session "ame desktop top-bar screens on" >/dev/null
wait_for 10 ev_is "${B}.bars().length" 3

# 1. Three bars, each at its screen's top, as wide as it and as tall as the main bar
check "three secondary bars" ev_is "${B}.bars().length" 3
geometry="$(ev "${B}.bars().map(b => { const m = Main.layoutManager.monitors[b.monitor]; return [b.monitor, b.actor.x === m.x && b.actor.y === m.y && b.actor.width === m.width, b.actor.height === Main.layoutManager.panelBox.height].join(':'); }).join(' ')")"
check "each bar at its screen's top, its width, the main bar's height" test "${geometry}" == "1:true:true 2:true:true 3:true:true"

# 2. The lit label is the workspace each screen shows; nothing of the main bar's status area; the main bar's time
shown() { ev "Main.layoutManager.monitors.map((m, i) => ${UI}.get('tiling').engine.screens.shownOn(i).index()).join(' ')"; }
lit() { ev "${B}.bars().map(b => b.activeIndex()).join(' ')"; }
expected_lit() { shown | cut -d' ' -f2-4; }
check "each bar lights the workspace its screen shows" test "$(lit)" == "$(expected_lit)"
check "the workspaces and the clock, and nothing else of the main bar" test     "$(ev "${B}.bars().map(b => b.parts().join(',')).join(' ')")" == "workspaces,clock workspaces,clock workspaces,clock"
main_time() { ev "(Main.panel.statusArea.dateMenu._clockDisplay.text.match(/[0-9]{1,2}[:.][0-9]{2}/) ?? [''])[0]"; }
bar_times() { ev "${B}.bars().map(b => (b.clock().match(/[0-9]{1,2}[:.][0-9]{2}/) ?? [''])[0]).join(' ')"; }
t="$(main_time)"
check "each bar's clock says the main bar's time" test "$(bar_times)" == "${t} ${t} ${t}"

# 3. A click on label 5 of screen 2 shows workspace 5 there; a click on the workspace screen 3 shows swaps the two
label_at() {
    # The centre of label $2 on screen $1's bar, as fractions of the whole desktop for keys.py
    ev "(() => { const l = ${B}.bars().find(b => b.monitor === $1).labels()[$2]; const [x, y] = l.get_transformed_position(); const [w, h] = l.get_transformed_size(); return ((x + w / 2) / global.stage.width).toFixed(4) + ',' + ((y + h / 2) / global.stage.height).toFixed(4); })()"
}
keys "click:$(label_at 2 4)"
sleep 2
check "a click on label 5 of screen 2 shows workspace 5 there" test "$(shown | cut -d' ' -f3)" == 4
before="$(shown)"
other="$(cut -d' ' -f4 <<<"${before}")"
keys "click:$(label_at 2 "${other}")"
sleep 2
after="$(shown)"
check "a click on the workspace screen 3 shows swaps it with screen 2's" test \
    "$(cut -d' ' -f3,4 <<<"${after}")" == "${other} 4"
check "...and the bars follow" test "$(lit)" == "$(expected_lit)"
# A click on an empty workspace's label opens the overview; GNOME 51 keeps it open for a window started from a unit
# (no activation token), where the bars and the peek rightly show over it
ev "Main.overview.hide(); 1" >/dev/null
sleep 1

# 4. A maximised window on screen 1 starts under its bar
user_bus systemd-run --user --collect --unit="vmlab-bars-files-$$" nautilus --new-window >/dev/null
wait_for 60 ev_is "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus')" true
files="global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus')"
ev "const w = ${files}; w.change_workspace(${UI}.get('tiling').engine.screens.shownOn(1)); w.move_to_monitor(1); w.maximize(imports.gi.Meta.MaximizeFlags.BOTH); 1" >/dev/null
sleep 2
check "a maximised window on screen 1 starts under its bar" ev_is \
    "${files}.get_frame_rect().y >= Main.layoutManager.monitors[1].y + ${B}.bars().find(b => b.monitor === 1).actor.height" true

# 5. Off and on again
in_session "ame desktop top-bar screens off" >/dev/null
wait_for 10 ev_is "${B}.bars().length" 0
check "ame desktop top-bar screens off takes the bars away" ev_is "${B}.bars().length" 0
in_session "ame desktop top-bar screens on" >/dev/null
wait_for 10 ev_is "${B}.bars().length" 3
check "...and on brings three back" ev_is "${B}.bars().length" 3

# 6. The peek on screen 2: a fullscreen window there hides its bar; pushing against its top brings it down
# The peek, timed in the shell: when the panel started sliding down and reached its place, when the pointer left
# the bar, and when GNOME hid the bar again (ms). $1 is the box GNOME tracks, $2 the panel that slides in it.
peek_record() {
    ev "(() => { const G = imports.gi.GLib; const box = $1, panel = $2; const r = globalThis.__peekRec = {}; const now = () => G.get_monotonic_time() / 1000; globalThis.__peekUndo = [[panel, panel.connect('notify::translation-y', p => { if (p.translation_y < 0 && r.start === undefined) r.start = now(); if (p.translation_y === 0 && r.start !== undefined && r.down === undefined) r.down = now(); })], [box, box.connect('notify::hover', b => { if (!b.hover && r.down !== undefined && r.left === undefined) r.left = now(); })], [box, box.connect('notify::visible', b => { if (!b.visible && r.left !== undefined && r.gone === undefined) r.gone = now(); })]]; return 1; })()" >/dev/null
}
peek_times() { ev "(() => { const r = globalThis.__peekRec ?? {}; return [Math.round(r.down - r.start), Math.round(r.gone - r.left)].join(' '); })()"; }
peek_stop() { ev "(() => { (globalThis.__peekUndo ?? []).forEach(([a, id]) => a.disconnect(id)); globalThis.__peekUndo = []; return 1; })()" >/dev/null; }
# The VM draws in software, where GNOME turns its animations off: the peek's slides are forced on while they are timed
animations() { ev "global.force_animations = $1; 1" >/dev/null; }
trap 'animations false' EXIT
ev "const w = ${files}; w.unmaximize(imports.gi.Meta.MaximizeFlags.BOTH); w.change_workspace(${UI}.get('tiling').engine.screens.shownOn(2)); w.move_to_monitor(2); w.make_fullscreen(); 1" >/dev/null
sleep 2
bar2="${B}.bars().find(b => b.monitor === 2).actor"
check "a fullscreen window on screen 2 hides its bar" ev_is "${bar2}.visible" false
x="$(ev "((Main.layoutManager.monitors[2].x + Main.layoutManager.monitors[2].width / 2) / global.stage.width).toFixed(4)")"
main_before="$(ev "Main.layoutManager.panelBox.visible")"
animations true
peek_record "${bar2}" "${bar2}.get_first_child()"
# GNOME's pressure barrier counts at most 15 px a hit against 100: twelve hits, so that the push adds up
push=("move:${x},0.05")
for _ in $(seq 12); do push+=("rel:0,-30"); done
keys "${push[@]}"
wait_for 5 ev_is "${B}.peeking(2)" true
check "pushing against the top of screen 2 brings its bar down" ev_is "${B}.peeking(2) && ${bar2}.visible" true
check "...and leaves the main bar as it was" ev_is "Main.layoutManager.panelBox.visible" "${main_before}"
wait_for 2 ev_is "${bar2}.get_first_child().translation_y" 0
keys "move:${x},0.5"
# How long it stayed is timed in the shell (peek_times): an Eval takes too long to time it from here
sleep 1.5
check "moving away, the bar goes again within a second and a half" ev_is "${B}.peeking(2) || ${bar2}.visible" false
read -r down gone <<<"$(peek_times)"
check "it slid down within 400 ms (${down} ms)" test "${down:-999}" -le 400
check "it went 700 ms after the pointer left, and its slide (${gone} ms)" test "${gone:-0}" -ge 700 -a "${gone:-0}" -le 1300
peek_stop
animations false
ev "${files}.unmake_fullscreen(); 1" >/dev/null
sleep 1

# 7. Super+Ctrl+B on screen 3 hides its bar, and shows it again
bar3="${B}.bars().find(b => b.monitor === 3).actor"
x3="$(ev "((Main.layoutManager.monitors[3].x + Main.layoutManager.monitors[3].width / 2) / global.stage.width).toFixed(4)")"
keys "move:${x3},0.5" super+ctrl+b
wait_for 5 ev_is "${bar3}.visible" false
check "Super+Ctrl+B with the pointer on screen 3 hides its bar" ev_is "${bar3}.visible" false
keys super+ctrl+b
wait_for 5 ev_is "${bar3}.visible" true
check "...and again shows it" ev_is "${bar3}.visible" true
user_bus systemctl --user stop "vmlab-bars-files-$$" >/dev/null 2>&1

# 8. Ten disable/enable cycles leave no bar behind
# One step an Eval: GNOME finishes each before the next
for i in $(seq 10); do
    ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
    ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
done
sleep 3
check "ten cycles: three bars, no more" ev_is "Main.layoutManager.uiGroup.get_children().filter(a => a.has_style_class_name?.('amethystora-bars-box')).length" 3
# GNOME 51 turns an extension off once the Eval that asked has returned (its manager awaits a lock): one step each
ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
sleep 1
check "...and GNOME's visibility rule is its own again while disabled" ev_is "!Object.prototype.hasOwnProperty.call(Main.layoutManager, '_updateActorVisibility') && !Main.layoutManager.uiGroup.get_children().some(a => a.has_style_class_name?.('amethystora-bars-box'))" true
ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
sleep 2
in_session "gsettings reset org.gnome.shell.extensions.amethystora-ui.bars every-screen" >/dev/null

log="$(journalctl --since "${since}" "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --no-pager -o cat 2>/dev/null)"
errors="$(grep -E 'JS ERROR|Amethystora UI: the [a-z]+ module did not' <<<"${log}" | head -n 10)"
if [[ -z "${errors}" ]]; then ok "nothing threw in the shell meanwhile"; else not_ok "the shell's log has errors" "${errors}"; fi
finish
