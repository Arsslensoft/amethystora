#!/usr/bin/bash
# vmlab-vm: glass-comfort
# The glass's three kinds, its continuous corners and its contrast guard, and Comfort (plans/beyond/spec-glass.md,
# section 9.2), in the test account's session, asked of GNOME Shell itself through Eval, which a lab-only extension of
# the test account opens (never in the image):
#   - Prism by default; ame desktop glass frost|prism|clear changes the picture and every surface's material at once;
#   - continuous corners on the shell's panes, circular ones on windows;
#   - text on the glass keeps 4.5:1 over the brightest wallpaper the image ships (dark theme) and the darkest (light);
#   - Reduce transparency, High Contrast and the power saver each make the glass solid and free its pictures, and the
#     power saver halves the animations' time, all within a second, and back;
#   - Reduce motion is GNOME's own Reduced Motion, and GNOME's Animations switch counts as it too;
#   - ten disables and enables leave nothing behind, and nothing of Amethystora UI throws.
# The lab draws without a GPU, where GNOME turns animations off by itself: the suite forces them on
# (global.force_animations) and puts the switch back. It leaves the account as it found it: Prism, not solid, Reduced
# Motion and High Contrast GNOME's defaults, theme amethystora, the image's wallpaper, the profile balanced.
# vmlab-paths: @ui/modules/pill/banners.js @ui/modules/pill/drop.js @ui/extension.js @ui/*glass* @ui/*comfort* @ui/*style* @sf/*themed/shell-glass* @sf/*themed/shell-styles* @sf/*just/desktop.just @ui/*pill*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
GLASS=org.gnome.shell.extensions.amethystora-ui.glass
A11Y=org.gnome.desktop.a11y.interface
SHOTS=/run/vmlab-13
# The value Eval gives back, a string as the expression made it
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
G="${UI}.get('glass')"
C="${UI}.get('comfort')"
# The first actor under the stage with a style class
FIND="const find = (a, c) => { if (a.has_style_class_name?.(c)) return a; for (const ch of a.get_children()) { const r = find(ch, c); if (r) return r; } return null; };"
profile() {
    busctl --system set-property org.freedesktop.UPower.PowerProfiles /org/freedesktop/UPower/PowerProfiles \
        org.freedesktop.UPower.PowerProfiles ActiveProfile s "$1"
}
wallpaper() {
    gs set org.gnome.desktop.background picture-uri "file:///usr/share/backgrounds/amethystora/$1"
    gs set org.gnome.desktop.background picture-uri-dark "file:///usr/share/backgrounds/amethystora/$1"
}
# The glass picture's measurement, numbered: a new picture is measured once it is a greater number than before
stats_version() { ev "${G}.frost?.picture(0, 'glass')?.stats?.version ?? 0"; }
stats_after() { [[ "$(stats_version)" -gt "$1" ]]; }
# within SECONDS COMMAND...: until the command succeeds, asked every tenth of a second, or false after SECONDS
within() {
    local deadline=$(($(date +%s%N) + $1 * 1000000000))
    shift
    until "$@" >/dev/null 2>&1; do
        (($(date +%s%N) < deadline)) || return 1
        sleep 0.1
    done
}
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi

gs reset "${GLASS}" material
gs reset "${GLASS}" solid
gs reset "${A11Y}" reduced-motion
gs reset "${A11Y}" high-contrast
profile balanced 2>/dev/null

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
ui_ready() { [[ "$(ev "typeof ${G}?.material")" == string ]]; }
if ! wait_for 240 has_session || ! wait_for 120 ui_ready; then
    not_ok "Amethystora UI did not start in the session"
    finish
