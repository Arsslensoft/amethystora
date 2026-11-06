#!/usr/bin/bash
# vmlab-vm: r5-finish
# Rounds 5 and 6, group finish (plans/ui-ideas/spec-finish.md, its acceptance criteria), in the test account's session,
# asked of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   1 named spaces in the overview; 2 the digest's two lists; 3 "+N" on the Downloads stack; 4 the dock's icon size;
#   5 each glass's blur and tint; 6 the gallery of widgets; 7 the resting screen; 8 living wallpapers; 9 trimming a
#   recording; 10 ten disables and enables leaving nothing; 11 Control, the recipes and the setup file; 12 nothing threw.
# The lab renders in software: GNOME turns its animations off there, so the parts that move are checked with
# global.force_animations set, and put back after.
# vmlab-paths: @ui/extension.js @ui/*workspaces* @ui/*pill* @ui/*dock* @ui/*glass* @ui/*rest* @ui/*living* @ui/*screenshot* @ui/*deskwidgets* @sf/*themed/shell-dock* @sf/*themed/shell-living* @sf/*widgets* @sf/usr/bin/amethystora-trim @sf/*amethystora-trim* apps/trim apps/control apps/control-core @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/*just/desktop.just @ui/*plugins* @sf/usr/share/amethystora/plugins/* @sf/usr/libexec/amethystora-plugins
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HERE="$(dirname "$0")"
HOME_DIR="/var/home/${TEST_USER}"
PILL=org.gnome.shell.extensions.amethystora-ui.pill
DOCK=org.gnome.shell.extensions.amethystora-ui.dock
GLASS=org.gnome.shell.extensions.amethystora-ui.glass
REST=org.gnome.shell.extensions.amethystora-ui.rest
LIVING=org.gnome.shell.extensions.amethystora-ui.living
SHOT=org.gnome.shell.extensions.amethystora-ui.screenshot
DESK=org.gnome.shell.extensions.amethystora-ui.deskwidgets
TILING=org.gnome.shell.extensions.amethystora-ui.tiling
WORK="$(mktemp -d)"
chmod 755 "${WORK}"
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
gs() { in_session "gsettings $(printf "%q " "$@")" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
like() { [[ "$1" == $2 ]]; }
is() { [[ "$(ev "$1")" == "$2" ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
P="${UI}.get('pill')"
H="${P}.hold"
D="${UI}.get('dock')"
G="${UI}.get('glass')"
R="${UI}.get('rest')"
LV="${UI}.get('living')"
W="${UI}.get('workspaces')._overviewNames"
S="${UI}.get('screenshot')"
state_is() { is "Main.extensionManager.lookup('${UUID}').state" "$1"; }
ready() { is "typeof ${R}?.state" function; }
keys() { python3 "${HERE}/keys.py" "$@"; }
n=0
notify() {
    n=$((n + 1))
    user_bus systemd-run --user --collect --quiet --unit="vmlab-finish-${n}" -- timeout 120 notify-send "$@" >/dev/null 2>&1
}
poke() { ev "Main.messageTray._updateState(); 'x'" >/dev/null; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

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
reset_all() {
    for key in digest digest-through digest-wait; do gs reset "${PILL}" "${key}"; done
    gs reset "${DOCK}" icon-size
    for key in blur-frost blur-prism blur-clear tint-frost tint-prism tint-clear; do gs reset "${GLASS}" "${key}"; done
    for key in enabled after; do gs reset "${REST}" "${key}"; done
    gs reset "${LIVING}" wallpaper
    gs reset "${SHOT}" recording-trim
    gs reset "${DESK}" enabled
    gs reset "${TILING}" workspace-backgrounds
    gs reset org.gnome.desktop.wm.preferences workspace-names
}
reset_all
loginctl unlock-sessions
systemctl restart gdm
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "the finish group's modules did not start in the session"
    finish
fi
wait_for 30 is "Main.layoutManager._startingUp" false
ev "Main.overview.hide(); 'hidden'" >/dev/null
wait_for 10 is "Main.overview.visible" false
cleanup() {
    ev "global.force_animations = false; ${R}?.endNow?.(); 'x'" >/dev/null 2>&1
    user_bus systemctl --user stop 'vmlab-finish-*' >/dev/null 2>&1
    pkill -u "${TEST_USER}" -f /usr/lib/amethystora-trim/amethystora-trim 2>/dev/null
    reset_all
    rm -rf "${HOME_DIR}/.config/amethystora/widgets/"{clock,clock-2,system} "${HOME_DIR}/Downloads/vmlab-"*
    rm -rf "${WORK}"
}
trap cleanup EXIT

# --- 1: named spaces in the overview (R5-32) ------------------------------------------------------------------------
gs set org.gnome.desktop.wm.preferences workspace-names "['Mail', 'Code']"
PICTURE=/usr/share/backgrounds/amethystora
picture="$(find "${PICTURE}" -maxdepth 1 -type f \( -name '*.png' -o -name '*.jpg' -o -name '*.svg' \) | head -n1)"
gs set "${TILING}" workspace-backgrounds "['', '${picture}']"
ev "Main.overview.show(); 'x'" >/dev/null
check "1 the overview's strip shows names" wait_for 20 is "${W}.names().length > 0" true
names="$(ev "JSON.stringify(${W}.names().slice(0, 3).map(n => n.name))")"
same "1 the person's names, then the number" "${names}" '["Mail","Code","3"]'
same "1 a workspace's own picture under its windows" "$(ev "${W}.names()[1].picture")" true
same "1 none for a workspace without one" "$(ev "${W}.names()[0].picture")" false
gs set org.gnome.desktop.wm.preferences workspace-names "['Post', 'Code']"
check "1 a new name shows at once" wait_for 10 is "${W}.names()[0].name" Post
ev "Main.overview.hide(); 'x'" >/dev/null
wait_for 10 is "Main.overview.visible" false
same "1 nothing is kept while the overview is shut" "$(ev "${W}.names().length")" 0
same "1 nothing is listened to then" "$(ev "!!${W}._watching")" false
gs reset org.gnome.desktop.wm.preferences workspace-names
gs reset "${TILING}" workspace-backgrounds

# --- 2: the digest's two lists (R5-33) ------------------------------------------------------------------------------
APP=org.gnome.TextEditor
in_session "ame desktop digest through ${APP}.desktop" >/dev/null
same "2 an app that always comes through" "$(gs get "${PILL}" digest-through)" "['${APP}.desktop']"
in_session "ame desktop digest wait ${APP}.desktop" >/dev/null
same "2 is moved to the other list, not on both" "$(gs get "${PILL}" digest-through)" "@as []"
same "2 always waits" "$(gs get "${PILL}" digest-wait)" "['${APP}.desktop']"
# With the digest off, the app that always waits is held for the digest, another app is not
before="$(ev "${H}.digestHeld")"
notify --hint=string:desktop-entry:${APP} "vmlab waits" "for the digest"
poke
check "2 with the digest off, an app on the wait list waits" wait_for 20 is "${H}.digestHeld > ${before}" true
notify "vmlab other" "comes as it comes"
poke
check "2 another app shows as it comes" wait_for 20 is "Main.messageTray._banner?.notification?.title ?? ''" "vmlab other"
ev "${H}.showDigestNow(); 'x'" >/dev/null
check "2 the waiting one comes in the digest card" wait_for 10 is "${H}.digestShown" true
ev "${H}._closeDigest(); 'x'" >/dev/null
# The digest on, an app let through shows at once while every other waits
in_session "ame desktop digest through ${APP}.desktop" >/dev/null
gs set "${PILL}" digest true
notify --hint=string:desktop-entry:${APP} "vmlab through" "at once"
poke
check "2 with the digest on, an app let through shows at once" \
    wait_for 20 is "Main.messageTray._banner?.notification?.title ?? ''" "vmlab through"
before="$(ev "${H}.digestHeld")"
notify "vmlab held" "waits"
poke
check "2 and another app waits" wait_for 20 is "${H}.digestHeld > ${before}" true
in_session "ame desktop digest through reset" >/dev/null
in_session "ame desktop digest wait reset" >/dev/null
gs reset "${PILL}" digest
same "2 the lists reset empty" "$(gs get "${PILL}" digest-wait)" "@as []"
user_bus gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications \
    --method org.freedesktop.Notifications.CloseNotification 0 >/dev/null 2>&1

# --- 3: "+N" on the Downloads stack (R5-35) -------------------------------------------------------------------------
DL="$(in_session "xdg-user-dir DOWNLOAD" 2>/dev/null)"
DL="${DL:-${HOME_DIR}/Downloads}"
as_user "mkdir -p '${DL}'"
same "3 nothing has arrived" "$(ev "${D}.stacks.arrivals()")" 0
as_user "echo one >'${DL}/vmlab-one.txt'"
check "3 a file arriving counts" wait_for 15 is "${D}.stacks.arrivals()" 1
# A browser's download: an empty file, the partial one, then the whole one renamed onto it, counts once
as_user "touch '${DL}/vmlab-two.zip'; echo half >'${DL}/vmlab-two.zip.part'; mv -f '${DL}/vmlab-two.zip.part' '${DL}/vmlab-two.zip'"
check "3 a download in two steps counts once" wait_for 15 is "${D}.stacks.arrivals()" 2
as_user "touch '${DL}/.vmlab-hidden'"
sleep 2
same "3 a hidden file does not count" "$(ev "${D}.stacks.arrivals()")" 2
check "3 the count shows on the tile" wait_for 10 is \
    "(() => { const d = ${D}._sites[0].dash; for (const s of d._items.values()) if (s.arrivals?.visible) return s.arrivals.text; return ''; })()" "+2"
ev "(() => { const d = ${D}._sites[0].dash; const place = ${D}.stacks.list().find(p => p.value === 'xdg:DOWNLOAD'); ${D}.stacks._seen(place); return 'x'; })()" >/dev/null
same "3 opened, the count is gone" "$(ev "${D}.stacks.arrivals()")" 0
as_user "rm -f '${DL}'/vmlab-* '${DL}/.vmlab-hidden'"

# --- 4: the dock's icon size (R6-28) --------------------------------------------------------------------------------
gnome_size="$(ev "${D}._sites[0].dash.iconSize")"
in_session "ame desktop dock size 56" >/dev/null
check "4 the icons take the size picked" wait_for 10 is "${D}._sites[0].dash.iconSize" 56
in_session "ame desktop dock size 40" >/dev/null
check "4 smaller too" wait_for 10 is "${D}._sites[0].dash.iconSize" 40
if in_session "ame desktop dock size 80" >/dev/null 2>&1; then not_ok "4 a size out of range is refused"; else ok "4 a size out of range is refused"; fi
in_session "ame desktop dock size reset" >/dev/null
check "4 reset, the dock's own choice again" wait_for 10 is "${D}._sites[0].dash.iconSize" "${gnome_size}"

# --- 5: each glass's blur and tint (R6-28) --------------------------------------------------------------------------
material="$(gs get "${GLASS}" material | tr -d "'")"
radius="$(ev "${G}.recipeFor('${material}').radius")"
in_session "ame desktop glass-strength blur 150" >/dev/null
same "5 a deeper blur, the same pass" "$(ev "${G}.recipeFor('${material}').radius === Math.round(${radius} * 1.5)")" true
own="$(ev "JSON.stringify([${G}.materialFor('glass').veil[0], ${G}.materialFor('glass').tint])")"
in_session "ame desktop glass-strength tint 50" >/dev/null
same "5 a clearer tint: the veil and the white at half the glass's own" \
    "$(ev "(() => { const [veil, tint] = ${own}; const m = ${G}.materialFor('glass');
        return Math.abs(m.veil[0] - veil / 2) < 1e-9 && Math.abs(m.tint - tint / 2) < 1e-9; })()")" true
other=frost
[[ "${material}" == frost ]] && other=clear
in_session "ame desktop glass-strength tint 150 ${other}" >/dev/null
same "5 each glass keeps its own" "$(gs get "${GLASS}" "tint-${other}")" 1.5
same "5 the glass in use is not changed by another's" "$(gs get "${GLASS}" "tint-${material}")" 0.5
for key in blur tint; do in_session "ame desktop glass-strength ${key} reset" >/dev/null; done
in_session "ame desktop glass-strength tint reset ${other}" >/dev/null
same "5 reset, the glass's own again" "$(ev "${G}.materialFor('glass').tint === ${own}[1]")" true

# --- 6: the gallery of widgets (R6-29), now the plugins the image ships ----------------------------------------------
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "6 Control's gallery" "$(jq -r '.items[] | select(.id == "plugins-shipped") | [.rows[].id] | sort | join(",")' <<<"${control}")" \
    "amethystora.calendar,amethystora.clock,amethystora.do-not-disturb,amethystora.hello,amethystora.now-playing,amethystora.pomodoro,amethystora.system"
same "6 none is added" "$(in_session "/usr/libexec/amethystora-plugins list --json | jq length" 2>/dev/null)" 0
for example in /usr/share/amethystora/plugins/*/; do
    check "6 $(basename "${example}") passes the plugins' check" in_session "/usr/libexec/amethystora-plugin-host check ${example} --system"
done
if in_session "ame desktop widgets add nothing-such bar" >/dev/null 2>&1; then
    not_ok "6 an example that is not there is refused"
else
    ok "6 an example that is not there is refused"
fi
same "6 nothing is read from the old widgets folder" "$(in_session "ls ~/.config/amethystora/widgets 2>/dev/null | wc -l" 2>/dev/null)" 0

# --- 7: the resting screen (R6-06) ----------------------------------------------------------------------------------
same "7 off as the image ships, and nothing watched" "$(ev "JSON.stringify([${R}.state().enabled, ${R}.state().watching])")" \
    "[false,false]"
in_session "ame desktop resting-screen on" >/dev/null
check "7 on, one idle watch" wait_for 10 is "${R}.state().watching" true
in_session "ame desktop resting-screen after 5" >/dev/null
same "7 after 5 minutes" "$(gs get "${REST}" after)" 300
ev "global.force_animations = true; 'x'" >/dev/null
same "7 it rests" "$(ev "${R}.restNow()")" true
check "7 a large clock" like "$(ev "${R}.state().time")" "*:*"
check "7 and when it locks, as it would anyway" like "$(ev "${R}.state().note")" "*"
same "7 the windows hidden under it" "$(ev "Main.layoutManager.uiGroup.get_children().some(a => a.name === 'amethystora-rest' && a.visible)")" true
check "7 the wallpaper drifts while it may" wait_for 10 is "${R}.state().drifting" true
gs set org.gnome.desktop.a11y.interface reduced-motion "'reduce'"
check "7 and holds still with reduced motion" wait_for 10 is "${R}.state().drifting" false
gs reset org.gnome.desktop.a11y.interface reduced-motion
keys shift
check "7 any key brings everything back" wait_for 10 is "${R}.state().resting" false
same "7 nothing of it is left" "$(ev "Main.layoutManager.uiGroup.get_children().some(a => a.name === 'amethystora-rest')")" false
ev "global.force_animations = false; 'x'" >/dev/null
# A fullscreen window keeps it away
user_bus systemd-run --user --collect --quiet --unit=vmlab-finish-full -- gnome-text-editor >/dev/null 2>&1
if wait_for 30 is "global.display.focus_window ? 1 : 0" 1; then
    ev "global.display.focus_window.make_fullscreen(); 'x'" >/dev/null
    check "7 not while a window is fullscreen" wait_for 10 is "${R}.state().why" fullscreen
    ev "global.display.focus_window?.unmake_fullscreen(); 'x'" >/dev/null
else
    skip "7 not while a window is fullscreen (no window opened)"
fi
user_bus systemctl --user stop vmlab-finish-full >/dev/null 2>&1
in_session "ame desktop resting-screen off" >/dev/null
check "7 off, nothing watched" wait_for 10 is "${R}.state().watching" false

# --- 8: living wallpapers (R6-07) -----------------------------------------------------------------------------------
same "8 off as the image ships, nothing drawn" "$(ev "${LV}.state().layers")" 0
ev "global.force_animations = true; 'x'" >/dev/null
in_session "ame desktop living-wallpaper aurora" >/dev/null
check "8 one layer for each screen" wait_for 10 is "${LV}.state().layers === Main.layoutManager.monitors.length" true
check "8 it moves while seen" wait_for 10 is "${LV}.state().moving" true
frames="$(ev "${LV}.state().frames")"
sleep 2
same "8 drawn at a low frame rate" "$(ev "(() => { const f = ${LV}.state().frames - ${frames}; return f > 2 && f < 15 * 2 * Main.layoutManager.monitors.length + 10; })()")" true
gs set org.gnome.desktop.a11y.interface reduced-motion "'reduce'"
check "8 still with reduced motion" wait_for 10 is "${LV}.state().still" reduce-motion
frames="$(ev "${LV}.state().frames")"
sleep 2
same "8 and nothing drawn by itself then" "$(ev "${LV}.state().frames - ${frames} < 3")" true
gs reset org.gnome.desktop.a11y.interface reduced-motion
user_bus systemd-run --user --collect --quiet --unit=vmlab-finish-max -- gnome-text-editor >/dev/null 2>&1
if wait_for 30 is "global.display.focus_window ? 1 : 0" 1; then
    ev "global.display.focus_window.make_fullscreen(); 'x'" >/dev/null
    check "8 covered, it is not drawn at all" wait_for 10 is "${LV}.state().still" covered
    same "8 and its layer is hidden" "$(ev "${LV}.state().shown")" 0
    ev "global.display.focus_window?.unmake_fullscreen(); 'x'" >/dev/null
else
    skip "8 covered, it is not drawn at all (no window opened)"
fi
user_bus systemctl --user stop vmlab-finish-max >/dev/null 2>&1
for kind in facets rain dusk; do
    in_session "ame desktop living-wallpaper ${kind}" >/dev/null
    check "8 ${kind} draws" wait_for 10 is "${LV}.kind" "${kind}"
done
in_session "ame desktop living-wallpaper off" >/dev/null
check "8 off, nothing left" wait_for 10 is "${LV}.state().layers" 0
ev "global.force_animations = false; 'x'" >/dev/null

# --- 9: trimming a recording (R6-19) --------------------------------------------------------------------------------
same "9 off as the image ships, GNOME's own" "$(ev "${S}.trim.wrapped")" false
in_session "ame desktop screenshot trim on" >/dev/null
check "9 on, the recording's notice is wrapped" wait_for 10 is "${S}.trim.wrapped" true
VIDEOS="${HOME_DIR}/Videos/Screencasts"
as_user "mkdir -p '${VIDEOS}' && ffmpeg -nostdin -v error -f lavfi -i testsrc=size=640x360:rate=24 -t 6 -c:v libvpx -b:v 1M '${VIDEOS}/vmlab.webm'"
check "9 a recording to trim" test -s "${VIDEOS}/vmlab.webm"
ev "(() => { const ui = Main.screenshotUI; ${S}.trim._stopping = '${VIDEOS}/vmlab.webm'; ui._screencastPath = '${VIDEOS}/vmlab.webm';
    ui._showNotification('vmlab recorded'); ${S}.trim._stopping = null; return 'x'; })()" >/dev/null
same "9 its notice offers Trim" "$(ev "(() => { const n = Main.messageTray.getSources().flatMap(s => s.notifications).find(n => n.title === 'vmlab recorded'); return JSON.stringify(n?.actions.map(a => a.label) ?? []); })()")" \
    '["Show in Files","Trim"]'
ev "(() => { Main.messageTray.getSources().flatMap(s => s.notifications).find(n => n.title === 'vmlab recorded').actions.find(a => a.label === 'Trim').activate(); return 'x'; })()" >/dev/null
trim_has() { user_bus python3 "${HERE}/atspi.py" amethystora-trim has "$1" >/dev/null 2>&1; }
check "9 Trim opens the window on it" wait_for 30 trim_has "Save as GIF"
check "9 its length read" wait_for 30 trim_has "0:06"
user_bus python3 "${HERE}/atspi.py" amethystora-trim click "Save as GIF" >/dev/null 2>&1
check "9 Save as GIF saves beside it" wait_for 60 test -s "${VIDEOS}/vmlab (trimmed).gif"
user_bus python3 "${HERE}/atspi.py" amethystora-trim click "Save as WebM" >/dev/null 2>&1
check "9 Save as WebM too" wait_for 120 test -s "${VIDEOS}/vmlab (trimmed).webm"
same "9 the recording itself is as it was" "$(ffprobe -v error -show_entries format=duration -of csv=p=0 "${VIDEOS}/vmlab.webm" | cut -d. -f1)" 6
user_bus python3 "${HERE}/atspi.py" amethystora-trim click "Save as GIF" >/dev/null 2>&1
check "9 and nothing is written over" wait_for 60 test -s "${VIDEOS}/vmlab (trimmed 2).gif"
pkill -u "${TEST_USER}" -f /usr/lib/amethystora-trim/amethystora-trim
# The find runs at each round (a substitution here would be read once)
check "9 its frames go with the window" wait_for 10 bash -c 'test -z "$(find /run/user/'"$(id -u "${TEST_USER}")"' -maxdepth 1 -name "amethystora-trim-*")"'
in_session "ame desktop screenshot trim off" >/dev/null
check "9 off, GNOME's own again" wait_for 10 is "${S}.trim.wrapped" false
rm -rf "${VIDEOS}/vmlab"*

# --- 10: ten disables and enables -----------------------------------------------------------------------------------
gs set "${REST}" enabled true
gs set "${LIVING}" wallpaper "'dusk'"
gs set "${SHOT}" recording-trim true
census='(() => { const m = {}; const v = a => { const k = a.style_class || a.constructor.name; m[k] = (m[k] ?? 0) + 1; a.get_children().forEach(v); }; v(Main.uiGroup); m.statusArea = Object.keys(Main.panel.statusArea).length; return m; })()'
off() { ev "Main.extensionManager.disableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 2 && sleep 1; }
on() { ev "Main.extensionManager.enableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 1; }
off
ev "globalThis.vmlabCensus = ${census}; 'x'" >/dev/null
cycles=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
    on || break
    off || break
    cycles=$((cycles + 1))
done
same "10 ten cycles" "${cycles}" 10
same "10 leave nothing" "$(ev "(() => { const a = ${census}, b = globalThis.vmlabCensus;
    return JSON.stringify(Object.keys({...a, ...b}).filter(k => (a[k] ?? 0) !== (b[k] ?? 0)).map(k => k + ' ' + ((a[k] ?? 0) - (b[k] ?? 0))).slice(0, 12)); })()")" "[]"
same "10 off, GNOME's recording notice is its own" "$(ev "Object.hasOwn(Main.screenshotUI, '_showNotification')")" false
on
wait_for 60 ready
gs reset "${REST}" enabled
gs reset "${LIVING}" wallpaper
gs reset "${SHOT}" recording-trim

# --- 11: Control, the recipes and the setup file --------------------------------------------------------------------
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "11 Control's entries" \
    "$(jq -r '[.items[] | select(.id | IN("digest-through", "digest-wait", "dock-size", "glass-blur", "glass-tint", "resting-screen", "resting-screen-after", "living-wallpaper", "screenshot-trim")) | .id] | sort | join(",")' <<<"${control}")" \
    "digest-through,digest-wait,dock-size,glass-blur,glass-tint,living-wallpaper,resting-screen,resting-screen-after,screenshot-trim"
same "11 the dock size is a slider of 40 to 64" "$(jq -c '.items[] | select(.id == "dock-size") | [.min, .max]' <<<"${control}")" "[40,64]"
in_session "ame desktop dock size 52" >/dev/null
in_session "ame desktop glass-strength blur 80" >/dev/null
in_session "ame desktop resting-screen after 10" >/dev/null
in_session "ame desktop living-wallpaper rain" >/dev/null
in_session "ame desktop digest wait ${APP}.desktop" >/dev/null
in_session "ame setup save" >/dev/null 2>&1
material="$(gs get "${GLASS}" material | tr -d "'")"
same "11 the setup file carries them" \
    "$(jq -c --arg m "blur-${material}" '[.settings.dock["icon-size"], .settings.glass[$m], .settings.rest.after, .settings.living.wallpaper, .settings.pill["digest-wait"]]' "${HOME_DIR}/.config/amethystora/setup.json")" \
    "[52,0.8,600,\"rain\",[\"${APP}.desktop\"]]"
reset_all

# --- 12: and nothing threw --------------------------------------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: (the overview's workspace names|Trim|the rest|the living|the dock|the glass)" | head -n 20)"
if [[ -z ${errors} ]]; then ok "12 nothing threw"; else not_ok "12 something threw" "${errors}"; fi
finish
