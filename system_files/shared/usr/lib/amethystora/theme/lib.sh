# shellcheck shell=bash
# Shared helpers for amethystora-theme and amethystora-menu.
#
# A theme is a directory of small files, so that adding one needs no code:
#
#   colors.toml   the palette: accent, foreground, background, cursor, selection_*, color0..color15
#   accent.theme  the GNOME accent name to pair with it (blue teal green yellow orange red pink
#                 purple slate); without it the palette's accent is matched to the closest one
#   light.mode    present when the theme is light
#
# colors.toml may also carry success, warning and error, for a theme whose palette slots color2, color3 and color1 are
# not a green, a yellow and a red (Matte Black's, Rose Pine Dawn's): the interface's status colours come from them.
#   pair.theme    the name of the light or dark theme this one flips to on Super+Ctrl+D
#   gtk.theme     a GTK theme name, when adw-gtk3 is not wanted
#   icons.theme   an icon theme name, when the image's own is not wanted (Glass, or Glass for light for a light theme)
#   style.theme   the desktop style: one of STYLES below, the shape of the top bar, menus, notifications,
#                 dialogs, buttons and windows (floating when the theme names none)
#   font.theme    the interface font's family, when the desktop's own (Adwaita Sans) is not wanted
#   mono.theme    the monospace font's family (terminals, code, Text Editor), when JetBrains Mono is not wanted
#   cursor.theme  the pointer: Halo for a dark theme, Ink for a light one (the image's own; any installed cursor
#                 theme's name works)
#   vscode.theme  the VSCodium colour theme to select
#   backgrounds.list  wallpapers elsewhere on the system, one path per line; the first is the one
#                 the theme opens on. Every shipped theme lists its pictures here.
#   daylight.list the opening picture at four times of day, exactly four lines `dawn PATH`, `day PATH`, `dusk PATH`
#                 and `night PATH` in that order, which Daylight shows as a slideshow GNOME fades through by itself
#   backgrounds/  wallpapers to cycle with Super+Ctrl+Space
#
# Themes ship read-only in /usr/share/amethystora/themes. A directory of the same name in
# ~/.config/amethystora/themes is copied over the top, so a theme can be tweaked file by file, and
# a directory that exists only there is a theme of the user's own.
#
# Applying a theme renders /usr/share/amethystora/themed/*.tpl against colors.toml and swaps the
# result into ~/.config/amethystora/current/theme. Everything downstream reads that one path, so
# nothing else has to know which theme is current.

SYSTEM_THEMES_DIR=/usr/share/amethystora/themes
SYSTEM_TEMPLATES_DIR=/usr/share/amethystora/themed
SYSTEM_HOOKS_DIR=/usr/share/amethystora/theme-set.hooks.d

CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora"
USER_THEMES_DIR="${CONFIG_DIR}/themes"
USER_TEMPLATES_DIR="${CONFIG_DIR}/themed"
USER_HOOKS_DIR="${CONFIG_DIR}/hooks/theme-set.d"
USER_BACKGROUNDS_DIR="${CONFIG_DIR}/backgrounds"
CURRENT_DIR="${CONFIG_DIR}/current"
CURRENT_THEME_DIR="${CURRENT_DIR}/theme"
CURRENT_THEME_NAME="${CURRENT_DIR}/theme.name"
CURRENT_BACKGROUND="${CURRENT_DIR}/background"

DEFAULT_THEME=amethystora
# The wallpaper of an image of one's own built on Amethystora (amethystora-brand), every theme's first
IMAGE_BACKGROUNDS_DIR=/usr/share/amethystora/image-setup/backgrounds
# ...and its look, which every account starts with before its theme's suggestion: style, icons, font, mono-font
IMAGE_SETUP_DIR=/usr/share/amethystora/image-setup
# The image's icon packs, drawn by branding/icons.mjs, each /usr/share/icons/amethystora-<pack>. Each theme names the
# one that suits its colours and its style (icons.theme); when it names none, Glass is the pack of a dark theme and
# Glass for light of a light one, whose colours are deepened to read on a light ground. Glass is also the default in
# zz1-amethystora-modifications, for accounts that have not applied a theme yet; the two are checked against each
# other in 20-tests.sh.
DEFAULT_ICON_THEME=amethystora-glass
DEFAULT_LIGHT_ICON_THEME=amethystora-glass-light
ICON_PACKS=(glass glass-light neon neon-light duotone line clay muted)
# The image's pointers, drawn by branding/cursors.mjs, each /usr/share/icons/amethystora-pointer-<set>: Halo for a dark
# theme and Ink for a light one (each theme's cursor.theme names its own), Facet for whoever chooses it. Halo is also
# the default in zz1-amethystora-modifications, for the login screen and an account before its first theme.
DEFAULT_POINTER=amethystora-pointer-halo
DEFAULT_LIGHT_POINTER=amethystora-pointer-ink
POINTER_SETS=(halo ink facet)

# The desktop styles, each drawn by Amethystora UI (themed/shell-styles.css.tpl) and in the applications
# (themed/style-<style>.gtk.css.tpl): the shape of the top bar, menus, notifications, dialogs, on-screen alerts,
# buttons, controls and windows. Floating is the desktop as it was before there were styles, and the schema's
# default, for accounts that have not applied a theme yet.
STYLES=(islands floating hairline capsule terminal segments neon)
DEFAULT_STYLE=floating
# What each style makes of the applications, in pixels (themed/style-base.gtk.css.tpl): the corners of a window, a dialog,
# a control (button, entry, tab, menu row), a card or list, a popover, a tile (an item of a grid or a selection), a check
# box, a switch and its knob, a tooltip, a toast, and the trough of a slider or a progress bar. A window's corners are the
# glass's too (modules/style.js), and 20-tests.sh holds the two to each other. Floating has none: the applications
# keep their own corners.
declare -gA STYLE_SHAPE=(
    [islands]="window=16 dialog=19 control=12 card=16 popover=18 tile=12 check=7 switch=999 knob=999 tooltip=10 toast=999 bar=999"
    [hairline]="window=6 dialog=8 control=4 card=6 popover=6 tile=4 check=3 switch=4 knob=3 tooltip=4 toast=6 bar=2"
    [capsule]="window=18 dialog=21 control=999 card=20 popover=24 tile=20 check=8 switch=999 knob=999 tooltip=16 toast=999 bar=999"
    [terminal]="window=0 dialog=0 control=0 card=0 popover=0 tile=0 check=0 switch=0 knob=0 tooltip=0 toast=0 bar=0"
    [segments]="window=12 dialog=15 control=8 card=12 popover=12 tile=8 check=6 switch=999 knob=999 tooltip=8 toast=10 bar=999"
    [neon]="window=10 dialog=13 control=8 card=10 popover=10 tile=8 check=6 switch=999 knob=999 tooltip=8 toast=10 bar=999"
)
# The interface font when neither the account nor the theme names one: zz0-amethystora-modifications' own
DEFAULT_FONT="Adwaita Sans"
FONT_SIZE=11
# The families offered for the interface (ame desktop font, Control): the desktop's own and the ones the themes are
# written in, each installed by 04-packages.sh or 09-fonts.sh. Any other installed family can be typed.
UI_FONTS=("Adwaita Sans" "Atkinson Hyperlegible Next" Comfortaa "DM Sans" Figtree "Fira Sans" Geist "IBM Plex Mono" "IBM Plex Sans" Inter
    "JetBrains Mono" Karla Lexend Manrope Nunito Outfit "Plus Jakarta Sans" Quicksand Rubik Sora "Space Grotesk"
    "Space Mono" "Work Sans")
