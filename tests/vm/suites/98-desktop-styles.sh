#!/usr/bin/bash
# The desktop styles, fonts and icon packs each theme brings (manual: themes.md#styles), as a person's session has them:
#   - every theme, applied, writes its own style, font and icon pack, renders the shell's half of the style and puts
#     the applications' half in both GTK stylesheets, and the shell's style module started and threw nothing;
#   - the shell draws what the style says: the top bar is held off the screen's edge in the floating, islands, capsule,
#     segments and neon styles and lies against it in the hairline and terminal ones, by where the shell's
#     accessibility tree puts the bar's buttons;
#   - every theme, applied, also writes its monospace font and its pointer, puts the palette layer (libadwaita's colour
#     variables, behind the query that leaves High Contrast alone, and adw-gtk3's named colours) in the GTK stylesheets, the
#     colours of the KDE applications in kdeglobals and the apps' palette.toml and style in the theme's folder;
#   - ame desktop style, font, mono-font and icons keep a choice over every theme, refuse what is not a style, not
#     installed or not a pack, and `theme` goes back to the theme's own; Control lists them;
#   - every font the themes and Control name is installed.
# The account is left on Tokyo Night, the neon style, with a Files window and the quick settings open, for the
# screenshot run.mjs takes after the suite.
# vmlab-paths: @ui/*style* @sf/*themed/* @sf/usr/lib/amethystora/theme/* @sf/usr/bin/amethystora-theme @sf/usr/share/icons/amethystora-*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
OWN=/run/vmlab-98
HOME_DIR="/var/home/${TEST_USER}"
CONFIG="${HOME_DIR}/.config/amethystora"
STYLE_SCHEMA=org.gnome.shell.extensions.amethystora-ui.style
THEMES_DIR=/usr/share/amethystora/themes

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

gsettings_get() { in_session "gsettings get $1 $2" 2>/dev/null; }
set_theme() { in_session "AMETHYSTORA_THEME_QUIET=1 amethystora-theme set '$1'" >/dev/null 2>&1; }
style_now() { gsettings_get "${STYLE_SCHEMA}" style | tr -d "'"; }
font_now() { gsettings_get org.gnome.desktop.interface font-name | tr -d "'"; }
icons_now() { gsettings_get org.gnome.desktop.interface icon-theme | tr -d "'"; }
mono_now() { gsettings_get org.gnome.desktop.interface monospace-font-name | tr -d "'"; }
cursor_now() { gsettings_get org.gnome.desktop.interface cursor-theme | tr -d "'"; }
# The pack a theme writes: its own icons.theme, else Glass for a dark theme and Glass for light for a light one
icons_of() {
    if [[ -f "${THEMES_DIR}/$1/icons.theme" ]]; then
        head -n1 "${THEMES_DIR}/$1/icons.theme"
    elif [[ -f "${THEMES_DIR}/$1/light.mode" ]]; then
        echo amethystora-glass-light
    else
        echo amethystora-glass
    fi
}
# The monospace font a theme writes: its own mono.theme, else JetBrains Mono
mono_of() {
    if [[ -f "${THEMES_DIR}/$1/mono.theme" ]]; then head -n1 "${THEMES_DIR}/$1/mono.theme"; else echo "JetBrains Mono"; fi
}

# --- The module --------------------------------------------------------------------------------------------------------
# The lock screen turns every extension off (INACTIVE), and an earlier suite may leave the screen locked: unlocked first,
# so that what follows is about the styles. One still off after that was turned off by somebody, which is said apart.
session_id() { loginctl show-user "${TEST_USER}" -p Display --value; }
screen_locked() { [[ "$(loginctl show-session "$(session_id)" -p LockedHint --value 2>/dev/null)" == yes ]]; }
screen_unlocked() { ! screen_locked; }
active() { in_session "gnome-extensions info ${UUID}" 2>&1 | grep -qE 'State: *(ACTIVE|ENABLED)'; }
if screen_locked; then
    echo "# the screen was locked: unlocking it"
    loginctl unlock-session "$(session_id)"
    wait_for 30 screen_unlocked || true
fi
if wait_for 30 active; then
    ok "Amethystora UI is active"
else
    info="$(in_session "gnome-extensions info ${UUID}" 2>&1)"
    not_ok "Amethystora UI is not active with the screen unlocked" "${info}" \
        "enabled-extensions: $(in_session 'gsettings get org.gnome.shell enabled-extensions' 2>&1)"
    in_session "gnome-extensions enable ${UUID}" >/dev/null 2>&1
    wait_for 30 active || { not_ok "and it did not come back when enabled again"; finish; }
fi

