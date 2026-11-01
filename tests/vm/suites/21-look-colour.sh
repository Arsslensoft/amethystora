#!/usr/bin/bash
# vmlab-vm: look
# The colour half of the look (plans/beyond/spec-look.md, section 7, criteria 1 to 26), in the test account's session:
#   - the twelve accents in a dark and a light theme, in every file the theme writes, GNOME's accent-color and the shell;
#     every accent in two themes, Follow the theme, and a bad accent refused, by the recipe and by the theme switch;
#   - Daylight: the slideshow written and shown, parsed by GNOME's own reader, the picture of each hour with the clock
#     set, Night Light's hours, a picture of one's own kept, the opening picture adopted, off and on, a light theme's;
#     every theme's four pictures there, its own, at size, darker by night than by day;
#   - colours from the wallpaper: written, merged key by key, each way, a light theme kept light, worked out again when
#     GNOME's Settings changes the picture and not when nothing changed, off, a chosen accent over them, the helper's
#     refusals;
#   - Control's theme preview follows the swatch with the keyboard's focus;
#   - an accent per workspace: the sets, the switch, the state file the apps read, Notes taking it, one workspace's
#     accent, off is off.
# It leaves the account on the Amethystora theme, no accent, no colours, Daylight on, accent per workspace off.
# vmlab-paths: @ui/extension.js @ui/*theme* @ui/*workspaces* @sf/usr/libexec/amethystora-wallpaper-colours @sf/usr/bin/amethystora-theme @sf/usr/lib/amethystora/theme/* @sf/*themed/* @sf/usr/share/amethystora/themes/* @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
OWN=/run/vmlab-21
HOME_DIR="/var/home/${TEST_USER}"
CFG="${HOME_DIR}/.config/amethystora"
CUR="${CFG}/current"
STATE="${HOME_DIR}/.local/state/amethystora/workspace-accent"
THEME_UI=org.gnome.shell.extensions.amethystora-ui.theme
COLOR=org.gnome.settings-daemon.plugins.color
B=/usr/share/backgrounds/amethystora
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
ame_() { in_session "AMETHYSTORA_THEME_QUIET=1 ame $*" 2>&1; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
key() { sed -nE "s/^$2 = \"(#[0-9a-f]{6})\"\$/\1/p" "$1" | head -n1; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
TH="${UI}.get('theme')"
# Where the journal stood when the suite began, by its cursor: the clock is moved below, which a time would not survive
CURSOR="$(journalctl -n 1 -o cat --show-cursor -q 2>/dev/null | sed -n 's/^-- cursor: //p')"

# OKLab of a colour (L C) and the mean lightness of a picture's 64x64 copy, by the helper's own arithmetic
oklab() {
    python3 -I -c 'import sys, math
def lin(v):
    v /= 255
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
h = sys.argv[1]
r, g, b = (lin(int(h[i:i + 2], 16)) for i in (1, 3, 5))
l, m, s = (math.copysign(abs(x) ** (1 / 3), x) for x in (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b,
    0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b, 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b))
L, a, bb = (0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s)
print("%.4f %.4f" % (L, math.hypot(a, bb)))' "$1"
}
ratio() {
    python3 -I -c 'import sys
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    c = [v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4 for v in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
x, y = sorted((lum(sys.argv[1]), lum(sys.argv[2])), reverse=True)
print("%.2f" % ((x + 0.05) / (y + 0.05)))' "$1" "$2"
}
mean_l() {
    as_user "python3 -I ${OWN}/mean.py $1" 2>/dev/null
}
module_up() { [[ "$(ev "typeof ${TH}")" == object ]]; }

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
mkdir -p "${OWN}"
cp "$(dirname "$0")/atspi.py" "${OWN}/"
cat >"${OWN}/mean.py" <<'EOF'
import sys
import gi
gi.require_version('GdkPixbuf', '2.0')
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file_at_scale(sys.argv[1], 64, 64, False)
data, n, stride = p.get_pixels(), p.get_n_channels(), p.get_rowstride()
def lin(v):
    v /= 255
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
total = 0
for y in range(64):
    for x in range(64):
        i = y * stride + x * n
        r, g, b = lin(data[i]), lin(data[i + 1]), lin(data[i + 2])
        l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ** (1 / 3)
        m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ** (1 / 3)
        s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ** (1 / 3)
        total += 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
print("%.4f" % (total / 4096))
EOF
# GNOME's own reader of a slideshow, as the shell's background takes it (gnome-desktop's GnomeBG)
cat >"${OWN}/slideshow.js" <<'EOF'
import Gio from 'gi://Gio';
import GnomeBG from 'gi://GnomeBG?version=4.0';
const show = new GnomeBG.BGSlideShow({file: Gio.File.new_for_path(ARGV[0])});
show.load();
const name = f => (typeof f === 'string' ? f : f?.get_path?.() ?? '');
if (ARGV[1] === 'now') {
    const [progress, duration, fixed, a, b] = show.get_current_slide(1920, 1080);
    print(`${fixed ? 'static' : 'fade'} ${progress.toFixed(3)} ${name(a)} ${name(b)}`);
} else {
    print(`${show.get_num_slides()} ${show.get_total_duration()}`);
}
EOF
chmod -R a+rX "${OWN}"

# The lab's Eval (a lab-only extension of the test account, never in the image)
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
    wait_for 240 has_session && wait_for 120 module_up
    sleep 10
fi
check "the theme module runs" module_up

# The account as the image leaves it
rm -f "${CFG}/accent" "${CFG}/colours" "${CFG}/daylight" "${CFG}/wallpaper-colours.toml"
gs reset "${THEME_UI}" accent-per-workspace
gs reset "${THEME_UI}" workspace-accents
gs reset "${COLOR}" night-light-enabled
gs reset "${COLOR}" night-light-schedule-automatic

# --- 1-6: the accents ---------------------------------------------------------------------------------------------------
ame_ desktop theme amethystora >/dev/null
before="$(cat "${CUR}"/theme/shell-workspaces.css)"
ame_ desktop accent jade >/dev/null
same "1 the account's accent is jade" "$(cat "${CFG}/accent" 2>/dev/null)" jade
same "1 colors.toml's accent is Jade's fill" "$(key "${CUR}/theme/colors.toml" accent)" "#16865c"
same "1 palette.toml: accent_bg" "$(key "${CUR}/theme/palette.toml" accent_bg)" "#16865c"
same "1 palette.toml: accent_fg" "$(key "${CUR}/theme/palette.toml" accent_fg)" "#ffffff"
same "1 palette.toml: accent_text" "$(key "${CUR}/theme/palette.toml" accent_text)" "#359c70"
check "1 the applications' stylesheet has it" grep -q "#16865c" "${HOME_DIR}/.config/gtk-4.0/gtk.css"
check "1 the shell's stylesheets have it" grep -lq "#16865c" "${CUR}"/theme/shell-*.css
if [[ "${before}" == *"#9148f0"* && "$(cat "${CUR}/theme/shell-workspaces.css")" != *"#9148f0"* ]]; then
    ok "1 the workspaces' stylesheet no longer has the theme's own accent"
else
    not_ok "1 the theme's own accent is still in the workspaces' stylesheet"
fi
same "1 GNOME's accent-color is the nearest of its nine" "$(gs get org.gnome.desktop.interface accent-color)" "'green'"
ame_ desktop theme catppuccin-latte >/dev/null
same "2 a light theme keeps the accent: accent_bg" "$(key "${CUR}/theme/palette.toml" accent_bg)" "#16865c"
same "2 accent_fg" "$(key "${CUR}/theme/palette.toml" accent_fg)" "#ffffff"
same "2 accent_text" "$(key "${CUR}/theme/palette.toml" accent_text)" "#00774e"
bad=""
for theme in amethystora catppuccin-latte; do
    ame_ desktop theme "${theme}" >/dev/null
    for id in amethyst iris sky lagoon jade moss amber ember coral rose orchid slate; do
        out="$(in_session "echo ${id} > ~/.config/amethystora/accent; AMETHYSTORA_THEME_QUIET=1 amethystora-theme reload" 2>&1)"
        [[ "$(key "${CUR}/theme/palette.toml" accent_fg)" == "#ffffff" && ${out} != *"cannot give every colour"* ]] ||
            bad+=" ${theme}/${id}"
    done
done
same "3 every accent in a dark and a light theme takes a white label and reads" "${bad}" ""
ame_ desktop theme amethystora >/dev/null
ame_ desktop accent coral >/dev/null
ev "Main.overview.hide(); global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 1" >/dev/null
sleep 2
pill="$(ev "(() => { let p = null; const walk = a => { if (a.has_style_class_name?.('amethystora-workspaces-active')) p = a;
    a.get_children().forEach(walk); }; walk(Main.panel); return p?.get_theme_node().get_background_color().to_string(); })()")"
same "4 the shell's workspace pill is Coral" "${pill:0:7}" "#d63b5c"
ame_ desktop accent theme >/dev/null
check "5 Follow the theme takes the account's accent away" test ! -e "${CFG}/accent"
same "5 colors.toml's accent is the theme's again" "$(key "${CUR}/theme/colors.toml" accent)" "#9148f0"
same "5 accent-color is the theme's own name for it" "$(gs get org.gnome.desktop.interface accent-color)" "'purple'"
code=0
out="$(in_session "ame desktop accent teal" 2>&1)" || code=$?
if [[ ${out} == *"usage: ame desktop accent "* && ! -e "${CFG}/accent" ]]; then
    ok "6 a bad accent is refused with the usage line, nothing written"
else
    not_ok "6 a bad accent was not refused" "${out}"
fi
in_session "printf '%s\n' '\"; rm -rf ~' > ~/.config/amethystora/accent"
out="$(ame_ desktop theme amethystora)"
same "6 a hand-written accent file is passed over" "$(key "${CUR}/theme/colors.toml" accent)" "#9148f0"
check "6 ...and the home folder is still there" test -d "${HOME_DIR}/.config"
rm -f "${CFG}/accent"

# --- 7-15: Daylight ------------------------------------------------------------------------------------------------------
rm -rf "${CUR}" "${CFG}/daylight"
ame_ desktop theme catppuccin >/dev/null
XML="${CUR}/backgrounds/catppuccin-daylight.xml"
check "7 the slideshow is written" test -s "${XML}"
for k in "org.gnome.desktop.background picture-uri" "org.gnome.desktop.background picture-uri-dark" \
    "org.gnome.desktop.screensaver picture-uri"; do
    same "7 ${k} is the slideshow" "$(gs get ${k})" "'file://${XML}'"
done
same "7 the background link still names the opening picture" "$(readlink "${CUR}/background")" "${B}/catppuccin/lakeside-peaks.png"
same "8 GNOME's reader finds nine slides, a day in all" "$(as_user "gjs -m ${OWN}/slideshow.js ${XML}" 2>/dev/null)" "9 86400"

# 9: the picture of each hour, with the clock set, and put back whatever happens: to the time it was plus the time the
# suite spent meanwhile (the boot clock, which setting the time does not move), then NTP on and one step to the network's
# time where there is one (chrony only steps on its first updates, and slews after)
CLOCK_WALL="$(date +%s)"
CLOCK_BOOT="$(cut -d. -f1 /proc/uptime)"
restore_clock() {
    timedatectl set-time "@$((CLOCK_WALL + $(cut -d. -f1 /proc/uptime) - CLOCK_BOOT))" >/dev/null 2>&1 ||
        date -s "@$((CLOCK_WALL + $(cut -d. -f1 /proc/uptime) - CLOCK_BOOT))" >/dev/null 2>&1 || true
    timedatectl set-ntp true >/dev/null 2>&1 || true
    chronyc -a makestep >/dev/null 2>&1 || true
}
trap restore_clock EXIT
timedatectl set-ntp false
for case in "02:00 static night" "06:00 fade 0.333" "08:00 static dawn" "11:15 fade 0.500" "14:00 static day" \
    "18:00 fade dusk" "20:00 static dusk" "21:30 fade 0.500" "23:00 static night"; do
    read -r at kind what <<<"${case}"
    timedatectl set-time "$(date +%F) ${at}:00"
    read -r got progress a b <<<"$(as_user "gjs -m ${OWN}/slideshow.js ${XML} now" 2>/dev/null)"
    want_file=""
    case "${what}" in
        night) want_file="${B}/catppuccin/lakeside-peaks.png" ;;
        dawn | day | dusk) want_file="${B}/catppuccin/lakeside-peaks-${what}.png" ;;
    esac
    if [[ ${got} == "${kind}" && (-z ${want_file} || ${a} == "${want_file}" || ${b} == "${want_file}") &&
        (${what} != 0.* || $(awk -v p="${progress}" -v w="${what}" 'BEGIN { print (p - w < 0.02 && w - p < 0.02) }') == 1) ]]; then
        ok "9 at ${at}: ${kind} ${what} (${progress})"
    else
        not_ok "9 at ${at}" "expected: ${kind} ${what}" "found: ${got} ${progress} ${a} ${b}"
    fi
done
timedatectl set-time "$(date +%F) 14:00:00"
in_session "amethystora-theme daylight refresh --force"
sleep 3
shown="$(ev "Main.layoutManager._bgManagers[0].backgroundActor.content.background._animation?.keyFrameFiles.map(f => f.get_path()).join(',')")"
same "9 the shell shows the day picture at 14:00" "${shown}" "${B}/catppuccin/lakeside-peaks-day.png"
restore_clock
trap - EXIT
drift="$(($(date +%s) - CLOCK_WALL - ($(cut -d. -f1 /proc/uptime) - CLOCK_BOOT)))"
((drift > -5 && drift < 5)) && ok "9 the clock is put back (${drift} s off)" || not_ok "9 the clock is not put back" "${drift} s off"

# 10: Night Light's own hours
gs set "${COLOR}" night-light-enabled true
gs set "${COLOR}" night-light-schedule-automatic false
gs set "${COLOR}" night-light-schedule-from 20.0
gs set "${COLOR}" night-light-schedule-to 5.5
durations() { grep -o '<duration>[0-9]*' "${XML}" | tr -dc '0-9\n' | paste -sd' '; }
night_light_times() { [[ "$(durations)" == "17100 2700 14400 5400 27000 5400 8100 3600 2700" ]]; }
check "10 Night Light's hours give dawn 05:30, day 11:00, dusk 20:00, night 23:15" wait_for 10 night_light_times
gs set "${COLOR}" night-light-schedule-from 9.0
default_times() { [[ "$(durations)" == "20700 2700 14400 5400 18900 5400 8100 3600 7200" ]]; }
check "10 hours out of bounds give the defaults" wait_for 10 default_times
gs reset "${COLOR}" night-light-enabled
gs reset "${COLOR}" night-light-schedule-automatic
gs reset "${COLOR}" night-light-schedule-from
gs reset "${COLOR}" night-light-schedule-to

gs set org.gnome.desktop.background picture-uri "file://${B}/nord/ice-sea.png"
in_session "amethystora-theme daylight adopt"
same "11 a picture of one's own is kept" "$(gs get org.gnome.desktop.background picture-uri)" "'file://${B}/nord/ice-sea.png'"
gs set org.gnome.desktop.background picture-uri "file://${B}/catppuccin/lakeside-peaks.png"
in_session "amethystora-theme daylight adopt"
same "12 an account on the opening picture is shown Daylight" "$(gs get org.gnome.desktop.background picture-uri)" "'file://${XML}'"
ame_ desktop daylight off >/dev/null
same "13 Daylight off writes off" "$(cat "${CFG}/daylight" 2>/dev/null)" off
same "13 ...and shows the plain opening picture" "$(gs get org.gnome.desktop.background picture-uri)" "'file://${B}/catppuccin/lakeside-peaks.png'"
ame_ desktop daylight on >/dev/null
same "13 Daylight on shows the slideshow again" "$(gs get org.gnome.desktop.background picture-uri)" "'file://${XML}'"
in_session "amethystora-theme bg next" >/dev/null 2>&1
same "13 the next wallpaper is the theme's second" "$(gs get org.gnome.desktop.background picture-uri)" "'file://${B}/catppuccin/pastel-waves.png'"
in_session "amethystora-theme bg next; amethystora-theme bg next" >/dev/null 2>&1
same "13 ...and round again to Daylight" "$(gs get org.gnome.desktop.background picture-uri)" "'file://${XML}'"
ame_ desktop theme catppuccin-latte >/dev/null
LXML="${CUR}/backgrounds/catppuccin-latte-daylight.xml"
check "14 a light theme's day is its own picture" grep -q "<file>${B}/catppuccin-latte/morning-peaks.png</file>" "${LXML}"
check "14 ...and its night the one drawn for the night" grep -q "<file>${B}/catppuccin-latte/morning-peaks-night.png</file>" "${LXML}"
bad=""
for list in /usr/share/amethystora/themes/*/daylight.list; do
    declare -A L=()
    while read -r time picture; do
        [[ "$(file -b "${picture}")" == "PNG image data, 3840 x 2160,"* ]] || bad+=" ${picture}:size"
        L[${time}]="$(mean_l "${picture}")"
    done <"${list}"
    # The Amethystora theme's day stays a dark amethyst, so it rises less
    rise=0.25
    [[ ${list} == */themes/amethystora/daylight.list ]] && rise=0.12
    awk -v n="${L[night]}" -v k="${L[dusk]}" -v w="${L[dawn]}" -v d="${L[day]}" -v r="${rise}" 'BEGIN { exit !(n < k && k < w && w < d && d - n >= r) }' ||
        bad+=" ${list}:${L[night]}<${L[dusk]}<${L[dawn]}<${L[day]}"
    unset L