fi
sleep 10
# The lab draws without a GPU, where GNOME turns its animations off by itself, and comfort then counts reduce motion
# as on (the manual says so). Animations are forced on for the suite, as GNOME's own tests do, and put back after.
check "GNOME Shell has its force-animations switch" test "$(ev "typeof global.force_animations")" == boolean
FORCED="$(ev "global.force_animations")"
ev "global.force_animations = true; 'forced'" >/dev/null
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme amethystora" >/dev/null 2>&1
ev "Main.overview.hide(); 'hidden'" >/dev/null
install -d -m 0755 "${SHOTS}"

# Two Files windows, the second used: the first is on glass
user_bus systemd-run --user --collect --unit="vmlab-files1-$$" nautilus --new-window >/dev/null
sleep 4
user_bus systemd-run --user --collect --unit="vmlab-files2-$$" nautilus --new-window >/dev/null
two_files() { [[ "$(ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').length")" == 2 ]]; }
wait_for 60 two_files
UNFOCUSED="global.get_window_actors().find(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus' && !a.meta_window.has_focus())"
python3 "$(dirname "$0")/keys.py" move:0.01,0.99 >/dev/null 2>&1

# --- 1-4: the variants on every surface ----------------------------------------------------------------------------
same "1 Prism by default, not solid" "$(ev "${G}.material + ' ' + ${G}.solid")" "prism false"
# What each listed surface's glass is made of, as variant:material, or none
PANE="const e = a => a?.get_effect('amethystora-glass'); const p = a => e(a)?.enabled ? (e(a).params?.variant + ':' + e(a).params?.material) : 'none';"
material_of() { ev "(() => { ${FIND} ${PANE} return [p(Main.panel), p(${UI}.get('dock')?.glassSurfaces?.()[0]?.actor), p(find(global.stage, 'quick-settings')), p(${UNFOCUSED})].join(' '); })()"; }
for v in frost prism clear; do
    check "2 ame desktop glass ${v}" in_session "ame desktop glass ${v}"
    is_v() { [[ "$(ev "${G}.material")" == "$1" ]]; }
    check "2 ${v}: the glass follows within 2 s" within 2 is_v "${v}"
    case "${v}" in frost) want="48 4" ;; prism) want="33 4" ;; clear) want="13 2" ;; esac
    same "2 ${v}: its picture has radius and shrink ${want}" \
        "$(ev "(() => { const p = ${G}.frost.picture(0, 'glass'); return p.radius + ' ' + p.shrink; })()")" "${want}"
    ev "Main.panel.statusArea.quickSettings.menu.open(); 'open'" >/dev/null
    sleep 2
    same "2 ${v}: the bar, dock, quick settings and the window not in use" "$(material_of)" \
        "${v}:menu-bar ${v}:dock ${v}:glass ${v}:pane"
    user_bus gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
        --method org.gnome.Shell.Screenshot.Screenshot false false "/tmp/vmlab-glass-${v}.png" >/dev/null 2>&1 &&
        mv "/tmp/vmlab-glass-${v}.png" "${SHOTS}/glass-${v}.png" 2>/dev/null
    if [[ ${v} == prism ]]; then
        corner="$(ev "(() => { ${FIND} const q = find(global.stage, 'quick-settings'); const r = q.get_theme_node().get_border_radius(0); const [s, n] = q.get_effect('amethystora-glass').params.corner; return (n > 2.76 && n < 2.78 && Math.abs(s - 1.4 * r) <= 0.5) + ' ' + ${UNFOCUSED}.get_effect('amethystora-glass').params.corner[1]; })()")"
        same "4 the quick settings' corners are continuous (n 2.77, s 1.4 r), the window's circular" "${corner}" "true 2"
    fi
    ev "Main.panel.statusArea.quickSettings.menu.close(); Main.panel.statusArea.dateMenu.menu.open(); 'open'" >/dev/null
    sleep 2
    same "2 ${v}: the date menu" "$(ev "(() => { ${FIND} ${PANE} return p(find(global.stage, 'datemenu-popover')); })()")" "${v}:glass"
    ev "Main.panel.statusArea.dateMenu.menu.close(); 'closed'" >/dev/null
    in_session "notify-send 'Test Glass' 'A banner on ${v} glass'" >/dev/null 2>&1
    sleep 2
    same "2 ${v}: a banner" "$(ev "(() => { ${PANE} return p(Main.messageTray._bannerBin.get_children()[0]); })()")" "${v}:glass"
    ev "Main.overview.show(); 'shown'" >/dev/null
    sleep 2
    same "2 ${v}: the overview stands on the frost" "$(ev "(() => { ${PANE} return p(Main.layoutManager.overviewGroup); })()")" "${v}:frost"
    ev "Main.overview.hide(); 'hidden'" >/dev/null
    sleep 2
