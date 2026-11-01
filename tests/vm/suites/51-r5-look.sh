#!/usr/bin/bash
# vmlab-vm: r5-look
# Rounds 5 and 6's look group (plans/ui-ideas/spec-look.md, section 8), in the test account's session:
#   1. Atkinson Hyperlegible Next and Mono installed, the mono one monospaced, and set through ame desktop font;
#   2. a palette saved as a theme of the account's own (ame desktop theme save), a bad one and a system theme's name refused;
#   3. Control's Make a theme: the card, its checks, a name typed and Save, which makes the theme;
#   4. a kitty, an Alacritty, a base16, a base24 and an Xresources file imported for their colours only;
#   5. a look shared as exactly three names, opening one refused while look files are off, a look file with a fourth
#      name refused whole;
#   6. a drawn wallpaper shown in the theme's colours, drawn again under a new name for a new accent, shuffled, at the four
#      times of day through Daylight, and off;
#   7. light by day, dark by night: the theme flips to its pair at a boundary, once; a theme flipped by hand stays; a
#      theme without a pair is left alone; off waits for nothing;
#   8. ten disables and enables of Amethystora UI with it on leave one timeout; the setup file saves the new keys.
# vmlab-paths: @ui/modules/theme.js @ui/modules/theme/* @ui/schemas/*theme* @sf/usr/lib/amethystora/theme/* @sf/usr/bin/amethystora-theme @sf/usr/libexec/amethystora-theme-file @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/*just/desktop.just @sf/usr/share/applications/amethystora-look.desktop @sf/usr/share/applications/amethystora-theme-import.desktop @sf/usr/share/mime/packages/amethystora-look.xml apps/control/* apps/control-core/* build_files/base/09-fonts.sh
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HOME_DIR="/var/home/${TEST_USER}"
CONFIG="${HOME_DIR}/.config/amethystora"
# Not under /run: amethystora-theme-file refuses to read /proc, /sys, /dev and /run, as a person's files are never there
WORK=/var/tmp/vmlab-51
THEME_UI=org.gnome.shell.extensions.amethystora-ui.theme
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
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
theme_now() { cat "${CONFIG}/current/theme.name" 2>/dev/null; }
DN="Main.extensionManager.lookup('${UUID}').stateObj.modules.get('theme').dayNight"

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
chown -R "${TEST_USER}:${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
# GNOME finds an extension installed while it runs only at its next start: listed, and the session started again
if [[ "$(ev "'up'" 2>/dev/null)" != up ]]; then
    enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
    [[ ${enabled} == *vmlab-eval@lab* ]] ||
        user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session
    sleep 15
fi
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}"
cp "$(dirname "$0")/atspi.py" "${WORK}/"
chmod a+r "${WORK}/atspi.py"
in_session "ame desktop theme amethystora" >/dev/null 2>&1

# --- 1. Atkinson Hyperlegible ---------------------------------------------------------------------------------------
for family in "Atkinson Hyperlegible Next" "Atkinson Hyperlegible Mono"; do
    if fc-list : family | tr ',' '\n' | grep -Fx -- "${family}" >/dev/null; then ok "${family} is installed"; else not_ok "${family} is not installed"; fi
done
if fc-list "Atkinson Hyperlegible Mono" spacing | grep -q 'spacing=100'; then ok "the mono cut is monospaced"; else not_ok "the mono cut is not monospaced"; fi
in_session 'ame desktop font "Atkinson Hyperlegible Next"' >/dev/null 2>&1
same "ame desktop font sets it" "$(gs get org.gnome.desktop.interface font-name)" "'Atkinson Hyperlegible Next 11'"
in_session 'ame desktop mono-font "Atkinson Hyperlegible Mono"' >/dev/null 2>&1
same "ame desktop mono-font sets it" "$(gs get org.gnome.desktop.interface monospace-font-name)" "'Atkinson Hyperlegible Mono 11'"
in_session "ame desktop font theme; ame desktop mono-font theme" >/dev/null 2>&1
LIST="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "Control offers both" "$(jq -r '[.items[] | select(.id == "font" or .id == "mono-font") | .choices[].value
    | select(startswith("Atkinson"))] | join(",")' <<<"${LIST}")" "Atkinson Hyperlegible Next,Atkinson Hyperlegible Mono"

