#!/usr/bin/bash
# The theme templates (render_templates in /usr/lib/amethystora/theme/lib.sh), for 20-tests.sh. A theme is
# data shared as dotfiles, so a colour that is not a colour must refuse the theme and run nothing, every
# shipped theme must render every template completely, and a value is put in as text.

set -euo pipefail

T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
export HOME="${T}/home" XDG_CONFIG_HOME="${T}/home/.config"
mkdir -p "${XDG_CONFIG_HOME}"
# shellcheck source=/dev/null
source /usr/lib/amethystora/theme/lib.sh
fail() { echo "test-theme-render: $*" >&2; exit 1; }

# A planted theme: its accent ends the old sed command and runs one of its own
mkdir -p "${T}/evil"
cat >"${T}/evil/colors.toml" <<EOF
accent = "#9148f0"
background = "x|g;e touch ${T}/pwned;s|x|"
EOF
if render_templates "${T}/evil" "theme_name=evil" 2>"${T}/evil.err"; then
    fail "a theme whose colour is not a colour was rendered"
fi
[[ ! -e "${T}/pwned" ]] || fail "a theme's colour ran a command"
grep -q "refused" "${T}/evil.err" || fail "the refusal does not say so"
compgen -G "${T}/evil/*" | grep -vx "${T}/evil/colors.toml" && fail "a refused theme left rendered files"

# A palette with no ground or no text has nothing to work its colours from, and is refused
mkdir -p "${T}/bare"
printf 'accent = "#9148f0"\n' >"${T}/bare/colors.toml"
if render_templates "${T}/bare" "theme_name=bare" 2>"${T}/bare.err"; then
    fail "a theme with no background was rendered"
fi
grep -q "refused" "${T}/bare.err" || fail "the refusal of a theme with no background does not say so"
# ...and one too close to its own ground still renders, saying which colours could not reach the contrast WCAG asks
mkdir -p "${T}/grey"
{
    echo 'accent = "#808080"'
    echo 'background = "#808080"'
    echo 'foreground = "#858585"'
    for n in 1 2 3 4 5 6; do echo "color${n} = \"#808080\""; done
} >"${T}/grey/colors.toml"
render_templates "${T}/grey" "theme_name=grey" 2>"${T}/grey.err" || fail "a theme too close to its ground was refused"
grep -q "cannot give every colour the contrast WCAG asks" "${T}/grey.err" || fail "a palette that cannot reach its contrast was not said to"
[[ -s "${T}/grey/palette-gtk4.css" ]] || fail "such a palette was not rendered"

# Every shipped theme renders every template, with nothing left to fill
for theme in "${SYSTEM_THEMES_DIR}"/*/; do
    slug="$(basename "${theme}")"
    rm -rf "${T}/stage"
    cp -r "${theme}" "${T}/stage"
    render_templates "${T}/stage" "theme_name=${slug}" "ptyxis_palette_name=Amethystora ${slug} & co" ||
        fail "${slug} does not render"
    for template in "${SYSTEM_TEMPLATES_DIR}"/*.tpl; do
        # The shape of every style is one template, filled for each style into that style's own file (below)
        [[ "${template##*/}" == style-base.gtk.css.tpl ]] && continue
        output="${T}/stage/$(basename "${template}" .tpl)"
        [[ -f "${output}" ]] || fail "${slug}: $(basename "${template}") was not rendered"
        grep -q '{{ [A-Za-z0-9_]* }}' "${output}" && fail "${slug}: $(basename "${output}") has a value left unfilled"
        # gdk-pixbuf knows an SVG only by its <svg within the first 256 bytes; past them the wallpaper is black
        [[ "${output}" != *.svg ]] || head -c 256 "${output}" | grep -q '<svg' || fail "${slug}: $(basename "${output}") opens too late"
        # The last newline is kept, as sed kept it
        [[ "$(tail -c1 "${template}" | od -An -tx1 | tr -d ' ')" == "$(tail -c1 "${output}" | od -An -tx1 | tr -d ' ')" ]] ||
            fail "${slug}: $(basename "${output}") lost its last newline"
    done
    # Each style but floating, which leaves the applications their own corners, has its shape, with the style's radii
    for style in "${STYLES[@]}"; do
        [[ "${style}" == floating ]] && continue
        shape="${T}/stage/style-${style}.gtk.css"
        grep -q "^/\* The ${style} desktop style in the applications, for ${slug}" "${shape}" || fail "${slug}: ${style} has no shape"
        grep -q "window.csd,$" "${shape}" || fail "${slug}: ${style} has no window"
        for token in ${STYLE_SHAPE[${style}]}; do
            [[ "${token}" == window=* ]] || continue
            grep -q "border-radius: ${token#window=}px;" "${shape}" || fail "${slug}: ${style}'s windows are not ${token#window=}px"
        done
        # A style's own lines and glows come after its shape, in the same file
        [[ "${style}" == segments ]] && ! grep -q "^row:selected," "${shape}" && fail "${slug}: segments lost its own rules"
    done
    # What the terminal and the prompt read has to be accepted by the programs that read it: a key either does not
    # know would be a warning at the top of every new terminal, or colours that are not applied
    if command -v starship >/dev/null; then
        [[ -z "$(STARSHIP_CONFIG="${T}/stage/starship.toml" starship prompt 2>&1 >/dev/null)" ]] ||
            fail "${slug}: starship does not accept its prompt"
    fi
    if command -v kitty >/dev/null; then
        KITTY_THEME="${T}/stage/kitty-theme.conf" kitty +runpy '
import os
from kitty.config import load_config
bad = []
load_config(os.environ["KITTY_THEME"], accumulate_bad_lines=bad)
if bad: raise SystemExit(bad)' || fail "${slug}: kitty does not accept its theme"
    fi
done
# A value is text: & is not the matched pattern, \ is not an escape
grep -rqF "Amethystora ${slug} & co" "${T}/stage" 2>/dev/null ||
    ! grep -rqF '{{ ptyxis_palette_name }}' "${SYSTEM_TEMPLATES_DIR}" || fail "a value with & was not put in as text"

# dconf strings are escaped for GVariant
dconf() { printf '%s\n' "$3" >"${T}/dconf.value"; }
dconf_write_string /test "it's a \\ test"
[[ "$(cat "${T}/dconf.value")" == "'it\\'s a \\\\ test'" ]] || fail "dconf string not escaped: $(cat "${T}/dconf.value")"

echo "test-theme-render: every shipped theme renders, and a planted one runs nothing"