done

# --- 5-9: text keeps 4.5:1 on the glass -----------------------------------------------------------------------------
# The lowest contrast of a surface's text against its glass, its children hidden, over a grid of points
CONTRAST="(() => {
    const {Shell, GLib} = imports.gi; ${FIND}
    const lin = v => v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    const Y = c => 0.2126 * lin(c[0] / 255) + 0.7152 * lin(c[1] / 255) + 0.0722 * lin(c[2] / 255);
    const ratio = (a, b) => (Math.max(Y(a), Y(b)) + 0.05) / (Math.min(Y(a), Y(b)) + 0.05);
    const wait = ms => new Promise(r => GLib.timeout_add(0, ms, () => { r(); return false; }));
    const shot = new Shell.Screenshot();
    const pick = (x, y) => new Promise(r => shot.pick_color(x, y, (o, res) => { const [, c] = shot.pick_color_finish(res); r([c.red, c.green, c.blue]); }));
    const measure = async (actor, text, points) => {
        const kids = actor.get_children();
        for (const k of kids) k.opacity = 0;
        await wait(400);
        const [x, y] = actor.get_transformed_position();
        const [w, h] = actor.get_transformed_size();
        const f = text.get_theme_node().get_foreground_color();
        let min = 99;
        for (const [a, b] of points) min = Math.min(min, ratio([f.red, f.green, f.blue], await pick(Math.round(x + a * w), Math.round(y + b * h))));
        for (const k of kids) k.opacity = 255;
        return min.toFixed(2);
    };
    globalThis.vmlabContrast = null;
    (async () => {
        Main.panel.statusArea.quickSettings.menu.open();
        await wait(1000);
        const grid = [];
        for (const a of [0.15, 0.325, 0.5, 0.675, 0.85]) for (const b of [0.15, 0.325, 0.5, 0.675, 0.85]) grid.push([a, b]);
        const qs = await measure(find(global.stage, 'quick-settings'), find(global.stage, 'quick-settings'), grid);
        Main.panel.statusArea.quickSettings.menu.close();
        await wait(600);
        // Behind the clock's own text: the bar's pane in the floating style, the clock's pill in the islands one
        // (amethystora-light's), where the wallpaper between the pills carries no text
        const date = Main.panel.statusArea.dateMenu;
        const bar = await measure(date, date, [0.1, 0.3, 0.5, 0.7, 0.9].map(a => [a, 0.5]));
        globalThis.vmlabContrast = qs + ' ' + bar;
    })().catch(e => { globalThis.vmlabContrast = 'error ' + e; });
    return 'started';
})()"
contrast_done() { [[ "$(ev "globalThis.vmlabContrast !== null")" == true ]]; }
contrast_of() {
    ev "${CONTRAST}" >/dev/null
    wait_for 30 contrast_done
    ev "globalThis.vmlabContrast"
}
# Windows away, so that the bar is the glass's
user_systemctl stop "vmlab-files1-$$" "vmlab-files2-$$" 2>/dev/null
sleep 2
readable() {
    local label="$1" v qs bar before
    for v in frost prism clear; do
        before="$(stats_version)"
        in_session "ame desktop glass ${v}" >/dev/null 2>&1
        within 5 stats_after "${before}" || not_ok "${label} ${v}: the new picture was not measured within 5 s"
        sleep 1
        read -r qs bar <<<"$(contrast_of)"
        if awk -v a="${qs}" -v b="${bar}" 'BEGIN { exit !(a >= 4.5 && b >= 4.5) }'; then
            ok "${label} ${v}: the quick settings ${qs}:1, behind the clock ${bar}:1"
        else
            not_ok "${label} ${v}: text under 4.5:1" "the quick settings ${qs}:1, behind the clock ${bar}:1"
        fi
    done
}
before="$(stats_version)"
wallpaper amethystora-facets-l.png
check "5 the brightest wallpaper's picture is measured within 3 s" within 3 stats_after "${before}"
readable "6-7 dark theme over amethystora-facets-l.png,"
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme amethystora-light" >/dev/null 2>&1
before="$(stats_version)"
wallpaper matte-black/night-sea.png
within 5 stats_after "${before}" || not_ok "8 the darkest wallpaper's picture was not measured within 5 s"
readable "8 light theme over matte-black/night-sea.png,"
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop theme amethystora" >/dev/null 2>&1