# --- 2. A palette saved as a theme ----------------------------------------------------------------------------------
PALETTE="$(grep ' = "#' /usr/share/amethystora/themes/nord/colors.toml | sed 's/"#2e3440"/"#202838"/')"
printf '%s\n' "${PALETTE}" >"${WORK}/palette"
chmod a+r "${WORK}/palette"
in_session "ame desktop theme save 'Lab Night' <${WORK}/palette" >/dev/null 2>&1
same "save writes the theme folder and applies it" "$(theme_now)" lab-night
if grep -q '^background = "#202838"$' "${CONFIG}/themes/lab-night/colors.toml" 2>/dev/null; then ok "the palette is the one sent"; else not_ok "the palette is not the one sent"; fi
for bad in nord "../x"; do
    if in_session "ame desktop theme save '${bad}' <${WORK}/palette" >/dev/null 2>&1; then not_ok "the name ${bad} was taken"; else ok "the name ${bad} is refused"; fi
done
sed 's/^accent = .*/accent = "$(id)"/' "${WORK}/palette" >"${WORK}/bad"
chmod a+r "${WORK}/bad"
if in_session "ame desktop theme save 'Lab Bad' <${WORK}/bad" >/dev/null 2>&1 || [[ -e "${CONFIG}/themes/lab-bad" ]]; then
    not_ok "a palette with a command in it was taken"
else
    ok "a palette with a command in it is refused"
fi