# --- Every theme's style, font and icons -------------------------------------------------------------------------------
rm -f "${CONFIG}/style" "${CONFIG}/font" "${CONFIG}/mono-font" "${CONFIG}/icons"
for dir in "${THEMES_DIR}"/*/; do
    theme="$(basename "${dir}")"
    style="$(head -n1 "${dir}style.theme")"
    font="$(head -n1 "${dir}font.theme")"
    icons="$(icons_of "${theme}")"
    set_theme "${theme}"
    got_style="$(style_now)" got_font="$(font_now)" got_icons="$(icons_now)"
    if [[ "${got_style}" == "${style}" && "${got_font}" == "${font} 11" && "${got_icons}" == "${icons}" ]]; then
        ok "${theme}: the ${style} style, ${font} and ${icons#amethystora-}"
    else
        not_ok "${theme} did not write its style, font and icons" \
            "style ${got_style} (want ${style}), font ${got_font} (want ${font} 11), icons ${got_icons} (want ${icons})"
    fi
    rendered="${CONFIG}/current/theme/shell-styles.css"
    gtk4="${HOME_DIR}/.config/gtk-4.0/gtk.css"
    gtk3="${HOME_DIR}/.config/gtk-3.0/gtk.css"
    if [[ -s "${rendered}" ]] && ! grep -q '{{' "${rendered}" &&
        grep -q "The ${style} desktop style in the applications" "${gtk4}" "${gtk3}" 2>/dev/null &&
        [[ "$(grep -l "The ${style} desktop style in the applications" "${gtk4}" "${gtk3}" 2>/dev/null | wc -l)" == 2 ]]; then
        ok "${theme}: the shell's stylesheet is rendered, and both GTK stylesheets carry the ${style} style"
    else
        not_ok "${theme}: a half of the style is missing" "$(ls "${CONFIG}/current/theme" 2>&1 | paste -sd ' ')" \
            "$(grep -h 'desktop style in the applications' "${gtk4}" "${gtk3}" 2>&1)"
    fi
    # The monospace font and the pointer of the theme, and its colours in every place an application takes them from
    mono="$(mono_of "${theme}")"
    cursor="$(head -n1 "${dir}cursor.theme")"
    if [[ "$(mono_now)" == "${mono} 11" && "$(cursor_now)" == "${cursor}" ]]; then
        ok "${theme}: ${mono} for the monospace font and ${cursor} for the pointer"
    else
        not_ok "${theme} did not write its monospace font and pointer" "$(mono_now) (want ${mono} 11), $(cursor_now) (want ${cursor})"
    fi
    # The palette for libadwaita is limited to the scheme the theme is drawn in, and not to High Contrast
    scheme=dark
    [[ -e "${dir}light.mode" ]] && scheme=light
    accent_bg="$(sed -n 's/^accent_bg = "\(#[0-9a-f]*\)"$/\1/p' "${CONFIG}/current/theme/palette.toml")"
    if [[ -n "${accent_bg}" ]] &&
        grep -qi -- "--accent-bg-color: ${accent_bg};" "${gtk4}" &&
        grep -q "@media (prefers-color-scheme: ${scheme}) and (not (prefers-contrast: more))" "${gtk4}" &&
        grep -qi -- "@define-color accent_bg_color ${accent_bg};" "${gtk3}" &&
        grep -q '^\[Colors:View\]' "${HOME_DIR}/.config/kdeglobals" &&
        [[ "$(cat "${CONFIG}/current/style.name")" == "${style}" ]] &&
        # Floating has no shape: the applications keep their own corners, and shapes.conf says nothing of it
        { [[ "${style}" == floating ]] || grep -q "^${style}.card = " "${CONFIG}/current/theme/shapes.conf"; }; then
        ok "${theme}: the palette layer is in both GTK stylesheets (accent ${accent_bg}), kdeglobals, and the apps' palette.toml and style"
    else
        not_ok "${theme}: a place the palette goes is missing" "accent ${accent_bg}" "$(head -c 300 "${gtk4}" 2>&1)"
    fi
done

log="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --no-pager -o cat 2>/dev/null)"
failed="$(grep -E 'Amethystora UI: the (style|glass) module did not' <<<"${log}")"
errors="$(grep -E 'JS ERROR' <<<"${log}" | grep -E 'amethystora-ui@.*(style|glass)' | head -n 10)"
if [[ -z "${failed}${errors}" ]]; then
    ok "the style and glass modules ran through every theme without a fault in the shell's log"
else
    not_ok "the style or glass module failed" "${failed}" "${errors}"
fi

# --- Where the shell draws the bar ---------------------------------------------------------------------------------------
# The top of the highest thing the shell shows near the top of the screen, through its accessibility tree: the bar or
# its buttons. A bar held off the edge starts below it.
mkdir -p "${OWN}"
install -m 0644 "$(dirname "$0")/atspi.py" "${OWN}/"
cat >"${OWN}/bar.py" <<'EOF'
import sys
sys.path.insert(0, "/run/vmlab-98")
from atspi import Atspi, application, walk

# The bar's buttons only: the box GNOME holds the bar in is in the tree too, and it starts at the screen's edge
# whatever margin the bar has inside it (round 6 of the combined run read 0 from it with the neon bar plainly 8 px down)
BUTTONS = {Atspi.Role.MENU, Atspi.Role.PUSH_BUTTON, Atspi.Role.TOGGLE_BUTTON}
shell = application("gnome-shell")
tops = []
for node, _ in walk(shell):
    try:
        if not node.get_state_set().contains(Atspi.StateType.SHOWING) or node.get_role() not in BUTTONS:
            continue
        box = Atspi.Component.get_extents(node, Atspi.CoordType.SCREEN)
    except Exception:
        continue
    # The bar's own row: shown, small, at the top
    if 0 <= box.y < 60 and 0 < box.height < 60 and box.width > 0:
        tops.append((box.y, node.get_role_name(), node.get_name() or ""))
if tops:
    top = min(tops)
    print(f"{top[0]} {top[1]} '{top[2]}'")
else:
    print(-1)
EOF
chmod -R a+rX "${OWN}"
# The top of the highest button, then what it is, for the log
bar_top() { user_bus python3 "${OWN}/bar.py" 2>/dev/null | tail -n 1; }
# Each style on the Amethystora theme, by the account's choice
set_theme amethystora
for entry in floating:off islands:off capsule:off segments:off neon:off hairline:on terminal:on; do
    style="${entry%%:*}" edge="${entry##*:}"
    in_session "ame desktop style ${style}" >/dev/null 2>&1
    sleep 2
    measured="$(bar_top)"
    top="${measured%% *}"
    echo "# ${style}: the highest of the bar's buttons: ${measured}"
    if [[ ! "${top}" =~ ^[0-9]+$ ]]; then
        not_ok "${style}: the shell's accessibility tree shows no button on the top bar" "${measured}"
    # A style's margin is 6 px or more; a button against the edge may still have a pixel or two of its own
    elif [[ "${edge}" == on && "${top}" -le 4 ]] || [[ "${edge}" == off && "${top}" -ge 6 ]]; then
        ok "${style}: the top bar is drawn $([[ ${edge} == on ]] && echo "against the edge" || echo "off the edge") (top at ${top})"
    else
        not_ok "${style}: the top bar is not where the style puts it" "top at ${top}, want $([[ ${edge} == on ]] && echo "0 to 4" || echo "6 or more")"
    fi
done

# --- The account's own style and font ------------------------------------------------------------------------------------
set_theme nord
in_session "ame desktop style neon" >/dev/null 2>&1
check "ame desktop style neon writes neon over Nord's islands" test "$(style_now)" == neon
set_theme gruvbox
check "and a theme switch keeps it" test "$(style_now)" == neon
in_session "ame desktop style theme" >/dev/null 2>&1
check "ame desktop style theme goes back to Gruvbox's terminal" test "$(style_now)" == terminal
if in_session "ame desktop style bubbles" >/dev/null 2>&1; then
    not_ok "ame desktop style took a style that does not exist"
else
    ok "ame desktop style refuses a style that does not exist"
fi
in_session "ame desktop font 'Space Mono'" >/dev/null 2>&1
check "ame desktop font 'Space Mono' writes it over Gruvbox's JetBrains Mono" test "$(font_now)" == "Space Mono 11"
set_theme catppuccin
check "and a theme switch keeps it" test "$(font_now)" == "Space Mono 11"
in_session "ame desktop font theme" >/dev/null 2>&1
check "ame desktop font theme goes back to Catppuccin's Nunito" test "$(font_now)" == "Nunito 11"
if in_session "ame desktop font 'No Such Font'" >/dev/null 2>&1; then
    not_ok "ame desktop font took a font that is not installed"
else
    ok "ame desktop font refuses a font that is not installed"
fi
check "a refused font leaves the theme's" test "$(font_now)" == "Nunito 11"
# The monospace font, as the interface font is
check "Catppuccin's monospace font is Fira Code" test "$(mono_now)" == "Fira Code 11"
in_session "ame desktop mono-font 'Space Mono'" >/dev/null 2>&1
check "ame desktop mono-font 'Space Mono' writes it over Catppuccin's Fira Code" test "$(mono_now)" == "Space Mono 11"
check "...and kitty's theme takes it" grep -q '^font_family Space Mono$' "${CONFIG}/current/theme/kitty-theme.conf"
set_theme gruvbox
check "and a theme switch keeps it" test "$(mono_now)" == "Space Mono 11"
in_session "ame desktop mono-font theme" >/dev/null 2>&1
check "ame desktop mono-font theme goes back to Gruvbox's JetBrains Mono" test "$(mono_now)" == "JetBrains Mono 11"
if in_session "ame desktop mono-font 'No Such Font'" >/dev/null 2>&1; then
    not_ok "ame desktop mono-font took a font that is not installed"
else
    ok "ame desktop mono-font refuses a font that is not installed"
fi
set_theme catppuccin
# The icon packs, the account's own choice over the theme's
in_session "ame desktop icons clay" >/dev/null 2>&1
check "ame desktop icons clay writes the Clay pack over Catppuccin's Duotone" test "$(icons_now)" == amethystora-clay
set_theme nord
check "and a theme switch keeps it" test "$(icons_now)" == amethystora-clay
in_session "ame desktop icons theme" >/dev/null 2>&1
check "ame desktop icons theme goes back to Nord's Muted" test "$(icons_now)" == amethystora-muted
if in_session "ame desktop icons sparkle" >/dev/null 2>&1; then
    not_ok "ame desktop icons took a pack that does not exist"
else
    ok "ame desktop icons refuses a pack that does not exist"
fi
set_theme catppuccin

# --- The fonts --------------------------------------------------------------------------------------------------------
missing="$(
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    for family in "${UI_FONTS[@]}"; do
        font_installed "${family}" || echo "${family}"
    done
)"
if [[ -z "${missing}" ]]; then
    ok "every font the themes and Control offer is installed"
else
    not_ok "a font offered is not installed" "${missing}"
fi

# --- Control ----------------------------------------------------------------------------------------------------------
list="$(user_bus systemd-run --user --pipe --wait --quiet --collect --setenv=LC_ALL=C.UTF-8 --setenv=LANGUAGE= \
    -- /usr/libexec/amethystora-control-list --json </dev/null 2>/dev/null)"
# On Catppuccin (set above) with no choice of the account's: "Follow the theme" says what the theme gives, each font
# is drawn in itself, and the theme comes first, then icons, style and font in that order (spec-look 3.2 puts Accent
# and Colours from the wallpaper right after Theme, and the comfort entries sit between style and font)
if jq -e '
    ([.items[] | select(.id == "style") | .choices[].value] | sort == (["capsule","floating","hairline","islands","neon","segments","terminal","theme"]))
    and ([.items[] | select(.id == "icons") | .choices[].value] | sort == (["clay","duotone","glass","glass-light","line","muted","neon","neon-light","theme"]))
    and ([.items[] | select(.id == "mono-font") | .choices[] | select(.value == "Fira Code") | [.command, .font]]
         == [[["ame","desktop","mono-font","Fira Code"], "Fira Code"]])
    and ([.items[] | select(.id == "mono-font") | .choices[] | select(.value == "theme") | .text] == ["The theme'"'"'s own: Fira Code"])
    and ([.items[] | select(.id == "font") | .choices[] | select(.value == "Space Grotesk") | [.command, .font]]
         == [[["ame","desktop","font","Space Grotesk"], "Space Grotesk"]])
    and ([.items[] | select(.id == "font") | .state] == ["theme"])
    and ([.items[] | select(.id == "font") | .choices[] | select(.value == "theme") | .text] == ["The theme'"'"'s own: Nunito"])
    and ([.items[] | select(.id == "style") | .choices[] | select(.value == "theme") | .text] == ["The theme'"'"'s own: Segments"])
    and ([.items[] | select(.page == "appearance") | .id] as $a | ($a[0] == "theme")
         and ((["icons", "style", "font"] | map(. as $k | $a | index([$k]))) as $at | ($at | all(. != null)) and $at == ($at | sort)))' <<<"${list}" >/dev/null 2>&1; then
    ok "Control lists the theme first, then icons, style and font, says what the theme gives, and draws each font in itself"
else
    not_ok "Control's style or font entry is wrong" "$(jq -c '.items[] | select(.id == "style" or .id == "font")' <<<"${list}" 2>&1 | head -c 800)"
fi

# --- Left open for the screenshot ----------------------------------------------------------------------------------------
set_theme tokyo-night
user_bus systemd-run --user --quiet --collect --unit=vmlab-98-files -- nautilus --new-window "${HOME_DIR}" >/dev/null 2>&1 || true
sleep 3
python3 "$(dirname "$0")/keys.py" super+s >/dev/null 2>&1 || true
sleep 2
finish