# The monospace font (terminals, code, Text Editor, anything asking GNOME for the system's fixed-width face) when neither
# the account nor the theme names one, and the families `ame desktop mono-font` and Control offer: each installed by
# 04-packages.sh or 09-fonts.sh. Any other installed family can be typed.
DEFAULT_MONO_FONT="JetBrains Mono"
MONO_FONT_SIZE=11
MONO_FONTS=("Adwaita Mono" "Atkinson Hyperlegible Mono" "DM Mono" "Fira Code" "Geist Mono" "IBM Plex Mono" "JetBrains Mono" "Space Mono" "Victor Mono")

# The twelve accents any theme can take in place of its own (ame desktop accent, Control's Accent), in the order Control
# shows them, each an id and the fill a button, a switch or a selection is drawn in. Every one carries a white label at
# 4.5:1 (Sky, Lagoon and Amber are a little darker than the boards drew them for that), so palette.awk keeps the fill as
# it is in every theme; test-theme-palette.sh measures each of them on every theme's surfaces, and 20-tests.sh holds the
# list to Control's, the recipe's and the setup file's.
ACCENTS=(
    "amethyst #9148f0"
    "iris #5b5cf0"
    "sky #2172e2"
    "lagoon #008194"
    "jade #16865c"
    "moss #5b7f1f"
    "amber #a66800"
    "ember #c6531c"
    "coral #d63b5c"
    "rose #c2338f"
    "orchid #a43fc7"
    "slate #5b6680"
)
# Colours from the wallpaper (ame desktop colours): the ways /usr/libexec/amethystora-wallpaper-colours works them out,
# and the only keys of its output that are merged over a theme's colors.toml
COLOUR_WAYS=(vivid soft mono)
WALLPAPER_COLOUR_KEYS=(background foreground accent selection_background)
WALLPAPER_COLOURS="${CONFIG_DIR}/wallpaper-colours.toml"
# Daylight (ame desktop daylight): the four times of day in the order daylight.list holds them, the moment each
# picture is fully shown (minutes after midnight), and how long the fade into each takes (minutes)
DAYLIGHT_TIMES=(dawn day dusk night)
DAYLIGHT_AT=(390 720 1125 1320)
DAYLIGHT_FADE=(45 90 90 60)

# GNOME's accent colours, with a representative sRGB value for each, used to pick the closest one
# for a theme that does not name its own
GNOME_ACCENTS=(
    "blue #3584e4"
    "teal #2190a4"
    "green #3a944a"
    "yellow #c88800"
    "orange #ed5b00"
    "red #e62d42"
    "pink #d56199"
    "purple #9141ac"
    "slate #6f8396"
)

die() {
    echo "${0##*/}: $*" >&2
    exit 1
}

# A theme name as it is typed ("Tokyo Night") to the directory name (tokyo-night)
theme_slug() {
    # Never only dots: ".." would name the folder above the themes
    echo "$1" | tr '[:upper:]' '[:lower:]' | sed -e 's/[^a-z0-9._-]\+/-/g' -e 's/^[.-]*//' -e 's/-$//'
}

theme_exists() {
    local slug="$1"
    [[ -d "${SYSTEM_THEMES_DIR}/${slug}" || -d "${USER_THEMES_DIR}/${slug}" ]]
}

# Every theme, shipped and user-made, one name per line
theme_list() {
    {
        [[ -d ${SYSTEM_THEMES_DIR} ]] && find "${SYSTEM_THEMES_DIR}" -mindepth 1 -maxdepth 1 -type d -printf '%f\n'
        [[ -d ${USER_THEMES_DIR} ]] && find "${USER_THEMES_DIR}" -mindepth 1 -maxdepth 1 -type d -printf '%f\n'
    } 2>/dev/null | sort -u
}

theme_current() {
    [[ -f ${CURRENT_THEME_NAME} ]] && cat "${CURRENT_THEME_NAME}"
}

# The value of a one-line file in the current theme, or nothing when the theme does not ship it.
# Missing is normal, not an error: every one of these files is optional.
theme_file() {
    local name="$1"
    if [[ -f "${CURRENT_THEME_DIR}/${name}" ]]; then
        head -n1 "${CURRENT_THEME_DIR}/${name}"
    fi
    return 0
}

theme_is_light() {
    [[ -f "${CURRENT_THEME_DIR}/light.mode" ]]
}

# Whether an icon theme of that name is installed, for the machine or for the account
icon_theme_installed() {
    local dir
    for dir in /usr/share/icons "${XDG_DATA_HOME:-${HOME}/.local/share}/icons" "${HOME}/.icons"; do
        [[ -f "${dir}/$1/index.theme" ]] && return 0
    done
    return 1
}

# The icon theme to write: the account's own choice (ame desktop icons), else its image's (amethystora-brand), else the
# one the theme names, else the
# image's, by whether the theme is dark or light. A name that is not installed is passed over, whoever wrote it:
# candy-icons, which an earlier image had, would leave every icon to the fallback.
icon_theme() {
    local icons="" name
    for name in "$(head -n1 "${CONFIG_DIR}/icons" 2>/dev/null || true)" "$(head -n1 "${IMAGE_SETUP_DIR}/icons" 2>/dev/null || true)" \
        "$(theme_file icons.theme)"; do
        if [[ -n ${name} ]] && icon_theme_installed "${name}"; then
            icons="${name}"
            break
        fi
    done
    if [[ -z ${icons} ]]; then
        if theme_is_light; then icons="${DEFAULT_LIGHT_ICON_THEME}"; else icons="${DEFAULT_ICON_THEME}"; fi
    fi
    echo "${icons}"
}