done
same "15 every theme's four pictures are there, at size, night < dusk < dawn < day by 0.25 or more (the dark Amethystora theme's by 0.12)" "${bad}" ""
same "15 ...and no picture is on two lists, or twice on one" "$(cut -d' ' -f2- /usr/share/amethystora/themes/*/daylight.list | sort | uniq -d)" ""

# --- 16-22: colours from the wallpaper -------------------------------------------------------------------------------
ame_ desktop theme catppuccin >/dev/null
style_before="$(cat "${CUR}/style.name")"
icons_before="$(gs get org.gnome.desktop.interface icon-theme)"
ame_ desktop colours soft >/dev/null
TOML="${CFG}/wallpaper-colours.toml"
same "16 the way is soft" "$(cat "${CFG}/colours" 2>/dev/null)" soft
check "16 the toml has its stamp" grep -q '^# stamp soft dark ' "${TOML}"
for k in background foreground accent selection_background; do
    v="$(key "${TOML}" "${k}")"
    [[ -n ${v} && "$(key "${CUR}/theme/colors.toml" "${k}")" == "${v}" ]] && ok "16 ${k} merged (${v})" || not_ok "16 ${k} not merged"
done
same "16 the terminal's colours are the theme's" "$(key "${CUR}/theme/colors.toml" color1)" "#f38ba8"
same "16 the cursor is the theme's" "$(key "${CUR}/theme/colors.toml" cursor)" "#f5e0dc"
check "16 catppuccin stays dark" test ! -e "${CUR}/theme/light.mode"
same "16 the style is kept" "$(cat "${CUR}/style.name")" "${style_before}"
same "16 the icons are kept" "$(gs get org.gnome.desktop.interface icon-theme)" "${icons_before}"
soft_accent="$(key "${TOML}" accent)"
read -r soft_l soft_c <<<"$(oklab "$(key "${TOML}" background)")"
ame_ desktop colours vivid >/dev/null
read -r al _ <<<"$(oklab "$(key "${TOML}" accent)")"
read -r _ vivid_c <<<"$(oklab "$(key "${TOML}" background)")"
if [[ "$(key "${TOML}" accent)" != "${soft_accent}" ]] && awk -v l="${al}" 'BEGIN { exit !(l > 0.73 && l < 0.75) }'; then
    ok "17 vivid's accent differs from soft's, at L ${al}"
