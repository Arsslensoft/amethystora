#!/usr/bin/bash
# vmlab-vm: pointer
# Amethystora's own pointers and shake to find (plans/beyond/spec-pointer.md, section 7), in the test account's session,
# asked of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - each theme writes its pointer (Halo dark, Ink light), a pointer chosen with ame desktop pointer stays across a
#     theme switch, and one the account set itself is never written over;
#   - Mutter loads every one of its 36 pointer shapes from the set, at 30 px for size 24 with the drawing's hot spot,
#     and 40 px at size 32; every name a toolkit asks for resolves through libXcursor to the set's own file;
#   - a quick shake grows the pointer once (or sends rings out), and the pointer comes back however the flare ends;
#   - a slow back-and-forth, a drag with the button held and a long glide do not fire, and with shaking off nothing is
#     even looked at;
#   - the dense-screen rule, as a pure function and applied to the account's cursor-size, never over the account's own;
#   - nothing of Amethystora UI throws meanwhile.
# It leaves the account as it found it: shake off, dense on, cursor-size reset, no pointer of its own, theme amethystora.
# vmlab-paths: @ui/extension.js @ui/*pointer* @sf/usr/share/icons/amethystora-pointer-* @sf/usr/lib/amethystora/theme/* @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
POINTER=org.gnome.shell.extensions.amethystora-ui.pointer
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
# The value Eval gives back, a string as the expression made it
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
P="${UI}.get('pointer')"
T="global.backend.get_cursor_tracker()"
pstat() { ev "${P}.stats.$1"; }
# Conditions a wait asks again each round (wait_for runs its command anew; a value in "$(...)" would be read once)
module_up() { [[ "$(ev "typeof ${P}?.stats")" == object ]]; }
types_read() { [[ "$(ev 'globalThis.vmlabTypes !== null')" == true ]]; }
locked_is() { [[ "$(loginctl show-session "${SESSION}" -p LockedHint --value)" == "$1" ]]; }
pointer_visible() { [[ "$(ev "${T}.get_pointer_visible()")" == true ]]; }
# A flare the test holds open: started, its own end (the hold) taken away, so that only the way out being tested can
# give the pointer back; says whether the pointer is hidden
pinned_flare() {
    ev "(() => { ${P}._onShake(); const f = ${P}._flare; if (f?.hold) { imports.gi.GLib.source_remove(f.hold); f.hold = 0; }
        return ${P}.stats.hidden + ' ' + ${T}.get_pointer_visible(); })()"
}
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"

# The account starts as the image leaves it: the size GNOME's, the module's keys their defaults
gs reset org.gnome.desktop.interface cursor-size
gs reset "${POINTER}" shake
gs reset "${POINTER}" dense
rm -f "${HOME_DIR}/.config/amethystora/pointer" "${HOME_DIR}/.local/state/amethystora/pointer.json"*

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
wait_for 240 has_session && wait_for 120 module_up
sleep 10
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"
check "the pointer module runs, with nothing listening: shaking is off by default" test "$(pstat connected)" == false

# --- V18 first: the lab's own screen is not dense, so the size the module may write is not there ---------------------
same "V18 the VM's screen is not dense: cursor-size has no value of the account's" \
    "$(user_bus dconf read /org/gnome/desktop/interface/cursor-size)" ""

# --- V1-V3: the pointer of each theme, one chosen, one of the account's own ------------------------------------------
for theme in amethystora tokyo-night amethystora-light rose-pine-dawn; do
    in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme ${theme}" >/dev/null 2>&1
    want=amethystora-pointer-halo
    [[ -f "/usr/share/amethystora/themes/${theme}/light.mode" ]] && want=amethystora-pointer-ink
    same "V1 ${theme} writes ${want}" "$(gs get org.gnome.desktop.interface cursor-theme)" "'${want}'"
done
in_session "ame desktop pointer facet" >/dev/null 2>&1
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme amethystora-light" >/dev/null 2>&1
same "V2 a pointer chosen with ame desktop pointer stays across a theme switch" \
    "$(gs get org.gnome.desktop.interface cursor-theme)" "'amethystora-pointer-facet'"
in_session "ame desktop pointer theme" >/dev/null 2>&1
same "V2 ame desktop pointer theme goes back to the theme's" "$(gs get org.gnome.desktop.interface cursor-theme)" \
    "'amethystora-pointer-ink'"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/icons/vmlab-pointer"