# The styles whose bar pieces have a background of their own, or none (capsule's bare bar), in the dark theme over
# the brightest wallpaper: the clock and the network meter's text, each against what is right behind it
BAR="(() => {
    const {Shell, GLib, St} = imports.gi;
    const lin = v => v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    const Y = c => 0.2126 * lin(c[0] / 255) + 0.7152 * lin(c[1] / 255) + 0.0722 * lin(c[2] / 255);
    const ratio = (a, b) => (Math.max(Y(a), Y(b)) + 0.05) / (Math.min(Y(a), Y(b)) + 0.05);
    const wait = ms => new Promise(r => GLib.timeout_add(0, ms, () => { r(); return false; }));
    const shot = new Shell.Screenshot();
    const pick = (x, y) => new Promise(r => shot.pick_color(x, y, (o, res) => { const [, c] = shot.pick_color_finish(res); r([c.red, c.green, c.blue]); }));
    const label = a => { if (a instanceof St.Label) return a; for (const c of a.get_children()) { const r = label(c); if (r) return r; } return null; };
    const measure = async actor => {
        const f = (label(actor) ?? actor).get_theme_node().get_foreground_color();
        const kids = actor.get_children();
        for (const k of kids) k.opacity = 0;
        await wait(400);
        const [x, y] = actor.get_transformed_position();
        const [w, h] = actor.get_transformed_size();
        let min = 99;
        for (const a of [0.2, 0.35, 0.5, 0.65, 0.8])
            min = Math.min(min, ratio([f.red, f.green, f.blue], await pick(Math.round(x + a * w), Math.round(y + h / 2))));
        for (const k of kids) k.opacity = 255;
        return min.toFixed(2);
    };
    globalThis.vmlabBar = null;
    (async () => {
        const s = Main.panel.statusArea;
        const clock = await measure(s.dateMenu);
        const meter = s['amethystora-meters-network'];
        globalThis.vmlabBar = clock + ' ' + (meter ? await measure(meter) : 'none');
    })().catch(e => { globalThis.vmlabBar = 'error ' + e; });
    return 'started';
})()"
bar_done() { [[ "$(ev "globalThis.vmlabBar !== null")" == true ]]; }
before="$(stats_version)"
wallpaper amethystora-facets-l.png
within 5 stats_after "${before}" || not_ok "8 the brightest wallpaper's picture was not measured again within 5 s"
for style in islands capsule segments neon; do
    in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop style ${style}" >/dev/null 2>&1
    for v in frost prism clear; do
        in_session "ame desktop glass ${v}" >/dev/null 2>&1
        sleep 3
        ev "${BAR}" >/dev/null
        wait_for 30 bar_done
        read -r clock meter <<<"$(ev "globalThis.vmlabBar")"
        if awk -v a="${clock}" -v b="${meter}" 'BEGIN { exit !(a >= 4.5 && (b == "none" || b >= 4.5)) }'; then
            ok "8 dark theme over amethystora-facets-l.png, ${style} ${v}: the clock ${clock}:1, the network meter ${meter}:1"
        else
            not_ok "8 dark theme over amethystora-facets-l.png, ${style} ${v}: text under 4.5:1" \
                "the clock ${clock}:1, the network meter ${meter}:1"
        fi
    done