else
    not_ok "17 vivid's accent" "${al}"
fi
ame_ desktop colours mono >/dev/null
read -r _ mono_c <<<"$(oklab "$(key "${TOML}" background)")"
check "17 mono writes no accent" bash -c "! grep -q '^accent' '${TOML}'"
same "17 mono keeps the theme's accent" "$(key "${CUR}/theme/colors.toml" accent)" "#89b4fa"
# The targets (0.012, 0.025, 0.050) and what writing them as #rrggbb adds: up to 0.003 on a dark ground
awk -v m="${mono_c}" -v s="${soft_c}" -v v="${vivid_c}" 'BEGIN { exit !(m <= 0.015 && s <= 0.028 && v <= 0.053) }' &&
    ok "17 the ground's chroma: mono ${mono_c}, soft ${soft_c}, vivid ${vivid_c}" ||
    not_ok "17 the ground's chroma" "mono ${mono_c} soft ${soft_c} vivid ${vivid_c}"
ame_ desktop theme rose-pine-dawn >/dev/null
ame_ desktop colours soft >/dev/null
check "18 a light theme stays light" test -e "${CUR}/theme/light.mode"
read -r own_l _ <<<"$(oklab "#faf4ed")"
read -r new_l _ <<<"$(oklab "$(key "${CUR}/theme/colors.toml" background)")"
awk -v a="${own_l}" -v b="${new_l}" 'BEGIN { d = a - b; exit !(d < 0.01 && d > -0.01) }' &&
    ok "18 the ground keeps the theme's lightness (${own_l} ${new_l})" || not_ok "18 the ground's lightness moved" "${own_l} ${new_l}"