cp -r /usr/share/icons/Adwaita/cursors "${HOME_DIR}/.local/share/icons/vmlab-pointer/"
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/icons"
gs set org.gnome.desktop.interface cursor-theme vmlab-pointer
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme amethystora" >/dev/null 2>&1
same "V3 a pointer the account set itself is left as it is" "$(gs get org.gnome.desktop.interface cursor-theme)" "'vmlab-pointer'"
gs reset org.gnome.desktop.interface cursor-theme
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme amethystora" >/dev/null 2>&1
rm -rf "${HOME_DIR}/.local/share/icons/vmlab-pointer"
same "the theme writes Halo again once the key is reset" "$(gs get org.gnome.desktop.interface cursor-theme)" \
    "'amethystora-pointer-halo'"

# --- V4, V6: what Mutter loads, over the wallpaper of an empty workspace ----------------------------------------------
ev "Main.overview.hide(); global.workspace_manager.get_workspace_by_index(global.workspace_manager.n_workspaces - 1).activate(global.get_current_time()); 1" >/dev/null
sleep 2
keys move:0.5,0.6
# Each type set on the stage, a nudge of the pointer so that Mutter takes it, then its sprite read 150 ms later; the
# sprite answers only while the seat keeps its focus, which the probe holds meanwhile
probe() {
    ev "(() => { const C = imports.gi.Clutter, G = imports.gi.GLib; const seat = global.stage.context.get_backend().get_default_seat();
        globalThis.vmlabVp ??= seat.create_virtual_device(C.InputDeviceType.POINTER_DEVICE); const t = ${T};
        const names = Object.keys(C.CursorType).filter(n => /^[A-Z_]+$/.test(n) && n !== 'INHERIT' && n !== 'NONE'); const out = []; let i = 0;
        globalThis.vmlabTypes = null; seat.inhibit_unfocus(); const [p] = t.get_pointer();
        G.timeout_add(G.PRIORITY_DEFAULT, 150, () => {
            if (i > 0) { const s = t.get_sprite(); out.push(names[i - 1] + ':' + (s ? s.get_width() : 0) + '@' + t.get_hot().join(',')); }
            if (i >= names.length) { global.stage.set_cursor_type(C.CursorType.DEFAULT); seat.uninhibit_unfocus(); globalThis.vmlabTypes = out.join(' '); return false; }
            global.stage.set_cursor_type(C.CursorType[names[i++]]);
            globalThis.vmlabVp.notify_absolute_motion(G.get_monotonic_time(), p.x + (i % 2), p.y); return true; });
        return names.length; })()" >/dev/null
    wait_for 20 types_read
    ev 'globalThis.vmlabTypes'
}
expect_types() {
    local width="$1" arrow="$2" zoom="$3" centre="$4" zoom51="${5:-$3}" out="" entry name want
    for entry in $(probe); do
        name="${entry%%:*}"
        case "${name}" in
            DEFAULT | CONTEXT_MENU | HELP | POINTER | PROGRESS | ALIAS | COPY | NO_DROP | NOT_ALLOWED | DND_ASK) want="${width}@${arrow}" ;;
            ZOOM_IN | ZOOM_OUT) want="${width}@${zoom}" ;;
            *) want="${width}@${centre}" ;;
        esac
        # The magnifier's hot spot is at 13.5 units: GNOME 50 rounds it (14 at 24), GNOME 51 drops the fraction (13)
        [[ "${entry#*:}" == "${want}" || ("${want}" == "${width}@${zoom}" && "${entry#*:}" == "${width}@${zoom51:-}") ]] ||
            out+="${name}: ${entry#*:}, not ${want}; "
    done
    echo "${out}"
}
for set in halo ink facet; do
    gs set org.gnome.desktop.interface cursor-theme "amethystora-pointer-${set}"
    sleep 1
    wrong="$(expect_types 30 7,3 14,14 15,15 13,13)"
    if [[ -z "${wrong}" ]]; then ok "V4 Mutter loads all 36 shapes from ${set}, 30 px at 24, each with its hot spot"; else not_ok "V4 ${set}" "${wrong}"; fi