is_icon_pack() {
    local pack
    for pack in "${ICON_PACKS[@]}"; do
        [[ $1 == "${pack}" ]] && return 0
    done
    return 1
}

# Whether a cursor theme of that name is installed, for the machine or for the account
cursor_theme_installed() {
    local dir
    for dir in /usr/share/icons "${XDG_DATA_HOME:-${HOME}/.local/share}/icons" "${HOME}/.icons"; do
        [[ -f "${dir}/$1/cursors/default" || -f "${dir}/$1/cursors/left_ptr" ]] && return 0
    done
    return 1
}

is_pointer_set() {
    local set
    for set in "${POINTER_SETS[@]}"; do
        [[ $1 == "${set}" ]] && return 0
    done
    return 1
}

# The pointer to write: the account's own choice (ame desktop pointer), else the one the theme names, else the image's,
# by whether the theme is dark or light. A name that is not installed is passed over, whoever wrote it; Adwaita when
# nothing is left (an image without the sets).
pointer_theme() {
    local name
    for name in "$(head -n1 "${CONFIG_DIR}/pointer" 2>/dev/null || true)" "$(theme_file cursor.theme)"; do
        if [[ -n ${name} ]] && cursor_theme_installed "${name}"; then
            echo "${name}"
            return 0
        fi
    done
    if theme_is_light; then name="${DEFAULT_LIGHT_POINTER}"; else name="${DEFAULT_POINTER}"; fi
    if cursor_theme_installed "${name}"; then echo "${name}"; else echo Adwaita; fi
}

is_style() {
    local style
    for style in "${STYLES[@]}"; do
        [[ $1 == "${style}" ]] && return 0
    done
    return 1
}

# The desktop style to write: the account's own choice (ame desktop style), else its image's, else the theme's, else the
# default.
# A name that is no style, whoever wrote it, is passed over.
desktop_style() {
    local name
    for name in "$(head -n1 "${CONFIG_DIR}/style" 2>/dev/null || true)" "$(head -n1 "${IMAGE_SETUP_DIR}/style" 2>/dev/null || true)" \
        "$(theme_file style.theme)"; do
        if [[ -n ${name} ]] && is_style "${name}"; then
            echo "${name}"
            return 0
        fi
    done
    echo "${DEFAULT_STYLE}"
}

# Whether fontconfig knows a family of exactly that name. The list is read to its end, never with grep -q, which
# would stop fc-list mid-write and, under pipefail, fail the whole pipe now and then.
font_installed() {
    [[ -n $1 ]] && fc-list : family 2>/dev/null | tr ',' '\n' | grep -Fx -- "$1" >/dev/null
}

# The interface font's family: the account's own choice (ame desktop font), else its image's, else the theme's, else the
# desktop's.
# A family that is not installed is passed over, as an icon theme is.
ui_font() {
    local name
    for name in "$(head -n1 "${CONFIG_DIR}/font" 2>/dev/null || true)" "$(head -n1 "${IMAGE_SETUP_DIR}/font" 2>/dev/null || true)" \
        "$(theme_file font.theme)"; do
        if font_installed "${name}"; then
            echo "${name}"
            return 0
        fi
    done
    echo "${DEFAULT_FONT}"
}

# The monospace font's family: the account's own choice (ame desktop mono-font), else its image's, else the theme's (mono.theme in the
# folder given, the current theme's when none is), else JetBrains Mono. A family that is not installed is passed over.
mono_font() {
    local dir="${1:-${CURRENT_THEME_DIR}}" name
    for name in "$(head -n1 "${CONFIG_DIR}/mono-font" 2>/dev/null || true)" "$(head -n1 "${IMAGE_SETUP_DIR}/mono-font" 2>/dev/null || true)" \
        "$(head -n1 "${dir}/mono.theme" 2>/dev/null || true)"; do
        if font_installed "${name}"; then
            echo "${name}"
            return 0
        fi
    done
    echo "${DEFAULT_MONO_FONT}"
}

# A colour from a theme's colors.toml, e.g. colors_get <dir> accent -> #7c3aed
colors_get() {
    local dir="$1" key="$2"
    sed -nE "s/^[[:space:]]*['\"]?${key}['\"]?[[:space:]]*=[[:space:]]*['\"]([^'\"]+)['\"].*/\1/p" \
        "${dir}/colors.toml" 2>/dev/null | head -n1
}