r1="$(ratio "$(key "${CUR}/theme/palette.toml" accent_text)" "$(sed -nE 's/^ *--window-bg-color: (#[0-9a-f]{6}).*/\1/p' "${HOME_DIR}/.config/gtk-4.0/gtk.css" | head -n1)")"
awk -v r="${r1}" 'BEGIN { exit !(r >= 4.5) }' && ok "18 accent_text reads on the window (${r1}:1)" || not_ok "18 accent_text on the window" "${r1}"
ame_ desktop theme catppuccin >/dev/null
ame_ desktop colours soft >/dev/null
bg_before="$(key "${CUR}/theme/colors.toml" background)"
gs set org.gnome.desktop.background picture-uri "file://${B}/nord/aurora.png"
aurora_stamp() { grep -q 'aurora.png' "${TOML}"; }
check "19 a picture set in GNOME's Settings is worked out within 5 s" wait_for 8 aurora_stamp
colours_changed() { [[ "$(key "${CUR}/theme/colors.toml" background)" != "${bg_before}" ]]; }
check "19 ...and applied" wait_for 8 colours_changed
mtime="$(stat -c %Y "${TOML}")"
sleep 2
gs set org.gnome.desktop.background picture-uri "file://${B}/nord/aurora.png"
in_session "amethystora-theme colours refresh"
same "19 the same picture again leaves the toml as it was" "$(stat -c %Y "${TOML}")" "${mtime}"
ame_ desktop colours off >/dev/null
check "20 off takes the way and the toml away" test ! -e "${CFG}/colours" -a ! -e "${TOML}"
same "20 the theme's own ground is back" "$(key "${CUR}/theme/colors.toml" background)" "#1e1e2e"
ame_ desktop colours vivid >/dev/null
ame_ desktop accent rose >/dev/null
same "21 a chosen accent wins over the wallpaper's" "$(key "${CUR}/theme/colors.toml" accent)" "#c2338f"
detail="$(in_session "/usr/libexec/amethystora-control-list --json" | jq -r '.items[] | select(.id == "colours") | .detail')"
same "21 Control says the accent is kept" "${detail}" "Your accent is kept: choose Follow the theme under Accent to take the wallpaper's."
ame_ desktop accent theme >/dev/null
ame_ desktop colours off >/dev/null
toml_before="$(cat "${TOML}" 2>/dev/null)"
helper() {
    local code=0
    in_session "/usr/libexec/amethystora-wallpaper-colours --way soft --mode dark --background '#1e1e2e' --foreground '#cdd6f4' $1" \
        >/dev/null 2>&1 || code=$?
    echo "${code}"
}
same "22 /proc/self/status is refused" "$(helper /proc/self/status)" 3
in_session "echo not a picture > /tmp/vmlab-x.png"
same "22 a text file named x.png is refused" "$(helper /tmp/vmlab-x.png)" 3
code=0
/usr/libexec/amethystora-wallpaper-colours --way soft --mode dark --background '#1e1e2e' --foreground '#cdd6f4' \
    "${B}/nord/aurora.png" >/dev/null 2>&1 || code=$?