done
gs set org.gnome.desktop.interface cursor-theme amethystora-pointer-halo
gs set "${POINTER}" dense false
gs set org.gnome.desktop.interface cursor-size 32
sleep 1
wrong="$(expect_types 40 9,4 18,18 20,20)"
if [[ -z "${wrong}" ]]; then ok "V6 at size 32 every shape is 40 px, the arrow's hot spot 9,4"; else not_ok "V6 size 32" "${wrong}"; fi
gs reset org.gnome.desktop.interface cursor-size
gs set "${POINTER}" dense true
sleep 1
same "V6 back at 24, the arrow is 30 px with its hot spot at 7,3" "$(probe | tr ' ' '\n' | grep '^DEFAULT:')" "DEFAULT:30@7,3"

# --- V5: every name, through libXcursor, from the set itself (not from Adwaita, whose images are 24 px) ---------------
v5="$(python3 - <<'PY'
import ctypes, os
X = ctypes.CDLL('libXcursor.so.1')
class Img(ctypes.Structure):
    _fields_ = [('version', ctypes.c_uint), ('size', ctypes.c_uint), ('width', ctypes.c_uint), ('height', ctypes.c_uint),
                ('xhot', ctypes.c_uint), ('yhot', ctypes.c_uint), ('delay', ctypes.c_uint), ('pixels', ctypes.c_void_p)]
class Imgs(ctypes.Structure):
    _fields_ = [('nimage', ctypes.c_int), ('images', ctypes.POINTER(ctypes.POINTER(Img))), ('name', ctypes.c_char_p)]
X.XcursorLibraryLoadImages.restype = ctypes.POINTER(Imgs)
X.XcursorLibraryLoadImages.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int]
bad, n = [], 0
for s in ('halo', 'ink', 'facet'):
    d = f'/usr/share/icons/amethystora-pointer-{s}'
    names = sorted(os.listdir(d + '/cursors'))
    moving = {n for n in names if os.path.realpath(f'{d}/cursors/{n}').rsplit('/', 1)[1] in ('wait', 'progress')}
    for size, side in ((24, 30), (32, 40)):
        for name in names:
            n += 1
            r = X.XcursorLibraryLoadImages(name.encode(), f'amethystora-pointer-{s}'.encode(), size)
            want = 20 if name in moving else 1
            if not r or r.contents.nimage != want or r.contents.images[0].contents.width != side:
                bad.append(f'{s} {size} {name}')
print(n, len(bad), ' '.join(bad[:10]))
PY
)"
read -r lookups failed rest <<<"${v5}"
if [[ "${lookups}" == 696 && "${failed}" == 0 ]]; then ok "V5 all 116 names of each set resolve to its own images, at 24 and 32"; else not_ok "V5 libXcursor" "${v5}"; fi

# --- V7, V8, V14, V19: a shake grows the pointer once, and it comes back ---------------------------------------------
# The lab renders in software, so GNOME turns animations off; the swell and the rings are animations, so they are
# forced on for V7 to V12 and put back after
ev "global.force_animations = true; imports.gi.St.Settings.get().enable_animations" >/dev/null
check "animations are on for the swell" test "$(ev 'imports.gi.St.Settings.get().enable_animations')" == true
gs set "${POINTER}" shake grow
sleep 1
# The costs are counted from here, the handler warm
ev "Object.assign(${P}.stats, {handlerUsMax: 0, handlerUsSum: 0, frameUsMax: 0}); 1" >/dev/null
s0="$(pstat samples)"
f0="$(pstat fired)"
keys shake:0.5,0.5,0.15,6,110 >/dev/null
same "V7 a quick shake fires once" "$(pstat fired)" "$((f0 + 1))"
max="$(pstat maxScale)"
# The curve peaks at 3.59; a frame may miss the peak, never pass it
if awk -v m="${max}" 'BEGIN { exit !(m > 3.4 && m < 3.65) }'; then ok "V7 the pointer grew past 3.4 and settled (largest ${max})"; else not_ok "V7 maxScale" "${max}"; fi
sleep 3
same "V8 the pointer is back" "$(ev "${T}.get_pointer_visible() + ' ' + ${P}.stats.hidden + ' ' + ${P}.stats.actors")" "true false 0"
# What a look costs, timed alone: 2000 looks in one Eval, which nothing interrupts. The handler's own clock during a
# shake also counts whatever else the machine did between its two readings (in the lab, software rendering on every
# core while the swell is forced to animate): round 1 read 291 µs on average and 6.6 ms at most there, for code that
# takes about 40 µs in the headless shell. That reading is only reported.
look="$(ev "(() => { const p = ${P}, G = imports.gi.GLib; const t0 = G.get_monotonic_time();
    for (let i = 0; i < 2000; i++) p._look(t0 + i * 8000); const us = (G.get_monotonic_time() - t0) / 2000;
    p._detector.reset(); return us.toFixed(1); })()")"