# --- 4. Colour files from elsewhere ---------------------------------------------------------------------------------
cat >"${WORK}/kanagawa.conf" <<'EOF'
include ~/.config/kitty/evil.conf
background #1f1f28
foreground #dcd7ba
color4 #7e9cd8
color12 #7fb4ca
EOF
cat >"${WORK}/night.toml" <<'EOF'
import = ["/etc/passwd"]
[colors.primary]
background = "0x101418"
foreground = "#e0e0e0"
[colors.normal]
blue = "#6272a4"
EOF
printf 'scheme: "Ocean"\nbase00: "2b303b"\nbase05: "c0c5ce"\nbase0D: "8fa1b3"\n' >"${WORK}/ocean.yaml"
printf 'system: "base24"\nname: "Day"\nbase00: "#fafafa"\nbase05: "#202020"\nbase0D: "#1c4bd0"\nbase12: "#e02020"\n' >"${WORK}/day.yaml"
printf '#define B #002b36\n*background: B\n*foreground: #839496\n*color4: #268bd2\n' >"${WORK}/Xresources"
chmod a+r "${WORK}"/*
check_import() {
    local file="$1" key="$2" want="$3" got
    got="$(in_session "/usr/libexec/amethystora-theme-file read ${WORK}/${file}" 2>/dev/null | jq -r ".colors.${key}")"
    same "${file}: ${key}" "${got}" "${want}"
}
check_import kanagawa.conf background '#1f1f28'
check_import kanagawa.conf color12 '#7fb4ca'
check_import night.toml background '#101418'
check_import ocean.yaml accent '#8fa1b3'
check_import day.yaml color9 '#e02020'
check_import Xresources background '#002b36'
same "an include is left out and counted" "$(in_session "/usr/libexec/amethystora-theme-file read ${WORK}/kanagawa.conf" 2>/dev/null | jq -c '[.read, .left_out]')" "[4,1]"
if in_session "/usr/libexec/amethystora-theme-file read https://example.org/x.conf" >/dev/null 2>&1; then not_ok "an address was read"; else ok "an address is refused"; fi
if in_session "/usr/libexec/amethystora-theme-file read /proc/self/environ" >/dev/null 2>&1; then not_ok "/proc was read"; else ok "/proc is refused"; fi

# --- 5. A look in one file ------------------------------------------------------------------------------------------
in_session "rm -f ${HOME_DIR}/lab.amethystora-look; ame desktop theme share lab-night ${HOME_DIR}/lab.amethystora-look" >/dev/null 2>&1
same "a look holds exactly its three names" "$(tar -tf "${HOME_DIR}/lab.amethystora-look" 2>/dev/null | sort | tr '\n' ' ')" "colors.toml look.conf wallpaper.png "
same "an .amethystora-look is a type of its own" "$(in_session "gio info -a standard::content-type ${HOME_DIR}/lab.amethystora-look" 2>/dev/null | sed -n 's/.*content-type: //p')" "application/x-amethystora-look"
in_session "ame desktop look-files off" >/dev/null 2>&1
said="$(in_session "ame desktop theme look ${HOME_DIR}/lab.amethystora-look" 2>&1)"
if grep -q "is off" <<<"${said}"; then ok "opening a look is refused while look files are off"; else not_ok "opening a look while off said: ${said}"; fi
( cd "${WORK}" && cp "${CONFIG}/themes/lab-night/colors.toml" colors.toml && printf 'name = X\n' >look.conf && echo x >run.sh &&
    tar -cf bad.amethystora-look look.conf colors.toml run.sh && chmod a+r bad.amethystora-look )
if in_session "/usr/libexec/amethystora-theme-file look ${WORK}/bad.amethystora-look" >/dev/null 2>&1; then not_ok "a look with a fourth name was read"; else ok "a look with a fourth name is refused whole"; fi

# --- 6. Drawn wallpapers --------------------------------------------------------------------------------------------
in_session "ame desktop theme amethystora; ame desktop daylight off; ame desktop pattern facets" >/dev/null 2>&1
uri="$(gs get org.gnome.desktop.background picture-uri)"
if [[ "${uri}" == *"/current/patterns/facets-"*"-night.svg'" ]]; then ok "the facets drawing is the wallpaper"; else not_ok "the wallpaper is ${uri}"; fi
first="${uri}"
in_session "ame desktop accent jade" >/dev/null 2>&1
uri="$(gs get org.gnome.desktop.background picture-uri)"
if [[ "${uri}" == *"/patterns/facets-"* && "${uri}" != "${first}" ]]; then ok "a new accent draws it again under a new name"; else not_ok "after a new accent: ${uri}"; fi
in_session "ame desktop accent theme; ame desktop pattern shuffle" >/dev/null 2>&1
shuffled="$(head -n1 "${CONFIG}/pattern" 2>/dev/null)"
if [[ -n "${shuffled}" && "${shuffled}" != facets ]]; then ok "shuffle draws another (${shuffled})"; else not_ok "shuffle left ${shuffled}"; fi
in_session "ame desktop daylight on" >/dev/null 2>&1
xml="$(gs get org.gnome.desktop.background picture-uri | tr -d "'" | sed 's|^file://||')"
if [[ "$(grep -c "/patterns/${shuffled}-" "${xml}" 2>/dev/null)" -ge 4 ]]; then ok "Daylight shows the drawing's four tones"; else not_ok "Daylight's slideshow ${xml} has not the drawing's tones"; fi
in_session "ame desktop pattern off" >/dev/null 2>&1
uri="$(gs get org.gnome.desktop.background picture-uri)"
xml="$(tr -d "'" <<<"${uri}" | sed 's|^file://||')"
if [[ ! -e "${CONFIG}/pattern" && "${uri}" != *patterns* ]] && ! grep -qs /patterns/ "${xml}"; then
    ok "off gives the theme's pictures back"
else
    not_ok "off left a drawing shown: ${uri}"
fi
# GNOME reads a slideshow's path once: each drawing's has a name of its own (and its pictures stay two minutes)
if journalctl _UID="$(id -u "${TEST_USER}")" --since "-5min" 2>/dev/null | grep -q "Failed to load animation"; then
    not_ok "GNOME could not load a drawn wallpaper's slideshow"
else
    ok "GNOME loaded every drawn wallpaper's slideshow"
fi

# --- 7. Light by day, dark by night ---------------------------------------------------------------------------------
in_session "ame desktop theme amethystora" >/dev/null 2>&1
rm -f "${HOME_DIR}/.local/state/amethystora/day-night"
same "off: nothing waits" "$(ev "JSON.stringify(${DN})")" '{"on":false,"timeout":false,"sleep":false,"next":null}'
# The day began a minute ago and the night begins in ninety seconds
read -r day night <<<"$(python3 -c 'import time; t = time.localtime(); h = t.tm_hour + t.tm_min / 60 + t.tm_sec / 3600
print(f"{(h - 1 / 60) % 24:.4f} {(h + 1.5 / 60) % 24:.4f}")')"
gs set "${THEME_UI}" day-starts "${day}"
gs set "${THEME_UI}" night-starts "${night}"
gs set "${THEME_UI}" day-night-times hours
gs set "${THEME_UI}" day-night true
if wait_for 30 bash -c "[[ \"\$(cat '${CONFIG}/current/theme.name')\" == amethystora-light ]]"; then ok "by day the light side"; else not_ok "by day: $(theme_now)"; fi
same "one timeout and the sleep signal while on" "$(ev "JSON.stringify([${DN}.on, ${DN}.timeout, ${DN}.sleep])")" "[true,true,true]"
# By hand back to the dark side: kept until the next boundary
in_session "amethystora-theme toggle" >/dev/null 2>&1
sleep 20
same "a theme flipped by hand stays" "$(theme_now)" amethystora
if wait_for 120 bash -c "[[ \"\$(cat '${HOME_DIR}/.local/state/amethystora/day-night')\" == night* ]]"; then ok "the night's boundary is crossed"; else not_ok "no night boundary"; fi
same "by night the dark side" "$(theme_now)" amethystora
# Ten disables and enables leave one timeout
for _ in $(seq 10); do
    in_session "gnome-extensions disable ${UUID}; gnome-extensions enable ${UUID}" >/dev/null 2>&1
done
sleep 3
same "after ten disables and enables, one timeout" "$(ev "JSON.stringify([${DN}.on, ${DN}.timeout])")" "[true,true]"
# A theme without a pair is left alone
in_session "ame desktop theme nord" >/dev/null 2>&1
in_session "amethystora-theme daynight day" >/dev/null 2>&1
same "a theme without a pair stays" "$(theme_now)" nord
gs set "${THEME_UI}" day-night false
sleep 1
same "off again: nothing waits" "$(ev "JSON.stringify([${DN}.timeout, ${DN}.sleep])")" "[false,false]"

# --- 8. The setup file ----------------------------------------------------------------------------------------------
in_session "ame desktop pattern dots; ame desktop day-night hours 6 20" >/dev/null 2>&1
saved="$(in_session "/usr/libexec/amethystora-setup save -" 2>/dev/null | jq -c '.settings.look | {pattern: .pattern, "day-night": .["day-night"], "day-starts": (.["day-starts"] | floor)}')"
same "the setup file keeps the drawn wallpaper and the day's hours" "${saved}" '{"pattern":"dots","day-night":true,"day-starts":6}'
in_session "ame desktop pattern off; ame desktop day-night off" >/dev/null 2>&1
gs reset "${THEME_UI}" day-starts
gs reset "${THEME_UI}" night-starts
gs reset "${THEME_UI}" day-night-times

# --- 3. Control's Make a theme --------------------------------------------------------------------------------------
in_session "ame desktop theme amethystora" >/dev/null 2>&1
user_bus systemd-run --user --collect --quiet -- amethystora-control appearance theme-maker >/dev/null 2>&1
atspi() { user_bus python3 "${WORK}/atspi.py" amethystora-control "$@"; }
if atspi wait 60 "Make a theme" >/dev/null 2>&1; then ok "Control shows Make a theme"; else not_ok "Control shows no Make a theme"; fi
if atspi has "Text on the surfaces" >/dev/null 2>&1 && atspi has "passes" >/dev/null 2>&1; then ok "the checks are measured"; else not_ok "no checks"; fi
atspi type "Lab Made" "The theme's name" >/dev/null 2>&1
atspi click "Save as Lab Made" >/dev/null 2>&1
if wait_for 30 test -f "${CONFIG}/themes/lab-made/colors.toml"; then ok "Save makes the theme"; else not_ok "Save made no theme"; fi
same "and applies it" "$(theme_now)" lab-made
# The page draws its cards again after Save: given time
both_cards() { atspi has "Drawn wallpapers" && atspi has "Light by day, dark by night"; }
if wait_for 30 both_cards; then
    ok "Control has the drawn wallpapers and light by day"
else
    not_ok "Control lacks the drawn wallpapers or light by day"
fi
pkill -u "${TEST_USER}" -f amethystora-control || true

in_session "ame desktop theme amethystora; rm -rf ~/.config/amethystora/themes/lab-night ~/.config/amethystora/themes/lab-made ~/lab.amethystora-look" >/dev/null 2>&1
rm -rf "${WORK}"
finish