same "22 root is refused" "${code}" 2
same "22 the toml is untouched by all three" "$(cat "${TOML}" 2>/dev/null)" "${toml_before}"

# --- 23: Control's theme preview ---------------------------------------------------------------------------------------
ame_ desktop theme amethystora >/dev/null
in_session "systemd-run --user --collect /usr/bin/amethystora-control appearance" >/dev/null 2>&1
atspi() { user_bus python3 "${OWN}/atspi.py" amethystora-control "$@"; }
# The middle of a widget of Control's as X,Y fractions of the screen: AT-SPI gives a GTK 4 window's widgets from the
# window's top left corner (atspi.py's where is for GNOME Shell's own), and the shell knows where the window is
where() {
    local box
    box="$(atspi box "$1" 2>/dev/null)" || return 0
    ev "(() => { const [x, y, w, h] = '${box}'.split(',').map(Number);
        const win = global.get_window_actors().map(a => a.meta_window).find(m => m.get_wm_class() === 'org.amethystora.Control');
        const r = win.get_frame_rect();
        return ((r.x + x + w / 2) / global.stage.width).toFixed(4) + ',' + ((r.y + y + h / 2) / global.stage.height).toFixed(4); })()"
}
if user_bus python3 "${OWN}/atspi.py" amethystora-control wait 60 "Nord" >/dev/null 2>&1; then
    # The pointer through the shell's own virtual pointer, to X,Y fractions of the screen
    point() {
        ev "(() => { const [x, y] = '$1'.split(','); const C = imports.gi.Clutter;
            const seat = global.stage.context.get_backend().get_default_seat();
            globalThis.vmlabPointer ??= seat.create_virtual_device(C.InputDeviceType.POINTER_DEVICE);
            vmlabPointer.notify_absolute_motion(global.get_current_time() * 1000, x * global.stage.width, y * global.stage.height);
            return 1; })()" >/dev/null
    }
    # Nord's swatch is in a later row of the grid, which the preview above it pushes down to the bottom of the screen,
    # past the window or under the dock: the page is scrolled a step at a time, with the pointer over the grid, until
    # it is in view
    for _ in 1 2 3 4 5 6 7 8; do
        where="$(where "Nord")"
        [[ -n ${where} ]] && awk -v y="${where#*,}" 'BEGIN { exit !(y < 0.82) }' && break
        point "$(where "Amethystora Light")"
        ev "vmlabPointer.notify_discrete_scroll(global.get_current_time() * 1000, imports.gi.Clutter.ScrollDirection.DOWN, imports.gi.Clutter.ScrollSource.WHEEL); 1" >/dev/null
        sleep 0.5
    done
    point "${where}"
    sleep 1
    shot() {
        user_bus gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
            --method org.gnome.Shell.Screenshot.Screenshot false false "$1" >/dev/null 2>&1
    }
    # Pixels within 12 of a colour in the band of the window above the first swatch, where the preview is
    first="$(where "Amethystora Light")"
    near() {
        python3 -I -c 'import sys
import gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1]); bottom = int(float(sys.argv[2].split(",")[1]) * p.get_height()) - 40
want = [int(sys.argv[3][i:i + 2], 16) for i in (1, 3, 5)]
d, n, s, w = p.get_pixels(), p.get_n_channels(), p.get_rowstride(), p.get_width()
count = 0
for y in range(max(0, bottom - 600), max(0, bottom), 2):
    for x in range(0, w, 2):
        i = y * s + x * n
        if max(abs(d[i + k] - want[k]) for k in range(3)) <= 12:
            count += 1
