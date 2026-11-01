#!/usr/bin/bash
# vmlab-vm: screens
# The dock on four screens side by side (plans/beyond/spec-dock.md 1.15, criteria 31 and 32 of 7.1), asked of the
# session through GNOME Shell's Eval, which a lab-only extension of the test account opens (never in the image):
#   - with every screen, four docks, one per screen, each keeping its strip free at the bottom;
#   - a screen's dock lists the running apps whose windows are on that screen, and with the tiling's separate screens,
#     those of the workspace that screen shows;
#   - with the main screen only, one dock counting every window;
#   - on the left edge, only the screen with nothing to its left keeps a strip; the others hide when touched,
#     without a barrier, which would hold the pointer back from the next screen;
#   - nothing of Amethystora UI throws meanwhile.
# vmlab-paths: @ui/extension.js @ui/*dock* @sf/*themed/shell-dock* @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
DOCK_SCHEMA=org.gnome.shell.extensions.amethystora-ui.dock
# Eval's answer is JSON.stringify of the value, which gdbus prints as a GVariant string: decoded once here, a string
# printed as itself and anything else as compact JSON. A throw prints nothing, says why on stderr and fails.
EV_DECODE='
import ast, json, re, sys
text = sys.stdin.read().strip()
answer = re.fullmatch(r"\((true|false), (.*)\)", text, re.S)
if not answer:
    sys.exit("Eval answered: " + text)
value = ast.literal_eval(answer.group(2))
if answer.group(1) == "false":
    sys.exit("the shell threw: " + value)
try:
    value = json.loads(value)
except ValueError:
    pass
print(value if isinstance(value, str) else json.dumps(value, separators=(",", ":"), ensure_ascii=False))'
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -I -c "${EV_DECODE}"
}
ev_is() { [[ "$(ev "$1")" == "$2" ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
DOCK="${UI}.get('dock')"
S="${DOCK}.state()"
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
# Each dock's strip and the work area of its screen, as "strip-top work-area-bottom" per screen
struts() { ev "${S}.docks.map(d => [d.monitor, Math.round(d.strip.y), (() => { const a = global.workspace_manager.get_active_workspace().get_work_area_for_monitor(d.monitor); return a.y + a.height; })()].join(':')).join(' ')"; }
check "every screen: four docks, one for each screen" test "$(ev "${S}.docks.map(d => d.monitor).join(' ')")" == "0 1 2 3"
bad=""
for entry in $(struts); do
    IFS=: read -r monitor top bottom <<<"${entry}"
    [[ ${top} == "${bottom}" ]] || bad+="screen ${monitor}: strip at ${top}, work area to ${bottom}; "
done
if [[ -z "${bad}" ]]; then ok "each screen keeps its dock's strip free"; else not_ok "the docks' strips" "${bad}"; fi

# Files, not pinned, with its window on screen 2 only, is on dock 2 only
FAVORITES="$(user_bus gsettings get org.gnome.shell favorite-apps)"
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop']"
user_bus systemd-run --user --collect --unit="vmlab-dock4-files-$$" nautilus --new-window >/dev/null
wait_for 60 ev_is "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus')" true
files="global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus')"
on_docks() { ev "${S}.docks.filter(d => d.items.some(i => i.id === 'org.gnome.Nautilus.desktop')).map(d => d.monitor).join(' ')"; }
ev "const w = ${files}; const tiling = ${UI}.get('tiling'); const ws = tiling?.engine?.screens?.shownOn(2) ?? global.workspace_manager.get_active_workspace(); w.change_workspace(ws); w.move_to_monitor(2); 1" >/dev/null
sleep 2
check "a running app is on the dock of the screen its window is on, and only there" test "$(on_docks)" == 2
# With the tiling's separate screens, a window moved to the workspace screen 1 shows is on dock 1
if [[ "$(ev "Boolean(${UI}.get('tiling')?.engine?.screens?.separate)")" == true ]]; then
    ev "const w = ${files}; const ws = ${UI}.get('tiling').engine.screens.shownOn(1); w.change_workspace(ws); 1" >/dev/null
    sleep 2
    check "moved to the workspace screen 1 shows, it is on dock 1" test "$(on_docks)" == 1
else
    skip "the tiling's separate screens are not on: the workspace a screen shows is GNOME's"
fi
in_session "ame desktop dock screens primary" >/dev/null
sleep 2
check "main screen only: one dock, on the main screen" test "$(ev "${S}.docks.map(d => d.monitor).join(' ')")" == \
    "$(ev "Main.layoutManager.primaryIndex")"
check "...counting every window" test "$(ev "${S}.docks[0].items.some(i => i.id === 'org.gnome.Nautilus.desktop')")" == true
in_session "ame desktop dock screens every" >/dev/null
sleep 2

# The left edge: screen 0 keeps a strip, the screens to its right cannot and hide when touched, without a barrier
in_session "ame desktop dock edge left" >/dev/null
sleep 3
left="$(ev "${S}.docks.map(d => [d.monitor, d.touch, d.barrier, Math.round(d.strip.width), global.workspace_manager.get_active_workspace().get_work_area_for_monitor(d.monitor).x - Main.layoutManager.monitors[d.monitor].x].join(':')).join(' ')")"
read -r first rest <<<"${left}"
IFS=: read -r _ touch barrier width areax <<<"${first}"
check "left edge: screen 0's dock keeps its strip" test "${touch} ${areax}" == "false ${width}"
bad=""
for entry in ${rest}; do
    IFS=: read -r monitor touch barrier width areax <<<"${entry}"
    [[ ${touch} == true && ${barrier} == false && ${areax} == 0 ]] || bad+="screen ${monitor}: ${entry}; "
done
if [[ -z "${bad}" ]]; then ok "screens 1 to 3 keep no strip, and hide when touched without a barrier"; else not_ok "the other screens" "${bad}"; fi
ev "const w = ${files}; w.move_to_monitor(1); w.change_workspace(global.workspace_manager.get_active_workspace()); w.maximize(imports.gi.Meta.MaximizeFlags.BOTH); 1" >/dev/null
sleep 2
check "a maximized window on screen 1 hides its dock" test "$(ev "${S}.docks.find(d => d.monitor === 1).hidden")" == true
in_session "ame desktop dock edge bottom" >/dev/null
user_bus gsettings set org.gnome.shell favorite-apps "${FAVORITES}"
user_bus systemctl --user stop "vmlab-dock4-files-$$" >/dev/null 2>&1

log="$(journalctl --since "${since}" "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --no-pager -o cat 2>/dev/null)"
errors="$(grep -E 'JS ERROR|Amethystora UI: the [a-z]+ module did not' <<<"${log}" | head -n 10)"
if [[ -z "${errors}" ]]; then ok "nothing threw in the shell meanwhile"; else not_ok "the shell's log has errors" "${errors}"; fi
finish