done
in_session "AMETHYSTORA_THEME_QUIET=1 ame desktop style theme" >/dev/null 2>&1
in_session "ame desktop glass prism" >/dev/null 2>&1
before="$(stats_version)"
wallpaper amethystora-d.png
within 5 stats_after "${before}" || not_ok "9 the dark wallpaper's picture was not measured within 5 s"
sleep 1
ev "Main.panel.statusArea.quickSettings.menu.open(); 'open'" >/dev/null
sleep 2
# The guard thickens the veil only as far as text needs: on the image's own dark wallpaper the readback of the
# blurred picture asks for a hair more than Prism's 0.14 at most (0.155 in the harness)
veil="$(ev "(() => { ${FIND} return find(global.stage, 'quick-settings').get_effect('amethystora-glass').params.veil; })()")"
if awk -v v="${veil}" 'BEGIN { exit !(v >= 0.14 && v <= 0.16) }'; then ok "9 on amethystora-d.png Prism's veil stays its own ($veil)"
else not_ok "9 on amethystora-d.png Prism's veil stays its own" "found: ${veil}"; fi

# --- 10-12: Reduce transparency -------------------------------------------------------------------------------------
SOLID_STATE="(() => { ${FIND} const q = find(global.stage, 'quick-settings'); return [${G}.solid, Main.layoutManager.uiGroup.has_style_class_name('amethystora-glass-solid'), ${G}.frost === null, global.stage.get_children().some(a => a.name === 'amethystoraGlassPictures'), [Main.panel, q].some(a => !!a?.get_effect('amethystora-glass'))].join(' '); })()"
check "10 ame desktop comfort reduce-transparency on" in_session "ame desktop comfort reduce-transparency on"
is_solid() { [[ "$(ev "${SOLID_STATE}")" == "true true true false false" ]]; }
check "10 within 1 s the glass is solid, its pictures and panes gone" within 1 is_solid
popover="$(sed -n '/amethystora-glass-solid .amethystora-glass-surface,/,/}/s/.*background-color: \(#[0-9a-fA-F]\{6\}\);.*/\1/p' \
    "${HOME_DIR}/.config/amethystora/current/theme/shell-glass.css" | head -n1)"
same "11 the quick settings are the theme's popover colour" \
    "$(ev "(() => { ${FIND} const c = find(global.stage, 'quick-settings').get_theme_node().get_background_color(); return '#' + [c.red, c.green, c.blue].map(v => v.toString(16).padStart(2, '0')).join(''); })()")" \
    "${popover,,}"