# A line of colors.toml set to KEY = "VALUE", in place of the one it had or added at the end: the staged theme takes the
# account's accent and the wallpaper's colours key by key, so that every other line (the terminal's colours, the cursor)
# stays the theme's. Only a key and a colour of the expected shapes are ever written.
colors_merge() {
    local file="$1" key="$2" value="$3"
    [[ ${key} =~ ^[a-z][a-z0-9_]*$ && ${value} =~ ^#[0-9a-f]{6}$ && -f ${file} ]] || return 1
    awk -v key="${key}" -v line="${key} = \"${value}\"" '
        { gsub(/\r$/, "") }
        $0 ~ "^[[:space:]]*[\"'\'']?" key "[\"'\'']?[[:space:]]*=" { if (!done) print line; done = 1; next }
        { print }
        END { if (!done) print line }' "${file}" >"${file}.new" && mv "${file}.new" "${file}"
}

# --- Accents ----------------------------------------------------------------------------------

is_accent() {
    local entry
    for entry in "${ACCENTS[@]}"; do
        [[ $1 == "${entry%% *}" ]] && return 0
    done
    return 1
}

# The fill of one of the twelve accents, by its id
accent_fill() {
    local entry
    for entry in "${ACCENTS[@]}"; do
        if [[ $1 == "${entry%% *}" ]]; then
            echo "${entry##* }"
            return 0
        fi
    done
    return 1
}

# The accent the account chose (ame desktop accent), else its image's (amethystora-setup bake): an id, or nothing for the
# theme's own. The account's "theme" is a choice too, which an image's accent does not override. Anything else in either
# file counts as nothing, so that a stale or hand-edited file never stops a switch.
account_accent() {
    local name
    for name in "$(head -n1 "${CONFIG_DIR}/accent" 2>/dev/null || true)" "$(head -n1 "${IMAGE_SETUP_DIR}/accent" 2>/dev/null || true)"; do
        [[ ${name} == theme ]] && return 0
        if is_accent "${name}"; then
            echo "${name}"
            return 0
        fi
    done
    return 0
}

# The accents the workspaces name while each has its own (Amethystora UI's theme module, ame desktop
# accent-per-workspace), each once and only the twelve's ids: nothing while the feature is off. Read with dconf, which
# needs no schema; a list never written is the schema's default, WORKSPACE_ACCENTS. A focus profile's accent while one
# is on comes too, whatever the workspaces say: the shell names it in AMETHYSTORA_FOCUS_ACCENT when it asks.
WORKSPACE_ACCENTS=(theme sky coral jade amber orchid)
workspace_accent_ids() {
    local path=/org/gnome/shell/extensions/amethystora-ui/theme list id
    local -a ids=()
    # AMETHYSTORA_ACCENTS=on: as if it were on, for the command that turns it on, which renders the sets first
    if [[ ${AMETHYSTORA_ACCENTS:-} == on || "$(dconf read "${path}/accent-per-workspace" 2>/dev/null)" == true ]]; then
        list="$(dconf read "${path}/workspace-accents" 2>/dev/null || true)"
        if [[ -n ${list} ]]; then
            mapfile -t ids < <(grep -oE "'[a-z]+'" <<<"${list}" | tr -d "'")
        else
            ids=("${WORKSPACE_ACCENTS[@]}")
        fi
    fi
    [[ -n ${AMETHYSTORA_FOCUS_ACCENT:-} ]] && ids+=("${AMETHYSTORA_FOCUS_ACCENT}")
    # A theme switch (ACCENTS_KEEP=1) renders again every set the current theme has, a focus profile's included: the
    # shell may have its rules loaded, and GNOME Shell, which loads every stylesheet loaded again when light and dark
    # change, stops at one that is gone. `amethystora-theme accents` alone takes a set away.
    if [[ -n ${ACCENTS_KEEP:-} && -d "${CURRENT_THEME_DIR}/accents" ]]; then
        for id in "${CURRENT_THEME_DIR}"/accents/*/; do
            [[ -d ${id} ]] && id="${id%/}" && ids+=("${id##*/}")
        done
    fi
    ((${#ids[@]} > 0)) || return 0
    for id in "${ids[@]}"; do
        is_accent "${id}" && echo "${id}"
    done | awk '!seen[$0]++'
}

# The rules of a shell stylesheet rendered with another accent that differ from the theme's own rendering of the same
# template, each selector put under the class Amethystora UI gives the shell's top group (#uiGroup) while a workspace of
# that accent is shown: `#uiGroup.amethystora-accent-ID` where the selector names the top group, `.amethystora-accent-ID `
# before it where not. One class more than the theme's own rule, so it wins over that rule and keeps the order the
# theme's rules have among themselves. The two files are renderings of one template, so their rules pair up in order.
# Mind the load order: at login the overlays load before the modules' own stylesheets and after a theme switch after
# them, and St gives a tie to the stylesheet loaded first. A module rule exactly one class more specific than a theme
# rule that carries the accent would tie with the overlay and win or lose by that order: give such a rule a class more.
css_accent_overlay() {
    local base="$1" accent="$2" id="$3"
    awk -v id="${id}" '
        # The text with its comments taken out
        function bare(t,   out, i, j) {
            out = ""
            while ((i = index(t, "/*")) > 0) {
                j = index(substr(t, i + 2), "*/")
                if (j == 0) { t = substr(t, 1, i - 1); break }
                out = out substr(t, 1, i - 1)
                t = substr(t, i + j + 3)
            }
            return out t
        }
        function blocks(t, list,   n) {
            n = split(t, list, "}")
            return n
        }
        function under(selectors,   n, part, k, s, out) {
            n = split(selectors, part, ",")
            out = ""
            for (k = 1; k <= n; k++) {
                s = part[k]
                gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
                if (s == "") continue
                if (s ~ /^#uiGroup([.:[:space:]]|$)/) s = "#uiGroup.amethystora-accent-" id substr(s, 9)
                else s = ".amethystora-accent-" id " " s
                out = out (out == "" ? "" : ",\n") s
            }
            return out
        }
        FNR == 1 { file++ }
        { text[file] = text[file] $0 "\n" }
        END {
            n = blocks(bare(text[1]), A)
            m = blocks(bare(text[2]), B)
            if (n != m) exit 1
            for (k = 1; k < n; k++) {
                if (A[k] == B[k]) continue
                split(B[k], rule, "{")
                selectors = rule[1]
                sub(/^[^{]*\{/, "", B[k])
                printf "%s {%s}\n", under(selectors), B[k]
            }
        }' "${base}" "${accent}"
}

# An accent per workspace (ame desktop accent-per-workspace): for each accent the workspaces name, the shell's
# stylesheets and the Amethystora apps' palette rendered with it over the staged colours, into accents/<id>/ beside the
# theme's: shell-accent.css, the shell's rules that the accent changes, under a class Amethystora UI's theme module gives
# the shell's top group while a workspace of that accent is shown, and palette.toml, which the apps read in place of the
# theme's then. Each is rendered from the theme's own folders, as stage_theme copies them, with the staged colours: a
# file the theme ships itself (mono.theme, a hand-written stylesheet) is then the same for every accent and stays out of
# the overlay. Nothing while the feature is off.
render_accent_sets() {
    local staging="$1" slug="$2" id dir file
    rm -rf "${staging}/accents"
    local -a ids
    mapfile -t ids < <(workspace_accent_ids)
    ((${#ids[@]} > 0)) || return 0
    mkdir -p "${staging}/accents"
    for id in "${ids[@]}"; do
        dir="${staging}/accents/${id}"
        mkdir -p "${dir}"
        # The theme's own files only, the top of its folder, which is all render_templates reads: not its pictures
        find "${SYSTEM_THEMES_DIR}/${slug}" "${USER_THEMES_DIR}/${slug}" -mindepth 1 -maxdepth 1 -type f \
            -exec cp -f -t "${dir}" {} + 2>/dev/null || true
        chmod -R u+w "${dir}"
        cp "${staging}/colors.toml" "${dir}/colors.toml"
        colors_merge "${dir}/colors.toml" accent "$(accent_fill "${id}")"
        if ! RENDER_ONLY='^(shell-[a-z]+\.css|palette\.toml)\.tpl$' render_templates "${dir}" "theme_name=${slug}" 2>/dev/null; then
            rm -rf "${dir}"
            continue
        fi
        # The shell takes only the rules the accent changes (css_accent_overlay), so that a workspace switch changes a
        # class rather than reloading stylesheets, which restyles the whole stage each time
        for file in "${dir}"/shell-*.css; do
            if [[ -f ${file} && -f "${staging}/${file##*/}" ]]; then
                css_accent_overlay "${staging}/${file##*/}" "${file}" "${id}" || true
            fi
        done >"${dir}/.shell-accent.css"
        # The apps' palette, where the accent changes it, and nothing else of the theme's
        if cmp -s "${dir}/palette.toml" "${staging}/palette.toml"; then
            rm -f "${dir}/palette.toml"
        fi
        find "${dir}" -mindepth 1 -maxdepth 1 ! -name palette.toml ! -name .shell-accent.css -exec rm -rf {} +
        mv "${dir}/.shell-accent.css" "${dir}/shell-accent.css"
    done
}

# One amethystora-theme at a time for the account: a theme switch, the wallpaper's colours, the accents' sets and Daylight
# all write into current/, and the shell starts some of them by itself. Taken again by the same process, it is a no-op.
# The lock is beside current/, not in it, so that taking current/ away never makes a second lock. Nothing waits for it
# for ever: after two minutes the command gives up and says so. Children that may outlive the command (the theme's
# hooks) are run with its descriptor closed (THEME_LOCK_FD), so that a daemon a hook starts never holds it.
theme_lock() {
    [[ -n ${AMETHYSTORA_THEME_LOCKED:-} ]] && return 0
    mkdir -p "${CONFIG_DIR}"
    exec {THEME_LOCK_FD}>"${CONFIG_DIR}/.theme.lock"
    flock -w 120 "${THEME_LOCK_FD}" ||
        die "another amethystora-theme has been working for two minutes and is still not done: try again once it is"
    export AMETHYSTORA_THEME_LOCKED=1
}

# --- Colours from the wallpaper -----------------------------------------------------------------

is_colour_way() {
    local way
    for way in "${COLOUR_WAYS[@]}"; do
        [[ $1 == "${way}" ]] && return 0
    done
    return 1
}

# The way the account takes its colours from the wallpaper (ame desktop colours), else its image's, else nothing (off).
# The account's "off" is a choice too, which an image's way does not override.
colours_way() {
    local name
    for name in "$(head -n1 "${CONFIG_DIR}/colours" 2>/dev/null || true)" "$(head -n1 "${IMAGE_SETUP_DIR}/colours" 2>/dev/null || true)"; do
        [[ ${name} == off ]] && return 0
        if is_colour_way "${name}"; then
            echo "${name}"
            return 0
        fi
    done
    return 0
}

# The folder a theme's own colors.toml is read from: the account's copy of the theme when it has one, else the image's
theme_source_dir() {
    if [[ -f "${USER_THEMES_DIR}/$1/colors.toml" ]]; then
        echo "${USER_THEMES_DIR}/$1"
    else
        echo "${SYSTEM_THEMES_DIR}/$1"
    fi
}

theme_source_is_light() {
    [[ -f "${SYSTEM_THEMES_DIR}/$1/light.mode" || -f "${USER_THEMES_DIR}/$1/light.mode" ]]
}

# The wallpaper's colours over a staged theme (stage_theme): each of WALLPAPER_COLOUR_KEYS the helper wrote, when the
# account takes colours from the wallpaper in the way they were worked out in, for this theme's mode, ground and text
# (the stamp line says which). Anything else in the file is passed over.
colours_apply() {
    local staging="$1" slug="$2" way mode stamp key value
    way="$(colours_way)"
    [[ -n ${way} && -f ${WALLPAPER_COLOURS} ]] || return 0
    if [[ -f "${staging}/light.mode" ]]; then mode=light; else mode=dark; fi
    stamp="$(sed -n 's/^# stamp //p' "${WALLPAPER_COLOURS}" | head -n1)"
    local -a fields
    read -r -a fields <<<"${stamp}"
    [[ ${#fields[@]} -ge 7 && ${fields[0]} == "${way}" && ${fields[1]} == "${mode}" ]] || return 0
    # The ground and text it was worked from, the last two fields: a picture's path may hold spaces
    local own
    own="$(theme_source_dir "${slug}")"
    [[ ${fields[-2]} == "$(colors_get "${own}" background | tr '[:upper:]' '[:lower:]')" &&
        ${fields[-1]} == "$(colors_get "${own}" foreground | tr '[:upper:]' '[:lower:]')" ]] || return 0
    while IFS='=' read -r key value; do
        key="${key//[[:space:]]/}"
        value="${value//[[:space:]\"\']/}"
        [[ " ${WALLPAPER_COLOUR_KEYS[*]} " == *" ${key} "* && ${value} =~ ^#[0-9a-f]{6}$ ]] || continue
        colors_merge "${staging}/colors.toml" "${key}" "${value}"
    done < <(grep -v '^[[:space:]]*#' "${WALLPAPER_COLOURS}")
}

# --- Daylight -----------------------------------------------------------------------------------

# Daylight is on unless the account turned it off (ame desktop daylight off writes "off")
daylight_on() {
    [[ "$(head -n1 "${CONFIG_DIR}/daylight" 2>/dev/null || true)" != off ]]
}

# The four pictures of a theme folder's daylight.list (the current theme's when none is given), one per line in the order
# of DAYLIGHT_TIMES, when the list is exactly that: four lines, the four words in order, each an absolute path to a
# picture that is a regular file. A list that is not is no Daylight for the theme, and says so.
daylight_pictures() {
    local dir="${1:-${CURRENT_THEME_DIR}}" list line word path i=0
    list="${dir}/daylight.list"
    [[ -f ${list} ]] || return 1
    local -a pictures=()
    while IFS= read -r line || [[ -n ${line} ]]; do
        line="${line%$'\r'}"
        word="${line%% *}"
        path="${line#* }"
        if ((i >= 4)) || [[ ${word} != "${DAYLIGHT_TIMES[i]}" || ${path} != /* || ! -f ${path} ||
            ! ${path,,} =~ \.(jpg|jpeg|png|webp|svg)$ ]]; then
            echo "${0##*/}: ${list} is not four pictures for dawn, day, dusk and night: no Daylight for this theme" >&2
            return 1
        fi
        pictures+=("${path}")
        i=$((i + 1))
    done <"${list}"
    if ((i != 4)); then
        echo "${0##*/}: ${list} is not four pictures for dawn, day, dusk and night: no Daylight for this theme" >&2
        return 1
    fi
    printf '%s\n' "${pictures[@]}"
}

# The minutes after midnight at which dawn, day, dusk and night are each fully shown: Night Light's hours when it is on
# with hours of its own (its automatic schedule needs the location, which Daylight does not ask for), within bounds that
# keep every picture at least a quarter of an hour, else DAYLIGHT_AT
daylight_minutes() {
    local schema=org.gnome.settings-daemon.plugins.color from to
    if [[ "$(gsettings get "${schema}" night-light-enabled 2>/dev/null)" == true &&
        "$(gsettings get "${schema}" night-light-schedule-automatic 2>/dev/null)" == false ]]; then
        from="$(gsettings get "${schema}" night-light-schedule-from 2>/dev/null)"
        to="$(gsettings get "${schema}" night-light-schedule-to 2>/dev/null)"
        if [[ ${from} =~ ^[0-9]+(\.[0-9]+)?$ && ${to} =~ ^[0-9]+(\.[0-9]+)?$ ]] &&
            awk -v from="${from}" -v to="${to}" 'BEGIN {
                if (to < 3 || to > 10 || from < 16 || from > 22) exit 1
                dawn = to * 60; dusk = from * 60
                day = dawn + 330 < dusk - 150 ? dawn + 330 : dusk - 150
                night = dusk + 195 < 1425 ? dusk + 195 : 1425
                printf "%d %d %d %d\n", int(dawn + 0.5), int(day + 0.5), int(dusk + 0.5), int(night + 0.5)
            }'; then
            return 0
        fi
    fi
    echo "${DAYLIGHT_AT[*]}"
}

# Daylight's day, as GNOME's slideshow takes it: from midnight, the night picture, the fade into dawn ending at dawn's
# time, dawn, and so on round to the night again until midnight, nine slides of `static SECONDS N` or `fade SECONDS N M`
# (N, M the pictures by their place in DAYLIGHT_TIMES), 86400 seconds in all
daylight_slides() {
    local -a at
    read -r -a at <<<"$(daylight_minutes)"
    awk -v a="${at[*]}" -v f="${DAYLIGHT_FADE[*]}" 'BEGIN {
        split(a, at, " "); split(f, fade, " ")
        from = 0; previous = 4
        for (k = 1; k <= 4; k++) {
            start = at[k] - fade[k]
            if (start - from < 15) exit 1
            printf "static %d %d\nfade %d %d %d\n", (start - from) * 60, previous, fade[k] * 60, previous, k
            from = at[k]; previous = k
        }
        if (1440 - from < 15) exit 1
        printf "static %d 4\n", (1440 - from) * 60
    }'
}

# Text put into XML as text. The replacements are quoted: bash 5.2 reads an unquoted & in one as the text it matched.
xml_text() {
    local text="${1//&/'&amp;'}"
    text="${text//</'&lt;'}"
    printf '%s' "${text//>/'&gt;'}"
}

# The slideshow for a theme's four pictures (one per line on stdin, in the order of DAYLIGHT_TIMES), starting at local
# midnight of the date given as YYYY MM DD
daylight_xml() {
    local year="$1" month="$2" day="$3" kind seconds a b
    local -a pictures
    mapfile -t pictures
    ((${#pictures[@]} == 4)) || return 1
    local slides
    slides="$(daylight_slides)" || return 1
    echo "<background>"
    printf '  <starttime><year>%d</year><month>%d</month><day>%d</day><hour>0</hour><minute>0</minute><second>0</second></starttime>\n' \
        "$((10#${year}))" "$((10#${month}))" "$((10#${day}))"
    while read -r kind seconds a b; do
        if [[ ${kind} == static ]]; then
            printf '  <static><duration>%d</duration><file>%s</file></static>\n' "${seconds}" "$(xml_text "${pictures[a - 1]}")"
        else
            printf '  <transition type="overlay"><duration>%d</duration><from>%s</from><to>%s</to></transition>\n' \
                "${seconds}" "$(xml_text "${pictures[a - 1]}")" "$(xml_text "${pictures[b - 1]}")"
        fi
    done <<<"${slides}"
    echo "</background>"
}

# Which of the four pictures is shown at this minute of the day (the one a fade is more than half way to), as its place
# in DAYLIGHT_TIMES counting from 0
daylight_index_now() {
    local now
    now=$((10#$(date +%H) * 60 + 10#$(date +%M)))
    daylight_slides | awk -v now="${now}" '
        { end = start + $2 / 60 }
        $1 == "static" && now < end { print $3 - 1; exit }
        $1 == "fade" && now < end { print (now - start < (end - start) / 2 ? $3 : $4) - 1; exit }
        { start = end }'
}

# --- Wallpapers drawn from the theme ------------------------------------------------------------

# The seven patterns pattern.awk draws (ame desktop pattern), in the order Control shows them
PATTERNS=(facets waves contour mesh grain dots bands)
PATTERN_DIR="${CURRENT_DIR}/patterns"
PATTERN_PREVIEWS="${XDG_CACHE_HOME:-${HOME}/.cache}/amethystora/patterns"

is_pattern() {
    local pattern
    for pattern in "${PATTERNS[@]}"; do
        [[ $1 == "${pattern}" ]] && return 0
    done
    return 1
}

# The drawn wallpaper the account chose, as `PATTERN SEED`, or nothing while it is off: ~/.config/amethystora/pattern,
# its first line a pattern and its second the seed Shuffle last drew (1 when there is none). Anything else counts as off.
account_pattern() {
    local name="" seed=""
    [[ -r "${CONFIG_DIR}/pattern" ]] || return 0
    { IFS= read -r name; IFS= read -r seed; } <"${CONFIG_DIR}/pattern" || true
    name="${name%$'\r'}"
    seed="${seed%$'\r'}"
    is_pattern "${name}" || return 0
    [[ ${seed} =~ ^[0-9]{1,9}$ ]] || seed=1
    echo "${name} $((10#${seed}))"
}

# What a pattern is drawn from, as pattern.awk reads it: the palette of a theme folder, its lightness and the choice
pattern_input() {
    local dir="$1" name="$2" seed="$3" tone="${4:-}"
    # The keys it draws with, each the first line that gives it (as colors_get reads them), in one read of the file
    awk 'match($0, /^[[:space:]]*["\047]?(background|foreground|accent|color[1-6])["\047]?[[:space:]]*=[[:space:]]*["\047](#[0-9a-fA-F]{6})["\047]/, m) &&
        !seen[m[1]]++ { print m[1] "=" tolower(m[2]) }' "${dir}/colors.toml" 2>/dev/null
    [[ -f "${dir}/light.mode" ]] && echo "light=1"
    echo "pattern=${name}"
    echo "seed=${seed}"
    [[ -n ${tone} ]] && echo "tone=${tone}"
    return 0
}

# A short name for what a drawing is drawn from, so that a new palette or seed is a new file: GNOME keeps a picture by
# its path, and a drawing written over the one it shows would not be shown again
pattern_hash() {
    pattern_input "$1" "$2" "$3" | cksum | cut -d' ' -f1
}

# A pattern drawn into OUT, whole or not at all
draw_pattern() {
    local dir="$1" name="$2" seed="$3" tone="$4" out="$5"
    mkdir -p "${out%/*}"
    if pattern_input "${dir}" "${name}" "${seed}" "${tone}" | awk -f "$(dirname "${BASH_SOURCE[0]}")/pattern.awk" >"${out}.new"; then
        mv "${out}.new" "${out}"
    else
        rm -f "${out}.new"
        return 1
    fi
}

# The colours an interface needs beyond a palette's, as key=value lines: the surfaces, an accent and a label that can be
# read on it, status colours, the line a control is drawn with and how much of the text a dimmed label keeps, each at
# the ratio WCAG asks (palette.awk, which says which). A theme's folder holds colors.toml, and light.mode when it is light.
derive_palette() {
    local dir="$1" key
    {
        for key in background foreground accent color1 color2 color3 color4 color5 color6 success warning error; do
            echo "${key}=$(colors_get "${dir}" "${key}")"
        done
        if [[ -f "${dir}/light.mode" ]]; then
            echo "light=1"
        fi
    } | awk -f "$(dirname "${BASH_SOURCE[0]}")/palette.awk"
}

# The three checks Control's theme maker shows, for a theme's folder (palette.awk with checks=1): `NAME RATIO NEAREST`
# lines for text, label and outline
palette_checks() {
    local dir="$1" key
    {
        for key in background foreground accent color1 color2 color3 color4 color5 color6 success warning error; do
            echo "${key}=$(colors_get "${dir}" "${key}")"
        done
        [[ -f "${dir}/light.mode" ]] && echo "light=1"
        echo "checks=1"
    } | awk -f "$(dirname "${BASH_SOURCE[0]}")/palette.awk" | sed -n 's/^check_\([a-z]*\)=\(.*\)$/\1 \2/p'
}

hex_to_rgb() {
    local hex="${1#\#}"
    printf '%d,%d,%d' "0x${hex:0:2}" "0x${hex:2:2}" "0x${hex:4:2}"
}

# 0.0-1.0 components, the form GNOME extensions store colours in
hex_to_float() {
    local hex="${1#\#}"
    awk -v r="$((16#${hex:0:2}))" -v g="$((16#${hex:2:2}))" -v b="$((16#${hex:4:2}))" \
        'BEGIN { printf "%.3f, %.3f, %.3f", r/255, g/255, b/255 }'
}

# The GNOME accent whose colour is nearest the theme's, so a ported theme still tints the shell
closest_gnome_accent() {
    local hex="${1#\#}" best=purple best_distance=-1 entry name candidate distance
    [[ ${#hex} -eq 6 ]] || { echo "${best}"; return; }
    for entry in "${GNOME_ACCENTS[@]}"; do
        name="${entry%% *}"
        candidate="${entry##* }"
        candidate="${candidate#\#}"
        distance=$(awk -v a="$((16#${hex:0:2}))" -v b="$((16#${hex:2:2}))" -v c="$((16#${hex:4:2}))" \
            -v x="$((16#${candidate:0:2}))" -v y="$((16#${candidate:2:2}))" -v z="$((16#${candidate:4:2}))" \
            'BEGIN { print (a-x)^2 + (b-y)^2 + (c-z)^2 }')
        if [[ ${best_distance} -lt 0 ]] || ((distance < best_distance)); then
            best_distance=${distance}
            best="${name}"
        fi
    done
    echo "${best}"
}

# The template with each {{ key }} replaced by the value of that key in an associative array of the caller's, named by the
# first argument: the whole file, its last newline included, each value put in as text, the replacement quoted so that
# bash 5.2 reads no & in it
fill_template() {
    local -n table="$1"
    local text key
    # A theme switch fills some 60 templates, and this takes nearly all of its time: only the keys a template names, and
    # byte by byte, since bash's replacement is many times slower in a UTF-8 locale
    local LC_ALL=C
    IFS= read -r -d '' text <"$2" || true
    while IFS= read -r key; do
        key="${key:3:-3}"
        [[ -v table[${key}] ]] && text="${text//"{{ ${key} }}"/"${table[${key}]}"}"
    done < <(grep -oE '\{\{ [A-Za-z0-9_]+ \}\}' "$2" | sort -u)
    printf '%s' "${text}"
}

# Render every *.tpl into dest, substituting {{ key }}, {{ key_strip }} (no leading #) and
# {{ key_rgb }} (decimal r,g,b) from colors.toml. A file the theme ships itself is left alone, so a
# theme can hand-write any config a template would otherwise generate. Any further key=value pairs
# given as arguments are substituted too, for things a palette cannot hold, such as the theme name.
# A theme is shared as dotfiles and looks like data, so nothing in it is ever read as a program: each
# colour has to be a colour, and is put into the templates as text. (It used to become a sed command, and a
# value holding | could end it and run anything, at every login.) A value that is not #rrggbb refuses the
# whole theme, with its name, rather than render half of it.
render_templates() {
    local dest="$1" key value template output text style
    local -A values=()
    shift

    [[ -f "${dest}/colors.toml" ]] || return 0

    # The caller's own values, such as the theme's name, which it has checked
    for value in "$@"; do
        values["${value%%=*}"]="${value#*=}"
    done

    while IFS='=' read -r key value || [[ -n ${key} ]]; do
        key="${key//[\"\' ]/}"
        [[ -n ${key} && ${key} != \#* ]] || continue
        value="${value#*[\"\']}"
        value="${value%%[\"\']*}"
        if [[ ! ${key} =~ ^[A-Za-z][A-Za-z0-9_]*$ || ! ${value} =~ ^#[0-9a-fA-F]{6}$ ]]; then
            echo "The theme in ${dest##*/} is refused: in colors.toml, ${key%%[^A-Za-z0-9_]*} is not a colour written #rrggbb." >&2
            return 1
        fi
        values["${key}"]="${value}"
        values["${key}_strip"]="${value#\#}"
        values["${key}_rgb"]="$(hex_to_rgb "${value}")"
    done <"${dest}/colors.toml"

    # A palette needs its ground and its text to be worked from
    [[ -n ${values[background]:-} && -n ${values[foreground]:-} ]] ||
        { echo "The theme in ${dest##*/} is refused: its colors.toml has no background or no foreground." >&2; return 1; }

    # What the palette does not hold but an interface needs (derive_palette), in the same three forms, and the monospace
    # font, so that no caller has to pass them
    while IFS='=' read -r key value; do
        [[ -n ${key} ]] || continue
        # Every colour an interface takes from the palette is one, or the theme is refused: an empty one would be written
        # into a stylesheet as `--accent-bg-color: ;`
        if [[ ${key} =~ (_bg|_fg|_text)$|^outline$ && ! ${value} =~ ^#[0-9a-f]{6}$ ]]; then
            echo "The theme in ${dest##*/} is refused: its palette gives no colour for ${key}." >&2
            return 1
        fi
        values["${key}"]="${value}"
        if [[ ${value} =~ ^#[0-9a-fA-F]{6}$ ]]; then
            values["${key}_strip"]="${value#\#}"
            values["${key}_rgb"]="$(hex_to_rgb "${value}")"
        fi
    done < <(derive_palette "${dest}")
    [[ -n ${values[accent_fg]:-} ]] || { echo "The theme in ${dest##*/} is refused: its palette could not be worked out." >&2; return 1; }
    # A palette too close to its own ground has colours that no lightness brings to the ratio WCAG asks: it is applied as it is,
    # and says which
    if [[ -n ${values[unmet]:-} ]]; then
        echo "The palette of ${dest##*/} cannot give every colour the contrast WCAG asks (${values[unmet]}); it is applied as it is." >&2
    fi
    values[mono_font]="$(mono_font "${dest}")"
    # The scheme the theme is drawn in, which the palette for libadwaita is limited to
    if [[ -f "${dest}/light.mode" ]]; then values[scheme]=light; else values[scheme]=dark; fi

    # A file the theme ships itself is left alone, and so is a style's, which would otherwise be given its shape twice
    local -a own_styles=()
    for style in "${STYLES[@]}"; do
        [[ -f "${dest}/style-${style}.gtk.css" ]] && own_styles+=("${style}")
    done

    shopt -s nullglob
    for template in "${USER_TEMPLATES_DIR}"/*.tpl "${SYSTEM_TEMPLATES_DIR}"/*.tpl; do
        # The shape every style is drawn in is a template of its own, filled once for each style below
        [[ ${template##*/} == style-base.gtk.css.tpl ]] && continue
        # Only the templates asked for, when the caller asks for some (an accent's set: the shell's and the apps')
        [[ -z ${RENDER_ONLY:-} || ${template##*/} =~ ${RENDER_ONLY} ]] || continue
        output="${dest}/$(basename "${template}" .tpl)"
        [[ -f ${output} ]] && continue
        fill_template values "${template}" >"${output}"
    done
    shopt -u nullglob
    [[ -z ${RENDER_ONLY:-} ]] || return 0

    # Each style's file in the applications: the shape, filled with the style's radii, and then the style's own (lines,
    # glow and colours), which the loop above has rendered
    local base="" shaped_text control token
    for template in "${USER_TEMPLATES_DIR}/style-base.gtk.css.tpl" "${SYSTEM_TEMPLATES_DIR}/style-base.gtk.css.tpl"; do
        if [[ -f ${template} ]]; then
            base="${template}"
            break
        fi
    done
    for style in "${STYLES[@]}"; do
        [[ -n ${base} && -n ${STYLE_SHAPE[${style}]:-} && " ${own_styles[*]} " != *" ${style} "* ]] || continue
        local -A shaped=()
        for key in "${!values[@]}"; do
            shaped["${key}"]="${values[${key}]}"
        done
        shaped[style_name]="${style}"
        for token in ${STYLE_SHAPE[${style}]}; do
            shaped["s_${token%%=*}"]="${token#*=}"
        done
        # The inner corners of a toggle group are the group's less its padding
        control="${shaped[s_control]}"
        shaped[s_toggle]="$((control > 3 ? control - 3 : 0))"
        output="${dest}/style-${style}.gtk.css"
        shaped_text="$(fill_template shaped "${base}")"
        { printf '%s\n' "${shaped_text}"; cat "${output}" 2>/dev/null || true; } >"${output}.new"
        mv "${output}.new" "${output}"
        unset shaped
    done

    # Every style's radii, one `style.token = px` line each, for the apps that draw the style in their own stylesheets
    # (apps/common/src/palette.rs); a theme may ship its own
    if [[ ! -f "${dest}/shapes.conf" ]]; then
        for style in "${STYLES[@]}"; do
            for token in ${STYLE_SHAPE[${style}]:-}; do
                echo "${style}.${token%%=*} = ${token#*=}"
            done
        done >"${dest}/shapes.conf"
    fi
}

# Settings are written with dconf rather than gsettings: it needs no schema, so it also reaches the
# relocatable ones (Ptyxis profiles) and extensions that keep their schema out of the system path.
#
# The value comes from a theme's files, so it is escaped for the GVariant string dconf parses: a quote in a
# theme's gtk.theme cannot end the string and write something else.
dconf_write_string() {
    local value="${2//\\/\\\\}"
    value="${value//\'/\\\'}"
    dconf write "$1" "'${value}'"
}

have() {
    command -v "$1" >/dev/null 2>&1
}

notify() {
    have notify-send && notify-send --app-name=Amethystora --expire-time=2000 "$@" || true
}