print(count)' "$1" "${first}" "$2"
    }
    shot /tmp/vmlab-preview-nord.png
    nord="$(near /tmp/vmlab-preview-nord.png "#81a1c1")"
    # Off the grid, and nothing with the keyboard's focus there: the chosen theme's (Amethystora's) colours back
    ev "vmlabPointer?.notify_absolute_motion(global.get_current_time() * 1000, 5, global.stage.height / 2); 1" >/dev/null
    sleep 0.5
    shot /tmp/vmlab-preview-back.png
    back_nord="$(near /tmp/vmlab-preview-back.png "#81a1c1")"
    back_own="$(near /tmp/vmlab-preview-back.png "#9148f0")"
    if [[ -n ${first} ]] && ((nord > 40)); then
        ok "23 pointing at Nord draws the preview in Nord's accent (${nord} pixels)"
    else
        not_ok "23 pointing at Nord did not draw its accent in the preview" "${nord} pixels, Nord at ${where}, Amethystora Light at ${first}"
    fi
    if ((back_own > 40 && back_nord < nord / 4)); then
        ok "23 off the grid, the chosen theme's accent is back within 0.5 s (${back_own} pixels, Nord's ${back_nord})"
    else
        not_ok "23 the chosen theme did not come back" "own ${back_own}, Nord's ${back_nord}"
    fi
    # Decoration: its words are not in the accessibility tree as their own nodes
    if atspi has "Water the plants" >/dev/null 2>&1; then
        not_ok "23 the preview's words are in the accessibility tree"
    else
        ok "23 the preview is decoration: its words are not in the accessibility tree"
    fi