handler="$(ev "Math.round(${P}.stats.handlerUsSum / Math.max(1, ${P}.stats.samples - ${s0}))")"
if awk -v l="${look}" 'BEGIN { exit !(l > 0 && l < 50) }'; then
    ok "V14 a look at a move takes ${look} µs (during the shake, with the rendering: ${handler} µs on average)"
else
    not_ok "V14 a look costs ${look} µs" "during the shake: ${handler} µs on average, $(pstat handlerUsMax) µs at most"
fi
frame="$(pstat frameUsMax)"
if awk -v f="${frame}" 'BEGIN { exit !(f < 1000) }'; then ok "V19 a frame of the swell takes ${frame} µs at most"; else not_ok "V19 frame cost" "${frame} µs"; fi

# --- V9-V11: what is not a shake --------------------------------------------------------------------------------------
f0="$(pstat fired)"
keys shake:0.5,0.5,0.15,6,450 >/dev/null
same "V9 a slow back-and-forth does not fire" "$(pstat fired)" "${f0}"
keys shake:0.5,0.5,0.15,6,110,held >/dev/null
same "V10 a drag with the button held does not fire" "$(pstat fired)" "${f0}"
same "V10 the pointer is visible after the drag" "$(ev "${T}.get_pointer_visible()")" true
keys glide:0.05,0.5,0.95,0.5,500 glide:0.95,0.5,0.05,0.5,500 >/dev/null
same "V11 a long straight glide there and back does not fire" "$(pstat fired)" "${f0}"

# --- V12: beacon --------------------------------------------------------------------------------------------------
in_session "ame desktop pointer-shake beacon" >/dev/null 2>&1
sleep 1
f0="$(pstat fired)"
keys shake:0.5,0.5,0.15,6,110 >/dev/null
same "V12 beacon fires once" "$(pstat fired)" "$((f0 + 1))"
same "V12 three rings went out" "$(pstat ringsMax)" 3
max="$(pstat maxScale)"
if awk -v m="${max}" 'BEGIN { exit !(m > 1.7 && m < 1.8) }'; then ok "V12 the pointer grew a little (largest ${max})"; else not_ok "V12 maxScale" "${max}"; fi
sleep 3
same "V12 the rings are gone and the pointer is back" "$(ev "${P}.stats.rings + ' ' + ${T}.get_pointer_visible()")" "0 true"
ev "global.force_animations = true; 1" >/dev/null

# --- V13: off costs nothing --------------------------------------------------------------------------------------------
in_session "ame desktop pointer-shake off" >/dev/null 2>&1
sleep 1
same "V13 with shaking off nothing listens to the pointer" "$(pstat connected)" false
s0="$(pstat samples)"
keys shake:0.5,0.5,0.15,6,110 >/dev/null
same "V13 a shake is not even looked at" "$(pstat samples)" "${s0}"
in_session "ame desktop pointer-shake grow" >/dev/null 2>&1
sleep 1
same "V13 grow listens again" "$(pstat connected)" true