PICK="(() => {
    const {Shell, GLib} = imports.gi; ${FIND}
    const q = find(global.stage, 'quick-settings');
    for (const k of q.get_children()) k.opacity = 0;
    globalThis.vmlabPick = null;
    GLib.timeout_add(0, 400, () => {
        const [x, y] = q.get_transformed_position();
        const [w, h] = q.get_transformed_size();
        const shot = new Shell.Screenshot();
        shot.pick_color(Math.round(x + w / 2), Math.round(y + h / 2), (o, res) => {
            const [, c] = shot.pick_color_finish(res);
            for (const k of q.get_children()) k.opacity = 255;
            globalThis.vmlabPick = [c.red, c.green, c.blue].join(' ');
        });
        return false;
    });
    return 'started';
})()"
picked() { [[ "$(ev "globalThis.vmlabPick !== null")" == true ]]; }
ev "${PICK}" >/dev/null
wait_for 10 picked
read -r r g b <<<"$(ev "globalThis.vmlabPick")"
want="${popover#\#}"
close_to() { local d=$(($1 - 16#$2)); ((d >= -2 && d <= 2)); }
if close_to "${r:-0}" "${want:0:2}" && close_to "${g:-0}" "${want:2:2}" && close_to "${b:-0}" "${want:4:2}"; then
    ok "11 its centre is drawn in that colour (${r} ${g} ${b})"
else
    not_ok "11 its centre is drawn in that colour" "expected: ${popover}" "found: ${r} ${g} ${b}"
fi
check "12 ame desktop comfort reduce-transparency off" in_session "ame desktop comfort reduce-transparency off"
is_glass() { [[ "$(ev "${SOLID_STATE}")" == "false false false true true" && "$(ev "${G}.frost.picture(0, 'glass').ready")" == true ]]; }
check "12 within 2 s the glass and its panes are back" within 2 is_glass

# --- 13: Increase contrast ------------------------------------------------------------------------------------------
check "13 ame desktop comfort increase-contrast on" in_session "ame desktop comfort increase-contrast on"
same "13 GNOME's High Contrast is on" "$(gs get "${A11Y}" high-contrast)" true
contrast_on() { [[ "$(ev "${C}.highContrast + ' ' + ${G}.solid + ' ' + Main.layoutManager.uiGroup.has_style_class_name('amethystora-glass-contrast')")" == "true true true" ]]; }
check "13 within 1 s the glass is solid and outlined" within 1 contrast_on
same "13 the quick settings have a 2 px border in their text colour" \
    "$(ev "(() => { ${FIND} const n = find(global.stage, 'quick-settings').get_theme_node(); const b = n.get_border_color(0), f = n.get_foreground_color(); return n.get_border_width(0) + ' ' + (b.red === f.red && b.green === f.green && b.blue === f.blue); })()")" "2 true"
in_session "ame desktop comfort increase-contrast off" >/dev/null 2>&1
contrast_off() { [[ "$(ev "${C}.highContrast + ' ' + ${G}.solid")" == "false false" ]]; }
check "13 off again within 1 s" within 1 contrast_off
ev "Main.panel.statusArea.quickSettings.menu.close(); 'closed'" >/dev/null

# --- 14-15: the power saver -----------------------------------------------------------------------------------------
if busctl --system get-property org.freedesktop.UPower.PowerProfiles /org/freedesktop/UPower/PowerProfiles \
    org.freedesktop.UPower.PowerProfiles ActiveProfile >/dev/null 2>&1; then
    profile power-saver
    saver_on() { [[ "$(ev "${C}.powerSaver + ' ' + ${G}.solid + ' ' + imports.gi.St.Settings.get().slow_down_factor")" == "true true 0.3" ]]; }
    check "14 power saver: within 1 s solid, animations at 0.3" within 1 saver_on
    same "14 the glass's own key is untouched" "$(gs get "${GLASS}" solid)" false
    profile balanced
    saver_off() { [[ "$(ev "${C}.powerSaver + ' ' + ${G}.solid + ' ' + imports.gi.St.Settings.get().slow_down_factor")" == "false false 0.6" ]]; }
    check "15 balanced: within 1 s glass again, animations at 0.6" within 1 saver_off
else
    not_ok "14 tuned-ppd does not answer power-profiles-daemon's interface"
fi

# --- 16: Reduce motion ----------------------------------------------------------------------------------------------
check "16 ame desktop comfort reduce-motion on" in_session "ame desktop comfort reduce-motion on"
same "16 GNOME's Reduced Motion is reduce" "$(gs get "${A11Y}" reduced-motion)" "'reduce'"
motion() { [[ "$(ev "${C}.reduceMotion")" == "$1" ]]; }
check "16 comfort says reduce motion within 1 s" within 1 motion true
in_session "ame desktop comfort reduce-motion off" >/dev/null 2>&1
check "16 and no longer once it is off" within 1 motion false
gs set org.gnome.desktop.interface enable-animations false
check "16 GNOME's Animations off counts as reduce motion" within 2 motion true
gs set org.gnome.desktop.interface enable-animations true
check "16 and back with them" within 2 motion false
says_all() {
    local state
    state="$(in_session "ame desktop comfort")"
    grep -q "^Reduce transparency: off$" <<<"${state}" && grep -q "^Reduce motion: off$" <<<"${state}" &&
        grep -q "^Increase contrast: off$" <<<"${state}" && grep -q "^Power saver: off " <<<"${state}"
}
check "16 ame desktop comfort alone says how each is set" says_all

# --- 17: ten disables and enables -----------------------------------------------------------------------------------
CYCLE="(() => {
    const GLib = imports.gi.GLib;
    const uuid = '${UUID}';
    const wait = ms => new Promise(r => GLib.timeout_add(0, ms, () => { r(); return false; }));
    const state = () => Main.extensionManager.lookup(uuid).state;
    const holders = () => global.stage.get_children().filter(a => a.name === 'amethystoraGlassPictures').length;
    globalThis.vmlabCycle = null;
    (async () => {
        const seen = new Set();
        for (let i = 0; i < 10; i++) {
            const comfort = ${C};
            await Main.extensionManager.disableExtension(uuid);
            while (state() === 1) await wait(50);
            seen.add([holders(), !!comfort._proxy, imports.gi.St.Settings.get().slow_down_factor !== 0.6].join(' '));
            await Main.extensionManager.enableExtension(uuid);
            while (state() !== 1) await wait(50);
            await wait(500);
        }
        globalThis.vmlabCycle = [...seen].join(';') + ' / ' + holders();
    })().catch(e => { globalThis.vmlabCycle = 'error ' + e; });
    return 'started';
})()"
ev "${CYCLE}" >/dev/null
cycle_done() { [[ "$(ev "globalThis.vmlabCycle !== null")" == true ]]; }
wait_for 120 cycle_done
same "17 disabled ten times: no picture, no proxy, GNOME's speed; enabled: one holder" "$(ev "globalThis.vmlabCycle")" \
    "0 false true / 1"
if busctl --system get-property org.freedesktop.UPower.PowerProfiles /org/freedesktop/UPower/PowerProfiles \
    org.freedesktop.UPower.PowerProfiles ActiveProfile >/dev/null 2>&1; then
    profile power-saver
    within 2 saver_on
    ev "${CYCLE/i < 10/i < 1}" >/dev/null
    wait_for 60 cycle_done
    same "17 once more with the power saver on: the same, and solid and at 0.3 again after" \
        "$(ev "globalThis.vmlabCycle")" "0 false true / 0"
    check "17 the power saver applies again after the extension came back" within 3 saver_on
    profile balanced
    within 2 saver_off
fi

# --- 18-19: nothing threw, and the account as it was ----------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    awk -v uuid="${UUID}" '/JS ERROR/ { block = $0; next } block && /@(resource|file):\/\// { block = block "\n" $0; next }
        block { if (index(block, uuid)) print block; block = "" } /Amethystora UI: .*(failed|threw|could not)/ { print }
        END { if (block && index(block, uuid)) print block }' | head -n 20)"
if [[ -z ${errors} ]]; then ok "18 nothing of Amethystora UI threw"; else not_ok "18 Amethystora UI threw" "${errors}"; fi
gs reset "${GLASS}" material
gs reset "${GLASS}" solid
gs reset "${A11Y}" reduced-motion
gs reset "${A11Y}" high-contrast
gs reset org.gnome.desktop.interface enable-animations
gs reset org.gnome.desktop.background picture-uri
gs reset org.gnome.desktop.background picture-uri-dark
ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null
finish