else
    not_ok "23 Control did not show the themes"
fi
in_session "pkill -f amethystora-control" >/dev/null 2>&1

# --- 24-26: an accent per workspace -----------------------------------------------------------------------------------
ame_ desktop accent-per-workspace on >/dev/null
for id in sky coral jade amber orchid; do
    if [[ -s "${CUR}/theme/accents/${id}/shell-accent.css" && -s "${CUR}/theme/accents/${id}/palette.toml" ]]; then
        ok "24 ${id}'s set is rendered"
    else
        not_ok "24 ${id}'s set is missing"
    fi
done
listening() { [[ "$(ev "${TH}.listening")" == true ]]; }
check "24 the module listens to workspace switches" wait_for 10 listening
accent_before="$(gs get org.gnome.desktop.interface accent-color)"
ev "global.workspace_manager.get_workspace_by_index(1).activate(global.get_current_time()); 1" >/dev/null
sky_state() { [[ "$(cat "${STATE}" 2>/dev/null)" == sky ]]; }
check "24 within 1 s the state file says sky" wait_for 2 sky_state
pill="$(ev "(() => { let p = null; const walk = a => { if (a.has_style_class_name?.('amethystora-workspaces-active')) p = a;
    a.get_children().forEach(walk); }; walk(Main.panel); return p?.get_theme_node().get_background_color().to_string(); })()")"
same "24 the workspace pill is Sky's" "${pill:0:7}" "#2172e2"
swap="$(ev "${TH}.lastSwapMs")"
awk -v t="${swap}" 'BEGIN { exit !(t <= 20) }' && ok "24 the restyle took ${swap} ms" || not_ok "24 the restyle took ${swap} ms, over 20"
in_session "systemd-run --user --collect /usr/bin/amethystora-notes" >/dev/null 2>&1
notes_up() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.amethystora.Notes')")" == true ]]; }
if wait_for 30 notes_up; then
    sleep 3
    rect="$(ev "(() => { const w = global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.amethystora.Notes');
        const r = w.get_frame_rect(); return [r.x, r.y, r.width, r.height].join(','); })()")"
    user_bus gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
        --method org.gnome.Shell.Screenshot.Screenshot false false /tmp/vmlab-notes-sky.png >/dev/null 2>&1
    sky_pixels="$(python3 -I -c 'import sys
import gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1]); x0, y0, w0, h0 = (int(v) for v in sys.argv[2].split(","))
d, n, s = p.get_pixels(), p.get_n_channels(), p.get_rowstride()
want = (0x21, 0x72, 0xe2)
print(sum(1 for y in range(max(0, y0), min(p.get_height(), y0 + h0), 2) for x in range(max(0, x0), min(p.get_width(), x0 + w0), 2)
    if max(abs(d[y * s + x * n + k] - want[k]) for k in range(3)) <= 12))' /tmp/vmlab-notes-sky.png "${rect}")"
    if ((sky_pixels > 40)); then
        ok "24 Notes draws its accent in Sky's colour (${sky_pixels} pixels within 12 of #2172e2)"
    else
        not_ok "24 Notes does not draw Sky's accent" "${sky_pixels} pixels; look at /tmp/vmlab-notes-sky.png"
    fi
else
    not_ok "24 Notes did not open"
fi
in_session "pkill -f amethystora-notes" >/dev/null 2>&1