# --- V15: the pointer comes back however a flare ends --------------------------------------------------------------
# Each flare is held open by the test (pinned_flare), so the pointer can only come back by the way out being checked
keys move:0.5,0.6 >/dev/null
same "V15a a held flare hides the pointer" "$(pinned_flare)" "true false"
in_session "gnome-extensions disable ${UUID}" >/dev/null 2>&1
check "V15a turning the extension off gives the pointer back" wait_for 10 pointer_visible
in_session "gnome-extensions enable ${UUID}" >/dev/null 2>&1
wait_for 30 module_up
same "V15b a held flare hides the pointer" "$(pinned_flare)" "true false"
loginctl lock-session "${SESSION}"
check "V15b the screen locks" wait_for 30 locked_is yes
sleep 2
loginctl unlock-session "${SESSION}"
wait_for 30 locked_is no
wait_for 30 module_up
check "V15b after a lock and unlock the pointer is visible" wait_for 10 pointer_visible
same "V15b and the new module hides nothing" "$(pstat hidden)" false
same "V15c a held flare hides the pointer" "$(pinned_flare)" "true false"
gs set "${POINTER}" shake off
check "V15c turning shaking off mid-flare gives the pointer back" wait_for 10 pointer_visible
same "V15c and the copy is gone" "$(pstat actors)" 0
gs reset "${POINTER}" shake

# --- V16, V17: the dense-screen rule --------------------------------------------------------------------------------
dense() { ev "String(${P}.denseSize({widthPx: $1, heightPx: $2, widthMm: $3, heightMm: $4, scale: $5, tiled: false}))"; }
same "V16 14-inch 2560x1600 at 100% is dense: 32" "$(dense 2560 1600 302 189 1)" 32
same "V16 the same at 200% is left alone" "$(dense 2560 1600 302 189 2)" null
same "V16 14-inch 1920x1200 is not dense" "$(dense 1920 1200 302 189 1)" null
same "V16 27-inch 4K is dense: 32" "$(dense 3840 2160 597 336 1)" 32
same "V16 27-inch 2560x1440 is not dense" "$(dense 2560 1440 597 336 1)" null
same "V16 a size that is the screen's shape (160x90 mm) is ignored" "$(dense 1920 1080 160 90 1)" null
same "V16 a screen of no size is ignored" "$(dense 1920 1080 0 0 1)" null
DENSE='{widthPx: 2560, heightPx: 1600, widthMm: 302, heightMm: 189, scale: 1, tiled: false}'
PLAIN='{widthPx: 1920, heightPx: 1200, widthMm: 302, heightMm: 189, scale: 1, tiled: false}'
gs reset org.gnome.desktop.interface cursor-size
ev "${P}.applyDense(${DENSE}); 1" >/dev/null
same "V17 a dense screen writes 32" "$(gs get org.gnome.desktop.interface cursor-size)" 32
same "V17 and the state file says the module wrote it" \
    "$(jq -c . "${HOME_DIR}/.local/state/amethystora/pointer.json" 2>/dev/null)" '{"wrote":32}'
ev "${P}.applyDense(${PLAIN}); 1" >/dev/null
same "V17 a plain screen takes the module's size back" "$(user_bus dconf read /org/gnome/desktop/interface/cursor-size)" ""
gs set org.gnome.desktop.interface cursor-size 48
ev "${P}.applyDense(${DENSE}); 1" >/dev/null
same "V17 a size the account chose (48) is kept on a dense screen" "$(gs get org.gnome.desktop.interface cursor-size)" 48
gs set org.gnome.desktop.interface cursor-size 24
ev "${P}.applyDense(${DENSE}); 1" >/dev/null
same "V17 so is an explicit 24" "$(gs get org.gnome.desktop.interface cursor-size)" 24
gs reset org.gnome.desktop.interface cursor-size

# --- Leave the account as it was -------------------------------------------------------------------------------------
gs reset "${POINTER}" shake
gs reset "${POINTER}" dense
rm -f "${HOME_DIR}/.config/amethystora/pointer"
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme amethystora" >/dev/null 2>&1
ev "global.stage.set_cursor_type(imports.gi.Clutter.CursorType.DEFAULT); 1" >/dev/null

errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    awk -v uuid="${UUID}" '/JS ERROR/ { block = $0; next } block && /@(resource|file):\/\// { block = block "\n" $0; next }
        block { if (index(block, uuid)) print block; block = "" } /Amethystora UI: .*(failed|threw|pointer)/ { print }
        END { if (block && index(block, uuid)) print block }' | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of Amethystora UI threw"; else not_ok "Amethystora UI threw" "${errors}"; fi
critical="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "cursor_visibility_inhibitors|uninhibit_unfocus|unfocus_inhibit" | head -n 5)"
if [[ -z ${critical} ]]; then ok "Mutter's pointer counters stayed even"; else not_ok "Mutter complained about the pointer's counters" "${critical}"; fi
finish