# The path every real machine takes, with animations on: the accent once the slide has ended, and a quick run of
# switches restyling once (the lab draws in software and turns animations off; they are forced on here and put back)
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 1" >/dev/null
theme_state_now() { [[ ! -e "${STATE}" ]]; }
check "24 back on workspace 1 first" wait_for 4 theme_state_now
# Slowed down too: the first frame of a slide drawn in software holds the shell's loop for longer than the module's
# wait at the image's speed (0.6 of 300 ms), which would let the accent through mid-slide in the lab only
ev "global._vmlabForced = !imports.gi.St.Settings.get().enable_animations; global.force_animations = true;
    if (global._vmlabForced) imports.gi.St.Settings.get().uninhibit_animations();
    global._vmlabSlow ??= imports.gi.St.Settings.get().slow_down_factor; imports.gi.St.Settings.get().slow_down_factor = 4; 1" >/dev/null
unforce() {
    ev "global.force_animations = true; if (global._vmlabForced) imports.gi.St.Settings.get().inhibit_animations();
        global._vmlabForced = false; if (global._vmlabSlow !== undefined) imports.gi.St.Settings.get().slow_down_factor = global._vmlabSlow;
        delete global._vmlabSlow; 1" >/dev/null 2>&1
}
trap unforce EXIT
slide="$(ev "Math.round(300 * imports.gi.St.Settings.get().slow_down_factor)")"
swaps="$(ev "${TH}.swaps ?? 0")"
# The three switches and the count in one Eval, so that no round trip of the lab's software-drawn session eats the
# slide's ${slide} ms: each switch a little after the last, through the shell's own main loop
ev "(() => { const wm = global.workspace_manager, GLib = imports.gi.GLib;
    wm.get_workspace_by_index(1).activate(global.get_current_time());
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, 40, () => { wm.get_workspace_by_index(2).activate(global.get_current_time()); return false; });
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, 80, () => { wm.get_workspace_by_index(1).activate(global.get_current_time());
        globalThis.vmlabSwapsDuring = ${TH}.swaps ?? 0; return false; });
    return 1; })()" >/dev/null
sleep 0.2
same "24 with animations on, nothing restyles while the slide runs" "$(ev "globalThis.vmlabSwapsDuring")" "${swaps}"
sleep "$(awk -v ms="${slide}" 'BEGIN { printf "%.2f", (ms + 700) / 1000 }')"
same "24 ...and once it has ended, the shown workspace's accent, once for the run of three switches" \
    "$(ev "${TH}.swaps ?? 0") $(cat "${STATE}" 2>/dev/null)" "$((swaps + 1)) sky"
unforce
trap - EXIT
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 1" >/dev/null
theme_state() { [[ ! -e "${STATE}" ]]; }
check "24 back on workspace 1 the state file is gone (the theme's own)" wait_for 2 theme_state
same "24 GNOME's accent-color never changed" "$(gs get org.gnome.desktop.interface accent-color)" "${accent_before}"
ame_ desktop accent-per-workspace 3 moss >/dev/null
same "25 workspace 3 takes moss" "$(gs get "${THEME_UI}" workspace-accents)" "['theme', 'sky', 'moss', 'jade', 'amber', 'orchid']"
check "25 moss's set is rendered" test -d "${CUR}/theme/accents/moss"
ev "global.workspace_manager.get_workspace_by_index(2).activate(global.get_current_time());
    Main.panel.statusArea['amethystora-workspaces']; ${UI}.get('workspaces').openMenu(); 1" >/dev/null
sleep 1
moss="$(ev "(() => { const items = ${UI}.get('workspaces')._accentItems; const m = items?.get('moss');
    return m ? String(m._ornament === 2 /* PopupMenu.Ornament.CHECK */) : 'no menu'; })()")"
same "25 the label's Accent menu has Moss checked" "${moss}" true
ev "${UI}.get('workspaces')._menu.close(); 1" >/dev/null
code=0
in_session "ame desktop accent-per-workspace 3 teal" >/dev/null 2>&1 || code=$?
same "25 a bad accent for a workspace is refused" "${code}" 64
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 1" >/dev/null
ame_ desktop accent-per-workspace off >/dev/null
check "26 off takes the sets away" test ! -e "${CUR}/theme/accents"
check "26 off takes the state file away" test ! -e "${STATE}"
same "26 off listens to no workspace switch" "$(ev "${TH}.listening")" false

# --- Leave the account as it was --------------------------------------------------------------------------------------
gs reset "${THEME_UI}" workspace-accents
rm -f "${CFG}/accent" "${CFG}/colours" "${CFG}/daylight" "${CFG}/wallpaper-colours.toml"
ame_ desktop theme amethystora >/dev/null

errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --after-cursor "${CURSOR}" --no-pager -o cat 2>/dev/null |
    awk -v uuid="${UUID}" '/JS ERROR/ { block = $0; next } block && /@(resource|file):\/\// { block = block "\n" $0; next }
        block { if (index(block, uuid)) print block; block = "" } /Amethystora UI: .*(theme|accent)/ { print }
        END { if (block && index(block, uuid)) print block }' | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of Amethystora UI threw"; else not_ok "Amethystora UI threw" "${errors}"; fi
finish
