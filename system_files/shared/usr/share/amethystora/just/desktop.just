# vim: set ft=make :

set default-list

# Pick the desktop theme: colours, terminal palette, prompt and wallpaper (Super+Ctrl+Shift+Space); or save NAME (a
# palette on stdin, from Control's theme maker), import FILE (a kitty, Alacritty, base16, base24 or Xresources colour
# file), share [THEME] [FILE] (the look as one .amethystora-look file) and look FILE (open one, while ame desktop
# look-files is on)
[positional-arguments]
theme *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    helper=/usr/libexec/amethystora-theme-file
    # A palette's colours as blocks of themselves, for a terminal: the ground, the text, the accent, then the sixteen
    swatches() {
        jq -r '.colors | [.background, .foreground, .accent, .color0, .color1, .color2, .color3, .color4, .color5, .color6,
            .color7, .color8, .color9, .color10, .color11, .color12, .color13, .color14, .color15][]' <<<"$1" |
            while read -r hex; do
                printf '\e[48;2;%d;%d;%dm   \e[0m ' "0x${hex:1:2}" "0x${hex:3:2}" "0x${hex:5:2}"
            done
        echo
    }
    # The three checks of Control's theme maker for a palette in JSON, each its ratio and what it needs
    checks() {
        local dir name ratio nearest
        dir="$(mktemp -d)"
        jq -r '.colors | to_entries[] | "\(.key) = \"\(.value)\""' <<<"$1" >"${dir}/colors.toml"
        if [[ "$(jq -r .light <<<"$1")" == true ]]; then : >"${dir}/light.mode"; fi
        while read -r name ratio nearest; do
            case "${name}" in
                text) name="$(gettext "Text on the surfaces")" need=4.5 ;;
                label) name="$(gettext "A label on the accent")" need=4.5 ;;
                outline) name="$(gettext "Outlines")" need=3 ;;
                *) continue ;;
            esac
            if awk -v r="${ratio}" -v n="${need}" 'BEGIN { exit !(r >= n) }'; then
                echo "  ${name}: ${ratio}:1, $(gettext "passes")"
            else
                echo "  ${name}: ${ratio}:1, $(eval_gettext "needs \${need}:1: the theme draws it as \${nearest}")"
            fi
        done < <(palette_checks "${dir}")
        rm -rf "${dir}"
    }
    # A palette from Control's theme maker that fails a check is not kept: the reason, and a failure
    refuses() {
        local dir name ratio nearest need good=0
        dir="$(mktemp -d)"
        printf '%s\n' "$1" >"${dir}/colors.toml"
        if "${helper}" light <<<"$1" 2>/dev/null; then : >"${dir}/light.mode"; fi
        while read -r name ratio nearest; do
            case "${name}" in
                text) name="$(gettext "Text on the surfaces")" need=4.5 ;;
                label) name="$(gettext "A label on the accent")" need=4.5 ;;
                outline) name="$(gettext "Outlines")" need=3 ;;
                *) continue ;;
            esac
            if ! awk -v r="${ratio}" -v n="${need}" 'BEGIN { exit !(r >= n) }'; then
                echo "$(eval_gettext "Not saved, too little contrast. \${name}: \${ratio}:1, needs \${need}:1. \${nearest} would pass.")" >&2
                good=1
            fi
        done < <(palette_checks "${dir}")
        rm -rf "${dir}"
        return "${good}"
    }
    # A name of the account's own for a theme: one a shipped theme has gets a number
    own_name() {
        local name="$1" n=2
        while [[ -d "${SYSTEM_THEMES_DIR}/$(theme_slug "${name}")" ]]; do
            name="${1} ${n}"
            n=$((n + 1))
        done
        echo "${name}"
    }
    # Asked first when the account has a theme of that name already
    replace_ok() {
        local slug shown="$1"
        slug="$(theme_slug "$1")"
        [[ -d "${USER_THEMES_DIR}/${slug}" ]] || return 0
        gum confirm --default=false "$(eval_gettext "You have a theme called \${shown}. Replace it?")"
    }
    case "${1:-}" in
        save)
            # The palette Control's theme maker made, on stdin; the helper takes it only whole
            [[ $# == 2 ]] || { echo "usage: ame desktop theme save NAME"; exit 64; }
            palette="$(cat)"
            refuses "${palette}" || exit 3
            slug="$("${helper}" save "$2" <<<"${palette}")"
            AMETHYSTORA_THEME_QUIET=1 amethystora-theme set "${slug}" >/dev/null
            name="$2"
            echo "$(eval_gettext "Theme saved as \${name}, and applied.")"
            ;;
        import)
            [[ $# == 2 || $# == 3 ]] || { echo "usage: ame desktop theme import FILE [NAME]"; exit 64; }
            info="$("${helper}" read "$2")" || exit 3
            name="$(own_name "${3:-$(jq -r .name <<<"${info}")}")"
            file="${2##*/}"
            kind="$(jq -r .format <<<"${info}")"
            read_lines="$(jq -r .read <<<"${info}")"
            left_lines="$(jq -r .left_out <<<"${info}")"
            echo "$(eval_gettext "\${file}: a \${kind} colour file")"
            swatches "${info}"
            echo "$(eval_gettext "Colour lines read: \${read_lines}. Other lines left out: \${left_lines}.")"
            picture="$(jq -r '.picture // empty' <<<"${info}")"
            if [[ -n "${picture}" ]]; then
                picture="${picture##*/}"
                echo "$(eval_gettext "The picture beside it becomes its wallpaper: \${picture}")"
            fi
            checks "${info}"
            gum confirm "$(eval_gettext "Import it as the theme \${name}?")" || exit 0
            replace_ok "${name}" || exit 0
            slug="$("${helper}" import "$2" "${name}")"
            AMETHYSTORA_THEME_QUIET=1 amethystora-theme set "${slug}" >/dev/null
            echo "$(eval_gettext "Imported as \${name}, and applied.")"
            ;;
        share)
            slug="${2:-$(theme_current 2>/dev/null || true)}"
            slug="$(theme_slug "${slug}")"
            theme_exists "${slug}" || { echo "$(gettext "No such theme. Try: amethystora-theme list")" >&2; exit 64; }
            out="${3:-${HOME}/${slug}.amethystora-look}"
            [[ "${out}" == *.amethystora-look ]] || out="${out}.amethystora-look"
            dir="$(theme_source_dir "${slug}")"
            args=(--theme "${dir}" --name "${slug//-/ }")
            # The look as this account has it when the theme shared is the current one; else the theme's own
            if [[ "${slug}" == "$(theme_current 2>/dev/null || true)" ]]; then
                picture="$(readlink "${CURRENT_BACKGROUND}" 2>/dev/null || true)"
                if [[ -f "${picture}" ]]; then args+=(--picture "${picture}"); fi
                args+=(--style "$(desktop_style)" --icons "$(icon_theme)" --font "$(ui_font)" --mono-font "$(mono_font)"
                    --pointer "$(pointer_theme)")
                accent="$(account_accent)"
                if [[ -n "${accent}" ]]; then args+=(--accent "${accent}"); fi
            fi
            if [[ -e "${out}" ]]; then
                gum confirm --default=false "$(eval_gettext "\${out} is there already. Write over it?")" || exit 0
            fi
            "${helper}" share "${out}" "${args[@]}" >/dev/null
            echo "$(eval_gettext "The look is in \${out}: it holds the colours, the wallpaper and the names of the style, icons, fonts and pointer, and nothing that runs.")"
            ;;
        look)
            [[ $# == 2 ]] || { echo "usage: ame desktop theme look FILE"; exit 64; }
            if [[ "$(head -n1 "${CONFIG_DIR}/look-files" 2>/dev/null || true)" != on ]]; then
                echo "$(gettext "Opening looks from files is off, since a file from a stranger is still a file from a stranger.")"
                echo "$(gettext "To turn it on: ame desktop look-files on, or Control, under Appearance.")"
                exit 0
            fi
            info="$("${helper}" look "$2")" || exit 3
            name="$(own_name "$(jq -r .name <<<"${info}")")"
            echo "$(eval_gettext "The look \${name}")"
            swatches "${info}"
            if [[ "$(jq -r .picture <<<"${info}")" == true ]]; then echo "  $(gettext "Wallpaper"): $(gettext "1 picture")"; fi
            # Each name the look gives, kept only when this computer has it
            declare -A keep=()
            while IFS=$'\t' read -r key value; do
                case "${key}" in
                    style) is_style "${value}" && keep[style]="${value}" ;;
                    icons) icon_theme_installed "${value}" && keep[icons]="${value}" ;;
                    font) font_installed "${value}" && keep[font]="${value}" ;;
                    mono-font) font_installed "${value}" && keep[mono-font]="${value}" ;;
                    pointer) cursor_theme_installed "${value}" && keep[pointer]="${value}" ;;
                    accent) is_accent "${value}" && keep[accent]="${value}" ;;
                    *) continue ;;
                esac
                if [[ -n "${keep[${key}]:-}" ]]; then
                    echo "  ${key}: ${value}"
                else
                    echo "  ${key}: ${value} ($(gettext "this computer does not have it: left out"))"
                fi
            done < <(jq -r '.look | to_entries[] | select(.key != "name") | "\(.key)\t\(.value)"' <<<"${info}")
            checks "${info}"
            gum confirm "$(eval_gettext "Apply the look \${name}?")" || exit 0
            replace_ok "${name}" || exit 0
            slug="$("${helper}" look-import "$2" "${name}" | jq -r .slug)"
            mkdir -p "${CONFIG_DIR}"
            for key in style icons font mono-font pointer accent; do
                if [[ -n "${keep[${key}]:-}" ]]; then echo "${keep[${key}]}" >"${CONFIG_DIR}/${key}"; fi
            done
            AMETHYSTORA_THEME_QUIET=1 amethystora-theme set "${slug}" >/dev/null
            echo "$(eval_gettext "The look \${name} is applied.")"
            ;;
        "")
            ;;
        *)
            amethystora-theme set "$1"
            exit 0
            ;;
    esac
    [[ -z "${1:-}" ]] || exit 0
    current="$(amethystora-theme current 2>/dev/null || true)"
    shown="${current:-$(gettext "none")}"
    choice="$(amethystora-theme list | sed 's/^[*] /  /' | awk '{ print $1 }' |
        gum choose --header "$(eval_gettext "Theme (current: \${shown})")" --height 14 --selected "${current}")" || exit 0
    amethystora-theme set "${choice}"

# Cycle the wallpaper of the current theme (Super+Ctrl+Space), or show IMAGE, one that amethystora-theme bg list names
[positional-arguments]
background IMAGE="":
    #!/usr/bin/bash
    if [[ -n "${1:-}" ]]; then
        exec amethystora-theme bg set "$1"
    fi
    exec amethystora-theme bg next

# Pick the accent of buttons, switches and selections in any theme: one of twelve, or theme for the theme's own
[positional-arguments]
accent ACCENT="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    declare -A names=(
        [amethyst]="$(gettext "Amethyst")" [iris]="$(gettext "Iris")" [sky]="$(gettext "Sky")" [lagoon]="$(gettext "Lagoon")"
        [jade]="$(gettext "Jade")" [moss]="$(gettext "Moss")" [amber]="$(gettext "Amber")" [ember]="$(gettext "Ember")"
        [coral]="$(gettext "Coral")" [rose]="$(gettext "Rose")" [orchid]="$(gettext "Orchid")" [slate]="$(gettext "Slate")"
        [theme]="$(gettext "Follow the theme")"
    )
    accent="${1:-}"
    if [[ -z "${accent}" ]]; then
        current="$(account_accent)"
        for entry in "${ACCENTS[@]}" "theme"; do
            id="${entry%% *}"
            printf '%-9s %s\n' "${id}" "${names[${id}]}"
        done
        accent="$(gum choose --header "$(gettext "Accent")" --selected "${current:-theme}" \
            amethyst iris sky lagoon jade moss amber ember coral rose orchid slate theme)" || exit 0
    fi
    if [[ "${accent}" == theme && ! -e "${IMAGE_SETUP_DIR}/accent" ]]; then
        rm -f "${CONFIG_DIR}/accent"
    elif [[ "${accent}" == theme ]] || is_accent "${accent}"; then
        # theme is written down when the image has an accent of its own, which it would otherwise give back
        mkdir -p "${CONFIG_DIR}"
        echo "${accent}" >"${CONFIG_DIR}/accent"
    else
        echo "usage: ame desktop accent amethyst|iris|sky|lagoon|jade|moss|amber|ember|coral|rose|orchid|slate|theme"
        exit 64
    fi
    # The theme again, with its transition, for the applications too; the wallpaper stays
    AMETHYSTORA_THEME_QUIET=1 amethystora-theme reload >/dev/null
    name="${names[${accent}]}"
    echo "$(eval_gettext "Accent: \${name}. Applications that are open take it when they next start.")"

# Turn Daylight on or off: the theme's first wallpaper by the time of day, dawn, day, dusk and night (without STATE, it flips)
[positional-arguments]
daylight STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    status="$(amethystora-theme daylight status 2>/dev/null || echo none)"
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "${status}" == off ]]; then state=on; else state=off; fi
    fi
    case "${state}" in
        on | off) ;;
        *)
            echo "usage: ame desktop daylight on|off"
            exit 64
            ;;
    esac
    if [[ "${status}" == none ]]; then
        # Kept for the next theme that has the four pictures
        if [[ "${state}" == off ]]; then
            mkdir -p "${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora"
            echo off >"${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/daylight"
        else
            rm -f "${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/daylight"
        fi
        echo "$(gettext "This theme has no Daylight pictures.")"
        exit 0
    fi
    amethystora-theme daylight "${state}"
    if [[ "${state}" == on ]]; then
        echo "$(gettext "Daylight wallpapers: on.")"
    else
        echo "$(gettext "Daylight wallpapers: off.")"
    fi

# A wallpaper drawn from the theme's colours: facets, waves, contour, mesh, grain, dots or bands; shuffle for another;
# off for the theme's own pictures again
[positional-arguments]
pattern PATTERN="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    pattern="${1:-}"
    if [[ -z "${pattern}" ]]; then
        read -r current _ <<<"$(account_pattern)"
        pattern="$(gum choose --header "$(gettext "A wallpaper drawn from the theme")" --selected "${current:-off}" \
            "${PATTERNS[@]}" shuffle off)" || exit 0
    fi
    if [[ "${pattern}" != shuffle && "${pattern}" != off ]] && ! is_pattern "${pattern}"; then
        echo "usage: ame desktop pattern $(IFS='|'; echo "${PATTERNS[*]}")|shuffle|off"
        exit 64
    fi
    amethystora-theme pattern "${pattern}"
    if [[ "${pattern}" == off ]]; then
        echo "$(gettext "The theme's own pictures again.")"
    else
        read -r drawn _ <<<"$(account_pattern)"
        echo "$(eval_gettext "Wallpaper drawn from the theme: \${drawn}.")"
    fi

# The theme's light side by day and its dark side by night, for a theme with a pair (Amethystora and Amethystora Light,
# Catppuccin Mocha and Latte): on (Night Light's times), off, or hours FROM TO (each 0 to 23.75, as Night Light writes them)
[positional-arguments]
day-night STATE="" FROM="" TO="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.theme
    usage() { echo "usage: ame desktop day-night on|off|night-light|hours FROM TO"; exit 64; }
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" day-night)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on | night-light)
            gsettings set "${schema}" day-night-times night-light
            gsettings set "${schema}" day-night true
            ;;
        off) gsettings set "${schema}" day-night false ;;
        hours)
            [[ $# == 3 ]] || usage
            for value in "$2" "$3"; do
                [[ "${value}" =~ ^([0-9]|1[0-9]|2[0-3])(\.[0-9]{1,2})?$ ]] || usage
            done
            awk -v a="$2" -v b="$3" 'BEGIN { exit !(a + 0 != b + 0) }' || usage
            gsettings set "${schema}" day-starts "$2"
            gsettings set "${schema}" night-starts "$3"
            gsettings set "${schema}" day-night-times hours
            gsettings set "${schema}" day-night true
            ;;
        *) usage ;;
    esac
    if [[ "${state}" == off ]]; then
        echo "$(gettext "Light by day, dark by night: off.")"
        exit 0
    fi
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    if [[ -z "$(theme_file pair.theme)" ]]; then
        echo "$(gettext "Light by day, dark by night: on. This theme has no light and dark pair, so it stays as it is: Amethystora and Amethystora Light, and Catppuccin Mocha and Latte, have one.")"
    else
        echo "$(gettext "Light by day, dark by night: on.")"
    fi

# Open looks from .amethystora-look files (ame desktop theme look FILE, and Open With): on or off (without STATE, it flips)
[positional-arguments]
look-files STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    flag="${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/look-files"
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(head -n1 "${flag}" 2>/dev/null || true)" == on ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            mkdir -p "${flag%/*}"
            echo on >"${flag}"
            echo "$(gettext "Looks from files: on. Opening one shows what it holds and asks before anything changes.")"
            ;;
        off)
            rm -f "${flag}"
            echo "$(gettext "Looks from files: off.")"
            ;;
        *)
            echo "usage: ame desktop look-files on|off"
            exit 64
            ;;
    esac

# Take the desktop's colours from the wallpaper: vivid, soft (on), mono or off
[positional-arguments]
colours WAY="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    way="${1:-}"
    if [[ -z "${way}" ]]; then
        current="$(colours_way)"
        echo "vivid  $(gettext "The strongest hue leads")"
        echo "soft   $(gettext "The same hues, quieter")"
        echo "mono   $(gettext "Tinted greys, colour on controls only")"
        echo "off    $(gettext "The theme's own colours")"
        way="$(gum choose --header "$(gettext "Colours from the wallpaper")" --selected "${current:-soft}" vivid soft mono off)" || exit 0
    fi
    [[ "${way}" == on ]] && way=soft
    if [[ "${way}" == off && ! -e "${IMAGE_SETUP_DIR}/colours" ]]; then
        rm -f "${CONFIG_DIR}/colours"
    elif [[ "${way}" == off ]] || is_colour_way "${way}"; then
        # off is written down when the image takes colours from the wallpaper itself, which it would otherwise give back
        mkdir -p "${CONFIG_DIR}"
        echo "${way}" >"${CONFIG_DIR}/colours"
    else
        echo "usage: ame desktop colours vivid|soft|mono|on|off"
        exit 64
    fi
    code=0
    amethystora-theme colours refresh || code=$?
    if [[ "${way}" == off ]]; then
        echo "$(gettext "Colours from the wallpaper: off.")"
    elif [[ "${code}" == 3 ]]; then
        echo "$(gettext "Colours from the wallpaper need a picture: this wallpaper is not one.")"
    else
        echo "$(eval_gettext "Colours from the wallpaper: \${way}.")"
    fi

# An accent for each workspace, in the shell and Amethystora's apps (ACTION: on or off, or a workspace's number 1 to 6 and
# its ACCENT; without ACTION, it flips)
[positional-arguments]
accent-per-workspace ACTION="" ACCENT="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    schema=org.gnome.shell.extensions.amethystora-ui.theme
    usage="usage: ame desktop accent-per-workspace on|off|1-6 amethyst|iris|sky|lagoon|jade|moss|amber|ember|coral|rose|orchid|slate|theme"
    action="${1:-}"
    if [[ -z "${action}" ]]; then
        if [[ "$(gsettings get "${schema}" accent-per-workspace)" == true ]]; then action=off; else action=on; fi
    fi
    case "${action}" in
        on)
            # The accents' rules first, so that the shell never looks for a set that is not there
            AMETHYSTORA_ACCENTS=on amethystora-theme accents
            gsettings set "${schema}" accent-per-workspace true
            echo "$(gettext "Accent per workspace: on.")"
            ;;
        off)
            gsettings set "${schema}" accent-per-workspace false
            amethystora-theme accents
            rm -f "${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/workspace-accent"{,.bak}
            echo "$(gettext "Accent per workspace: off.")"
            ;;
        [1-6])
            accent="${2:-}"
            if [[ "${accent}" != theme ]] && ! is_accent "${accent}"; then
                echo "${usage}"
                exit 64
            fi
            mapfile -t list < <(gsettings get "${schema}" workspace-accents | grep -oE "'[^']*'" | tr -d "'")
            # Anything but an id in the list is the theme's own, as the shell reads it
            for i in "${!list[@]}"; do
                if [[ "${list[i]}" != theme ]] && ! is_accent "${list[i]}"; then list[i]=theme; fi
            done
            while ((${#list[@]} < action)); do
                list+=(theme)
            done
            list[action - 1]="${accent}"
            gsettings set "${schema}" workspace-accents "[$(printf "'%s', " "${list[@]}" | sed 's/, $//')]"
            amethystora-theme accents
            n="${action}"
            declare -A names=(
                [amethyst]="$(gettext "Amethyst")" [iris]="$(gettext "Iris")" [sky]="$(gettext "Sky")" [lagoon]="$(gettext "Lagoon")"
                [jade]="$(gettext "Jade")" [moss]="$(gettext "Moss")" [amber]="$(gettext "Amber")" [ember]="$(gettext "Ember")"
                [coral]="$(gettext "Coral")" [rose]="$(gettext "Rose")" [orchid]="$(gettext "Orchid")" [slate]="$(gettext "Slate")"
                [theme]="$(gettext "Follow the theme")"
            )
            name="${names[${accent}]}"
            echo "$(eval_gettext "Workspace \${n}: \${name}.")"
            ;;
        *)
            echo "${usage}"
            exit 64
            ;;
    esac

# Pick how a new theme or wallpaper is revealed: grow, outer, wipe, wave, fade, random or none (DURATION: how long, 200 to 5000 milliseconds); or reveal on|off, the splash's gem flying into the top bar at login (without on or off, it flips)
[positional-arguments]
transition STYLE="" DURATION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-transitions
    style="${1:-}"
    duration="${2:-}"
    # The reveal at login is a switch of its own, not a style
    if [[ "${style}" == reveal ]]; then
        state="${duration}"
        if [[ -z "${state}" ]]; then
            if [[ "$(gsettings get "${schema}" reveal)" == true ]]; then state=off; else state=on; fi
        fi
        case "${state}" in
            on)
                gsettings set "${schema}" reveal true
                echo "$(gettext "The desktop is revealed at login.")"
                ;;
            off)
                gsettings set "${schema}" reveal false
                echo "$(gettext "No reveal at login.")"
                ;;
            *)
                echo "usage: ame desktop transition reveal on|off"
                exit 64
                ;;
        esac
        exit 0
    fi
    if [[ -n "${duration}" && ! "${duration}" =~ ^[0-9]+$ ]]; then
        echo "usage: ame desktop transition STYLE [DURATION in milliseconds]"
        exit 64
    fi
    if [[ -z "${style}" ]]; then
        current="$(gsettings get "${schema}" style | tr -d "'")"
        style="$(gum choose --header "$(eval_gettext "Transition (current: \${current})")" --selected "${current}" \
            grow outer wipe wave fade random none)" || exit 0
    fi
    gsettings set "${schema}" style "${style}"
    # gsettings refuses a length outside the schema's range
    if [[ -n "${duration}" ]]; then
        gsettings set "${schema}" duration "${duration}"
    fi
    length="$(gsettings get "${schema}" duration | sed 's/^uint32 //')"
    echo "$(eval_gettext "Transition set to \${style}, taking \${length} ms.")"
    echo "$(gettext "Super+Ctrl+Space changes the wallpaper to show it.")"

# Pick the icon pack (glass, glass-light, neon, neon-light, duotone, line, clay or muted), or theme to follow the theme's own
[positional-arguments]
icons PACK="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    # Read by amethystora-theme, which writes the icon theme at every switch and would otherwise put the
    # theme's own back
    flag="${CONFIG_DIR}/icons"
    current="$(sed -e 's/^amethystora-//' "${flag}" 2>/dev/null || echo theme)"
    pack="${1:-}"
    if [[ -z "${pack}" ]]; then
        echo "glass        $(gettext "Each icon in its own gradient, with a glass sheen")"
        echo "glass-light  $(gettext "Glass with its colours deepened to read on a light desktop")"
        echo "neon         $(gettext "Outlines lit in colours of their own, for a dark desktop")"
        echo "neon-light   $(gettext "Neon with its colours deepened to read on a light desktop")"
        echo "duotone      $(gettext "Each icon in a gradient, its details in a contrasting one")"
        echo "line         $(gettext "One even outline, in colours that read on every theme")"
        echo "clay         $(gettext "Soft matte volumes, a highlight above and a shade below")"
        echo "muted        $(gettext "Glass in quieter colours, for themes of low chroma")"
        echo "theme        $(gettext "Whatever the theme chooses")"
        pack="$(gum choose --header "$(eval_gettext "Icons (current: \${current})")" --selected "${current}" "${ICON_PACKS[@]}" theme)" || exit 0
    fi
    if [[ "${pack}" == theme ]]; then
        rm -f "${flag}"
        # What the theme chooses, written on its own: applying the whole theme again would hold the screen for a
        # transition and put another wallpaper up
        gsettings set org.gnome.desktop.interface icon-theme "$(icon_theme)"
        echo "$(eval_gettext "Icons: \${pack}.")"
        exit 0
    elif ! is_icon_pack "${pack}"; then
        echo "usage: ame desktop icons $(IFS='|'; echo "${ICON_PACKS[*]}")|theme"
        exit 64
    fi
    mkdir -p "${CONFIG_DIR}"
    echo "amethystora-${pack}" >"${flag}"
    gsettings set org.gnome.desktop.interface icon-theme "amethystora-${pack}"
    echo "$(eval_gettext "Icons: \${pack}.")"

# Pick the desktop style: the shape of the top bar, menus, notifications, dialogs, buttons and windows
[positional-arguments]
style STYLE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    # Read by amethystora-theme, which writes the style at every switch and would otherwise put the theme's own back
    flag="${CONFIG_DIR}/style"
    current="$(head -n1 "${flag}" 2>/dev/null || echo theme)"
    style="${1:-}"
    if [[ -z "${style}" ]]; then
        echo "islands   $(gettext "A floating pill for each part of the top bar, soft corners everywhere")"
        echo "floating  $(gettext "One rounded top bar held off the edge of the screen")"
        echo "hairline  $(gettext "A thin, solid top bar, crisp corners and fine lines")"
        echo "capsule   $(gettext "The clock in a dark capsule, and everything else a pill")"
        echo "terminal  $(gettext "Monospace, square and lined, like a terminal")"
        echo "segments  $(gettext "The top bar in blocks of the theme's colours")"
        echo "neon      $(gettext "Dark glass outlined and glowing in the theme's accent")"
        echo "theme     $(gettext "Whatever the theme chooses")"
        style="$(gum choose --header "$(eval_gettext "Style (current: \${current})")" --selected "${current}" \
            islands floating hairline capsule terminal segments neon theme)" || exit 0
    fi
    if [[ "${style}" == theme ]]; then
        rm -f "${flag}"
    elif is_style "${style}"; then
        mkdir -p "${CONFIG_DIR}"
        echo "${style}" >"${flag}"
    else
        echo "usage: ame desktop style islands|floating|hairline|capsule|terminal|segments|neon|theme"
        exit 64
    fi
    # The whole theme again, with its transition, for the applications' half of the style too; the wallpaper stays
    AMETHYSTORA_THEME_QUIET=1 amethystora-theme reload >/dev/null
    style="$(desktop_style)"
    echo "$(eval_gettext "Style: \${style}. Applications that are open take it when they next start.")"

# Pick the glass the top bar, the dock, menus and notifications stand on: frost, prism or clear
[positional-arguments]
glass MATERIAL="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    SCHEMA=org.gnome.shell.extensions.amethystora-ui.glass
    current="$(gsettings get "${SCHEMA}" material | tr -d "'")"
    material="${1:-}"
    if [[ -z "${material}" ]]; then
        echo "frost  $(gettext "Thick, milky glass with an even tint")"
        echo "prism  $(gettext "Less tint, more colour, a lit top edge and light along the bottom")"
        echo "clear  $(gettext "The thinnest glass and the brightest rim; it darkens by itself where text needs it")"
        material="$(gum choose --header "$(eval_gettext "Glass (current: \${current})")" --selected "${current}" \
            frost prism clear)" || exit 0
    fi
    case "${material}" in
        frost | prism | clear) gsettings set "${SCHEMA}" material "${material}" ;;
        *)
            echo "usage: ame desktop glass frost|prism|clear"
            exit 64
            ;;
    esac
    case "${material}" in
        frost) echo "$(gettext "Frost: thick, milky glass with an even tint.")" ;;
        prism) echo "$(gettext "Prism: less tint, more colour, and light along the edges.")" ;;
        clear) echo "$(gettext "Clear: the thinnest glass. It darkens by itself where text needs it.")" ;;
    esac
    profile="$(busctl --system get-property org.freedesktop.UPower.PowerProfiles /org/freedesktop/UPower/PowerProfiles \
        org.freedesktop.UPower.PowerProfiles ActiveProfile 2>/dev/null || true)"
    if [[ "$(gsettings get "${SCHEMA}" solid)" == true ]] ||
        [[ "$(gsettings get org.gnome.desktop.a11y.interface high-contrast)" == true ]] ||
        [[ "${profile}" == 's "power-saver"' ]]; then
        echo "$(gettext "The glass is drawn solid while Reduce transparency, High Contrast or the power saver is on.")"
    fi

# How deep a glass's blur is and how dense its tint (R6-28): 50 to 150 per cent of the glass's own, for the glass in
# use or the one named (ame desktop glass-strength blur|tint PERCENT|reset [frost|prism|clear], or reset for both); without anything it
# shows them all. The text on the glass keeps 4.5:1 at every strength.
[positional-arguments]
glass-strength *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    SCHEMA=org.gnome.shell.extensions.amethystora-ui.glass
    usage() { echo "usage: ame desktop glass-strength blur|tint 50..150|reset [frost|prism|clear], or reset" >&2; exit 64; }
    percent() { awk -v a="$(gsettings get "${SCHEMA}" "$1")" 'BEGIN { printf "%.0f", a * 100 }'; }
    if [[ $# -eq 0 ]]; then
        for material in frost prism clear; do
            echo "${material} blur $(percent "blur-${material}") tint $(percent "tint-${material}")"
        done
        exit 0
    fi
    if [[ $# -eq 1 && "$1" == reset ]]; then
        material="$(gsettings get "${SCHEMA}" material | tr -d "'")"
        gsettings reset "${SCHEMA}" "blur-${material}"
        gsettings reset "${SCHEMA}" "tint-${material}"
        echo "$(eval_gettext "The \${material} glass is back to its own blur and tint.")"
        exit 0
    fi
    kind="${1:-}"
    value="${2:-}"
    material="${3:-$(gsettings get "${SCHEMA}" material | tr -d "'")}"
    [[ $# -le 3 && ( "${kind}" == blur || "${kind}" == tint ) && -n "${value}" ]] || usage
    case "${material}" in frost | prism | clear) ;; *) usage ;; esac
    if [[ "${value}" == reset ]]; then
        gsettings reset "${SCHEMA}" "${kind}-${material}"
    else
        [[ "${value}" =~ ^[0-9]{1,3}$ ]] && ((10#${value} >= 50 && 10#${value} <= 150)) || usage
        gsettings set "${SCHEMA}" "${kind}-${material}" "$(awk -v v="${value}" 'BEGIN { printf "%.2f", v / 100 }')"
    fi
    shown="$(percent "${kind}-${material}")"
    if [[ "${kind}" == blur ]]; then
        echo "$(eval_gettext "The \${material} glass's blur is \${shown}% of its own.")"
    else
        echo "$(eval_gettext "The \${material} glass's tint is \${shown}% of its own.")"
    fi

# Comfort: reduce transparency, reduce motion and increase contrast (WHAT: one of those, then on or off; without it, it flips)
[positional-arguments]
comfort *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    GLASS=org.gnome.shell.extensions.amethystora-ui.glass
    A11Y=org.gnome.desktop.a11y.interface
    usage() {
        echo "usage: ame desktop comfort reduce-transparency|reduce-motion|increase-contrast [on|off]"
        exit 64
    }
    # on or off for a word that may be missing, which flips the current state
    flip() { if [[ -n "$1" ]]; then echo "$1"; elif [[ "$2" == true ]]; then echo off; else echo on; fi; }
    # The words a person types back, so left as they are
    word() { if [[ "$1" == true ]]; then echo on; else echo off; fi; }
    solid() { gsettings get "${GLASS}" solid; }
    motion() { [[ "$(gsettings get "${A11Y}" reduced-motion)" == "'reduce'" ]] && echo true || echo false; }
    contrast() { gsettings get "${A11Y}" high-contrast; }
    # The power saver is the profile power-profiles-daemon's interface says is active (tuned-ppd answers it here);
    # one that cannot be read is off
    saver() {
        [[ "$(busctl --system get-property org.freedesktop.UPower.PowerProfiles /org/freedesktop/UPower/PowerProfiles \
            org.freedesktop.UPower.PowerProfiles ActiveProfile 2>/dev/null || true)" == 's "power-saver"' ]] &&
            echo true || echo false
    }
    if [[ $# -eq 0 ]]; then
        state="$(word "$(solid)")"
        echo "$(eval_gettext "Reduce transparency: \${state}")"
        state="$(word "$(motion)")"
        echo "$(eval_gettext "Reduce motion: \${state}")"
        state="$(word "$(contrast)")"
        echo "$(eval_gettext "Increase contrast: \${state}")"
        state="$(word "$(saver)")"
        echo "$(eval_gettext "Power saver: \${state} (the glass is solid and animations are shorter while it is on)")"
        exit 0
    fi
    [[ $# -le 2 ]] || usage
    case "$1" in
        reduce-transparency)
            case "$(flip "${2:-}" "$(solid)")" in
                on)
                    gsettings set "${GLASS}" solid true
                    echo "$(gettext "Reduce transparency is on: the glass is drawn in the theme's solid colours.")"
                    ;;
                off)
                    gsettings set "${GLASS}" solid false
                    echo "$(gettext "Reduce transparency is off.")"
                    ;;
                *) usage ;;
            esac
            ;;
        reduce-motion)
            case "$(flip "${2:-}" "$(motion)")" in
                on)
                    gsettings set "${A11Y}" reduced-motion reduce
                    echo "$(gettext "Reduce motion is on: animations fade in place instead of travelling.")"
                    ;;
                off)
                    gsettings set "${A11Y}" reduced-motion no-preference
                    echo "$(gettext "Reduce motion is off.")"
                    ;;
                *) usage ;;
            esac
            ;;
        increase-contrast)
            case "$(flip "${2:-}" "$(contrast)")" in
                on)
                    gsettings set "${A11Y}" high-contrast true
                    echo "$(gettext "Increase contrast is on: GNOME's High Contrast, with stronger outlines and plain text, in the apps too.")"
                    ;;
                off)
                    gsettings set "${A11Y}" high-contrast false
                    echo "$(gettext "Increase contrast is off.")"
                    ;;
                *) usage ;;
            esac
            ;;
        *) usage ;;
    esac

# Pick the interface font, or go back to the theme's
[positional-arguments]
font FAMILY="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    flag="${CONFIG_DIR}/font"
    current="$(head -n1 "${flag}" 2>/dev/null || echo theme)"
    family="${1:-}"
    if [[ -z "${family}" ]]; then
        family="$(gum choose --header "$(eval_gettext "Font (current: \${current})")" --selected "${current}" \
            theme "${UI_FONTS[@]}")" || exit 0
    fi
    if [[ "${family}" == theme ]]; then
        rm -f "${flag}"
    elif font_installed "${family}"; then
        mkdir -p "${CONFIG_DIR}"
        echo "${family}" >"${flag}"
    else
        echo "$(eval_gettext "No font called \${family} is installed. The fonts offered: ame desktop font")" >&2
        echo "usage: ame desktop font FAMILY|theme"
        exit 64
    fi
    # Written on its own: the font needs no transition, and applying the whole theme again would hold the screen
    gsettings set org.gnome.desktop.interface font-name "$(ui_font) ${FONT_SIZE}"
    gsettings set org.gnome.desktop.interface document-font-name "$(ui_font) $((FONT_SIZE + 1))"
    family="$(ui_font)"
    echo "$(eval_gettext "Font: \${family}.")"

# Pick the monospace font of the terminal, code and Text Editor, or go back to the theme's
[positional-arguments]
mono-font FAMILY="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    flag="${CONFIG_DIR}/mono-font"
    current="$(head -n1 "${flag}" 2>/dev/null || echo theme)"
    family="${1:-}"
    if [[ -z "${family}" ]]; then
        family="$(gum choose --header "$(eval_gettext "Monospace font (current: \${current})")" --selected "${current}" \
            theme "${MONO_FONTS[@]}")" || exit 0
    fi
    if [[ "${family}" == theme ]]; then
        rm -f "${flag}"
    elif font_installed "${family}"; then
        mkdir -p "${CONFIG_DIR}"
        echo "${family}" >"${flag}"
    else
        echo "$(eval_gettext "No font called \${family} is installed. The fonts offered: ame desktop mono-font")" >&2
        echo "usage: ame desktop mono-font FAMILY|theme"
        exit 64
    fi
    gsettings set org.gnome.desktop.interface monospace-font-name "$(mono_font) ${MONO_FONT_SIZE}"
    # kitty's is in the rendered theme, which the theme renders again (the wallpaper stays); a window that is open
    # takes it at once
    AMETHYSTORA_THEME_QUIET=1 amethystora-theme reload >/dev/null
    family="$(mono_font)"
    echo "$(eval_gettext "Monospace font: \${family}.")"

# Pick the pointer (halo, ink or facet), or theme to follow the theme's own
[positional-arguments]
pointer SET="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    # Read by amethystora-theme, which writes the pointer at every switch and would otherwise put the theme's own back
    flag="${CONFIG_DIR}/pointer"
    current="$(sed -e 's/^amethystora-pointer-//' "${flag}" 2>/dev/null || echo theme)"
    set="${1:-}"
    if [[ -z "${set}" ]]; then
        echo "halo   $(gettext "White, with an amethyst rim: for dark themes")"
        echo "ink    $(gettext "Dark, with a white rim: for light themes")"
        echo "facet  $(gettext "Cut like the gem, in two faces of amethyst")"
        echo "theme  $(gettext "Whatever the theme chooses")"
        set="$(gum choose --header "$(eval_gettext "Pointer (current: \${current})")" --selected "${current}" \
            "${POINTER_SETS[@]}" theme)" || exit 0
    fi
    if [[ "${set}" == theme ]]; then
        rm -f "${flag}"
    elif is_pointer_set "${set}"; then
        mkdir -p "${CONFIG_DIR}"
        echo "amethystora-pointer-${set}" >"${flag}"
    else
        echo "usage: ame desktop pointer $(IFS='|'; echo "${POINTER_SETS[*]}")|theme"
        exit 64
    fi
    # Written on its own, over whatever the key held: this is the account's own choice
    gsettings set org.gnome.desktop.interface cursor-theme "$(pointer_theme)"
    echo "$(eval_gettext "Pointer: \${set}.")"

# What shaking the mouse does: grow, beacon or off
[positional-arguments]
pointer-shake MODE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pointer
    current="$(gsettings get "${schema}" shake | tr -d "'")"
    mode="${1:-}"
    if [[ -z "${mode}" ]]; then
        echo "grow    $(gettext "The pointer swells to three and a half times its size while you shake it")"
        echo "beacon  $(gettext "The pointer grows a little and sends rings out from where it is")"
        echo "off     $(gettext "Shaking the mouse does nothing")"
        mode="$(gum choose --header "$(eval_gettext "Shake to find (current: \${current})")" --selected "${current}" \
            grow beacon off)" || exit 0
    fi
    case "${mode}" in
        grow | beacon | off) gsettings set "${schema}" shake "${mode}" ;;
        *)
            echo "usage: ame desktop pointer-shake grow|beacon|off"
            exit 64
            ;;
    esac
    echo "$(eval_gettext "Shake to find: \${mode}.")"

# A pointer one size larger on a screen with small pixels shown at 100% (STATE: on or off; without it, it flips)
[positional-arguments]
pointer-dense STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pointer
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" dense)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" dense true
            echo "$(gettext "The pointer is one size larger on a dense screen. A size chosen in Settings is kept.")"
            ;;
        off)
            gsettings set "${schema}" dense false
            echo "$(gettext "The pointer keeps its size on every screen.")"
            ;;
        *)
            echo "usage: ame desktop pointer-dense on|off"
            exit 64
            ;;
    esac

# Pick how windows are arranged: tiling, side by side on a strip, or classic, floating as on Windows and macOS
[positional-arguments]
layout STYLE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # The tiling is a module of Amethystora UI with one switch, which an update leaves as the account has it
    schema=org.gnome.shell.extensions.amethystora-ui.tiling
    if [[ "$(gsettings get "${schema}" enabled)" == false ]]; then current=classic; else current=tiling; fi
    style="${1:-}"
    if [[ -z "${style}" ]]; then
        echo "tiling   $(gettext "Windows open side by side on a strip that scrolls, arranged for you")"
        echo "classic  $(gettext "Windows float and overlap, and you move and size them, as on Windows and macOS")"
        style="$(gum choose --header "$(eval_gettext "Layout (current: \${current})")" --selected "${current}" tiling classic)" || exit 0
    fi
    case "${style}" in
        classic)
            gsettings set "${schema}" enabled false
            echo "$(gettext "Classic: windows float. Drag one by its title bar, Super+Up maximises it, Super+Down restores it, and Super+Left or Super+Right puts it on that half of the screen.")"
            ;;
        tiling)
            gsettings set "${schema}" enabled true
            echo "$(gettext "Tiling: new windows open beside the one you are on, and Super+Left and Super+Right move between them. The keys are in the manual: amethystora-manual desktop#tiling-windows")"
            ;;
        *) exit 0 ;;
    esac

# How a window goes into the dock and comes back: droplet or shrink
[positional-arguments]
minimize STYLE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.motion
    current="$(gsettings get "${schema}" minimize | tr -d "'")"
    style="${1:-}"
    if [[ -z "${style}" ]]; then
        echo "droplet  $(gettext "The window gathers into a drop of glass and arcs into its icon")"
        echo "shrink   $(gettext "The window shrinks and fades into its icon, as GNOME does it")"
        style="$(gum choose --header "$(eval_gettext "Minimize (current: \${current})")" --selected "${current}" droplet shrink)" || exit 0
    fi
    case "${style}" in
        droplet | shrink) gsettings set "${schema}" minimize "${style}" ;;
        *)
            echo "usage: ame desktop minimize droplet|shrink"
            exit 64
            ;;
    esac
    case "${style}" in
        droplet) echo "$(gettext "Droplet: a window gathers into a drop of glass and arcs into its icon in the dock.")" ;;
        shrink) echo "$(gettext "Shrink: a window shrinks and fades into its icon, as GNOME does it.")" ;;
    esac

# How workspaces change: depth or slide
[positional-arguments]
workspace-switch STYLE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.motion
    current="$(gsettings get "${schema}" workspace-switch | tr -d "'")"
    style="${1:-}"
    if [[ -z "${style}" ]]; then
        echo "depth  $(gettext "The wallpaper slides a third as far as the windows, which step back a little on the way")"
        echo "slide  $(gettext "Each workspace slides in whole, wallpaper and all")"
        style="$(gum choose --header "$(eval_gettext "Changing workspace (current: \${current})")" --selected "${current}" depth slide)" || exit 0
    fi
    case "${style}" in
        depth | slide) gsettings set "${schema}" workspace-switch "${style}" ;;
        *)
            echo "usage: ame desktop workspace-switch depth|slide"
            exit 64
            ;;
    esac
    case "${style}" in
        depth) echo "$(gettext "Depth: the wallpaper slides a third as far as the windows, which step back a little on the way.")" ;;
        slide) echo "$(gettext "Slide: each workspace slides in whole, wallpaper and all.")" ;;
    esac

# Workspaces without a name of yours show their main app's name (VALUE: on or off; without it, it flips)
[positional-arguments]
space-names VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.workspaces
    value="${1:-}"
    if [[ -z "${value}" ]]; then
        if [[ "$(gsettings get "${schema}" auto-names)" == true ]]; then value=off; else value=on; fi
    fi
    case "${value}" in
        on)
            gsettings set "${schema}" auto-names true
            echo "$(gettext "A workspace you have not named shows the name of the app with most windows on it.")"
            ;;
        off)
            gsettings set "${schema}" auto-names false
            echo "$(gettext "A workspace you have not named shows its number only.")"
            ;;
        *)
            echo "usage: ame desktop space-names on|off"
            exit 64
            ;;
    esac

# Saved sets in the workspace menu: save a workspace's apps and their places, and open them again (VALUE: on or off;
# without it, it flips). Turned off, the saved sets are kept.
[positional-arguments]
sets VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.workspaces
    value="${1:-}"
    if [[ -z "${value}" ]]; then
        if [[ "$(gsettings get "${schema}" sets)" == true ]]; then value=off; else value=on; fi
    fi
    case "${value}" in
        on)
            gsettings set "${schema}" sets true
            echo "$(gettext "The workspace menu saves a workspace's apps and their places as a set, and opens a set again.")"
            ;;
        off)
            gsettings set "${schema}" sets false
            echo "$(gettext "The workspace menu has no sets. The sets you saved are kept for when they are on again.")"
            ;;
        *)
            echo "usage: ame desktop sets on|off"
            exit 64
            ;;
    esac

# Pick up where you left off: when an update restarts the computer, the open apps and their places are kept, and the
# first login after it offers to open them again (VALUE: on or off; without it, it flips)
[positional-arguments]
resume VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.workspaces
    value="${1:-}"
    if [[ -z "${value}" ]]; then
        if [[ "$(gsettings get "${schema}" resume)" == true ]]; then value=off; else value=on; fi
    fi
    case "${value}" in
        on)
            gsettings set "${schema}" resume true
            echo "$(gettext "After an update's restart, the first login offers to open again the apps that were open, in their places.")"
            ;;
        off)
            gsettings set "${schema}" resume false
            echo "$(gettext "An update's restart keeps nothing of the open apps.")"
            ;;
        *)
            echo "usage: ame desktop resume on|off"
            exit 64
            ;;
    esac

# Saving data: on a connection that counts data, the pill says so and lists what waits for another network (VALUE: on
# or off; without it, it flips). The waiting is the connection's metered flag, which Settings switches per network.
[positional-arguments]
saving-data VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pill
    value="${1:-}"
    if [[ -z "${value}" ]]; then
        if [[ "$(gsettings get "${schema}" saving-data)" == true ]]; then value=off; else value=on; fi
    fi
    case "${value}" in
        on)
            gsettings set "${schema}" saving-data true
            echo "$(gettext "On a connection that counts data, the pill says Saving data and what waits.")"
            ;;
        off)
            gsettings set "${schema}" saving-data false
            echo "$(gettext "The pill says nothing of a connection that counts data. The automatic updates still wait on one.")"
            ;;
        *)
            echo "usage: ame desktop saving-data on|off"
            exit 64
            ;;
    esac

# A new network's zone: the first time a network is joined, a notification names its firewall zone, with Open Security
# (VALUE: on or off; without it, it flips). It switches nothing: naming a network home is ame security home-networks.
[positional-arguments]
zone-notice VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.privacy
    value="${1:-}"
    if [[ -z "${value}" ]]; then
        if [[ "$(gsettings get "${schema}" zone-notice)" == true ]]; then value=off; else value=on; fi
    fi
    case "${value}" in
        on)
            gsettings set "${schema}" zone-notice true
            echo "$(gettext "The first time a network is joined, a notification says which firewall zone it is in.")"
            ;;
        off)
            gsettings set "${schema}" zone-notice false
            echo "$(gettext "New networks are joined without a word about their zone.")"
            ;;
        *)
            echo "usage: ame desktop zone-notice on|off"
            exit 64
            ;;
    esac

# Open the tiling's settings: the gaps, the sizes windows step through, how the strip moves, every shortcut and the window rules
tiling-settings:
    #!/usr/bin/bash
    set -euo pipefail
    gnome-extensions prefs amethystora-ui@arsslensoft.github.io

# Windows in the classic layout: snap layouts, the switcher, the Super card, hot corners, show desktop, gestures,
# places and dimming, and for every layout the drop-down terminal and the scratchpad, keep on top, see-through
# windows, the frame, sizes for recording and reopening the app just closed:
# ame desktop windows [snap|switcher|show-desktop|places|dim on|off] [hold-super on|off|MS]
# [corner top-left|top-right|bottom-left|bottom-right ACTION] [gesture pinch|spread|down|today on|off] [places forget]
# [drop-down|scratchpad|keep-on-top|see-through|frame|recording-sizes|reopen on|off] [see-through-app APP PERCENT|off]
[positional-arguments]
windows *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.windows
    usage() {
        echo "usage: ame desktop windows [snap|switcher|show-desktop|places|dim on|off] [hold-super on|off|MS] [corner top-left|top-right|bottom-left|bottom-right none|overview|desktop|lock|note|notifications] [gesture pinch|spread|down|today on|off] [places forget] [drop-down|scratchpad|keep-on-top|see-through|frame|recording-sizes|reopen on|off] [see-through-app APP.desktop 40-100|off]"
        exit 64
    }
    # The words a person types back, so left as they are
    word() { if [[ "$(gsettings get "${schema}" "$1")" == true ]]; then echo on; else echo off; fi; }
    # on or off as GSettings has it; anything else is refused before a setting is touched
    bool() { if [[ "$1" == on ]]; then echo true; else echo false; fi; }
    onoff() { [[ "$1" == on || "$1" == off ]] || usage; }
    if [[ $# -eq 0 ]]; then
        for key in snap switcher hold-super show-desktop places dim; do
            echo "${key} $(word "${key}")"
        done
        echo "hold-super-delay $(gsettings get "${schema}" hold-super-delay)"
        for where in top-left top-right bottom-left bottom-right; do
            echo "corner ${where} $(gsettings get "${schema}" "corner-${where}" | tr -d "'")"
        done
        for name in pinch spread down today; do
            echo "gesture ${name} $(word "gesture-${name}")"
        done
        for key in drop-down scratchpad keep-on-top see-through frame recording-sizes reopen; do
            echo "${key} $(word "${key}")"
        done
        echo "see-through-apps $(gsettings get "${schema}" see-through-apps)"
        exit 0
    fi
    case "$1" in
        drop-down | scratchpad | keep-on-top | see-through | frame | recording-sizes | reopen)
            [[ $# -eq 2 ]] || usage
            onoff "$2"
            gsettings set "${schema}" "$1" "$(bool "$2")"
            case "$1:$2" in
                drop-down:on) echo "$(gettext "Super+Alt+Return slides a terminal down from the top of the screen, and hides it again, still running.")" ;;
                drop-down:off) echo "$(gettext "No drop-down terminal. One that was open is an ordinary terminal window again.")" ;;
                scratchpad:on) echo "$(gettext "In the classic layout, a window's menu can move it to the scratchpad, which Super+Alt+Return shows and hides.")" ;;
                scratchpad:off) echo "$(gettext "No scratchpad. The windows that were in it are back as they were.")" ;;
                keep-on-top:on) echo "$(gettext "Super+Alt+P keeps the window you are using on top, with a ring and a pin to show it.")" ;;
                keep-on-top:off) echo "$(gettext "Windows kept on top show nothing, and Super+Alt+P does nothing.")" ;;
                see-through:on) echo "$(gettext "Hold Super+Alt and scroll over a window to see through it, from 40 % to solid, until it closes.")" ;;
                see-through:off) echo "$(gettext "Every window is solid.")" ;;
                frame:on) echo "$(gettext "In the classic layout, the window you are using has a frame in the theme's colours.")" ;;
                frame:off) echo "$(gettext "No frame round the window in use.")" ;;
                recording-sizes:on) echo "$(gettext "A window's menu offers sizes for recording: 1280 × 720, 1920 × 1080, square and upright.")" ;;
                recording-sizes:off) echo "$(gettext "A window's menu offers no sizes for recording.")" ;;
                reopen:on) echo "$(gettext "Super+Alt+T opens again the app you just closed, at its place; five back. Nothing is written down.")" ;;
                reopen:off) echo "$(gettext "Super+Alt+T does nothing, and what had closed is forgotten.")" ;;
            esac
            ;;
        see-through-app)
            # An app's windows open see-through: its launcher's ID and a percentage from 40 to 100, or off
            [[ $# -eq 3 && "$2" =~ ^[A-Za-z0-9_][A-Za-z0-9._+-]*\.desktop$ ]] || usage
            app="$2"
            [[ "$3" == off ]] || { [[ "$3" =~ ^[0-9]{2,3}$ ]] && ((10#$3 >= 40 && 10#$3 <= 100)); } || usage
            # The rules as they are, each 'ID': PERCENT (IDs are checked names, so no quote is in one), less this app's
            entries=()
            while read -r id value; do
                [[ "${id}" == "${app}" ]] || entries+=("'${id}': ${value}")
            done < <(gsettings get "${schema}" see-through-apps | grep -oE "'[A-Za-z0-9._+-]+': [0-9]+" | tr -d "':")
            if [[ "$3" == off ]]; then
                echo "$(eval_gettext "\${app} opens solid.")"
            else
                percent="$((10#$3))"
                entries+=("'${app}': ${percent}")
                echo "$(eval_gettext "\${app} opens at \${percent} %. See-through windows work while they are on: ame desktop windows see-through on")"
            fi
            joined="$(IFS=,; echo "${entries[*]}")"
            gsettings set "${schema}" see-through-apps "@a{si} {${joined//,/, }}"
            ;;
        snap | switcher | show-desktop | dim)
            [[ $# -eq 2 ]] || usage
            onoff "$2"
            gsettings set "${schema}" "$1" "$(bool "$2")"
            case "$1:$2" in
                snap:on) echo "$(gettext "Snap layouts are on: drag a window to the top of the screen, or press Super+Z.")" ;;
                snap:off) echo "$(gettext "Snap layouts are off.")" ;;
                switcher:on) echo "$(gettext "Super+Tab and Alt+Tab show every window in a grid you can type into.")" ;;
                switcher:off) echo "$(gettext "Super+Tab and Alt+Tab are GNOME's own switchers again.")" ;;
                show-desktop:on) echo "$(gettext "A click on empty wallpaper slides the windows to the edges; a click anywhere brings them back.")" ;;
                show-desktop:off) echo "$(gettext "A click on the wallpaper does nothing.")" ;;
                dim:on) echo "$(gettext "The windows behind the one you are using are drawn a little darker.")" ;;
                dim:off) echo "$(gettext "No window is dimmed.")" ;;
            esac
            ;;
        places)
            [[ $# -eq 2 ]] || usage
            if [[ "$2" == forget ]]; then
                was="$(gsettings get "${schema}" places)"
                # Off first, so that nothing is written while the files go
                gsettings set "${schema}" places false
                sleep 1
                rm -f "${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/places.json" \
                    "${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/places.json.bak"
                gsettings set "${schema}" places "${was}"
                echo "$(gettext "The places windows went back to are forgotten.")"
                exit 0
            fi
            onoff "$2"
            gsettings set "${schema}" places "$(bool "$2")"
            if [[ "$2" == on ]]; then
                echo "$(gettext "Windows go back to their places for each arrangement of screens.")"
            else
                echo "$(gettext "Windows open where GNOME puts them.")"
            fi
            ;;
        hold-super)
            [[ $# -eq 2 ]] || usage
            case "$2" in
                on | off)
                    gsettings set "${schema}" hold-super "$(bool "$2")"
                    if [[ "$2" == on ]]; then
                        echo "$(gettext "Holding Super shows the main shortcuts and the dock's numbers.")"
                    else
                        echo "$(gettext "Holding Super shows nothing.")"
                    fi
                    ;;
                *)
                    # Base 10, so that 0900 is nine hundred and not an octal number bash refuses
                    [[ "$2" =~ ^[0-9]{1,5}$ ]] && ((10#$2 >= 500 && 10#$2 <= 3000)) || usage
                    set -- "$1" "$((10#$2))"
                    gsettings set "${schema}" hold-super-delay "$2"
                    gsettings set "${schema}" hold-super true
                    ms="$2"
                    echo "$(eval_gettext "The shortcuts show after Super has been held for \${ms} milliseconds.")"
                    ;;
            esac
            ;;
        corner)
            [[ $# -eq 3 ]] || usage
            case "$2" in top-left | top-right | bottom-left | bottom-right) ;; *) usage ;; esac
            case "$3" in none | overview | desktop | lock | note | notifications) ;; *) usage ;; esac
            gsettings set "${schema}" "corner-$2" "$3"
            where="$2"
            action="$3"
            echo "$(eval_gettext "The \${where} corner now does: \${action}.")"
            ;;
        gesture)
            [[ $# -eq 3 ]] || usage
            case "$2" in pinch | spread | down | today) ;; *) usage ;; esac
            onoff "$3"
            gsettings set "${schema}" "gesture-$2" "$(bool "$3")"
            name="$2"
            state="$3"
            echo "$(eval_gettext "The \${name} gesture is \${state}.")"
            ;;
        *) usage ;;
    esac

# The window settings GNOME keeps out of sight, GNOME's own keys: ame desktop window-settings
# [focus-follows-pointer|centre-new|resize-right|attach-dialogs on|off] [double-click|middle-click|right-click ACTION]
# [drag-key super|alt]
[positional-arguments]
window-settings *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    wm=org.gnome.desktop.wm.preferences
    mutter=org.gnome.mutter
    usage() {
        echo "usage: ame desktop window-settings [focus-follows-pointer|centre-new|resize-right|attach-dialogs on|off] [double-click|middle-click|right-click toggle-maximize|minimize|lower|menu|none] [drag-key super|alt]"
        exit 64
    }
    word() { if [[ "$(gsettings get "$1" "$2")" == true ]]; then echo on; else echo off; fi; }
    bool() { if [[ "$1" == on ]]; then echo true; else echo false; fi; }
    onoff() { [[ "$1" == on || "$1" == off ]] || usage; }
    if [[ $# -eq 0 ]]; then
        if [[ "$(gsettings get "${wm}" focus-mode)" == "'click'" ]]; then echo "focus-follows-pointer off"; else echo "focus-follows-pointer on"; fi
        echo "centre-new $(word "${mutter}" center-new-windows)"
        for click in double middle right; do
            echo "${click}-click $(gsettings get "${wm}" "action-${click}-click-titlebar" | tr -d "'")"
        done
        if [[ "$(gsettings get "${wm}" mouse-button-modifier)" == "'<Alt>'" ]]; then echo "drag-key alt"; else echo "drag-key super"; fi
        echo "resize-right $(word "${wm}" resize-with-right-button)"
        echo "attach-dialogs $(word "${mutter}" attach-modal-dialogs)"
        exit 0
    fi
    [[ $# -eq 2 ]] || usage
    case "$1" in
        focus-follows-pointer)
            onoff "$2"
            if [[ "$2" == on ]]; then
                gsettings set "${wm}" focus-mode sloppy
                echo "$(gettext "The window under the pointer takes the keys.")"
            else
                gsettings set "${wm}" focus-mode click
                echo "$(gettext "A window takes the keys when you click it.")"
            fi
            ;;
        centre-new)
            onoff "$2"
            gsettings set "${mutter}" center-new-windows "$(bool "$2")"
            if [[ "$2" == on ]]; then
                echo "$(gettext "New windows open in the middle of the screen.")"
            else
                echo "$(gettext "New windows open where GNOME places them.")"
            fi
            ;;
        double-click | middle-click | right-click)
            case "$2" in toggle-maximize | minimize | lower | menu | none) ;; *) usage ;; esac
            gsettings set "${wm}" "action-${1%-click}-click-titlebar" "$2"
            click="$1"
            action="$2"
            echo "$(eval_gettext "A \${click} on a title bar now does: \${action}.")"
            ;;
        drag-key)
            case "$2" in
                super)
                    gsettings set "${wm}" mouse-button-modifier '<Super>'
                    echo "$(gettext "Hold Super and drag to move a window from anywhere in it.")"
                    ;;
                alt)
                    gsettings set "${wm}" mouse-button-modifier '<Alt>'
                    echo "$(gettext "Hold Alt and drag to move a window from anywhere in it. Apps no longer see Alt with a click.")"
                    ;;
                *) usage ;;
            esac
            ;;
        resize-right)
            onoff "$2"
            gsettings set "${wm}" resize-with-right-button "$(bool "$2")"
            if [[ "$2" == on ]]; then
                echo "$(gettext "Hold the drag key and drag with the right button to resize a window.")"
            else
                echo "$(gettext "The drag key with the right button opens the window's menu.")"
            fi
            ;;
        attach-dialogs)
            onoff "$2"
            # The tiling keeps dialogs apart while it runs (a dialog glued to a window the strip moves is left behind),
            # and gives back what was there when it stops: changed now, the tiling would take it as the person's
            if [[ "$(gsettings get org.gnome.shell.extensions.amethystora-ui.tiling enabled 2>/dev/null)" != false ]]; then
                echo "$(gettext "The tiling keeps dialogs apart while it is on. Switch to the classic layout first: ame desktop layout classic")" >&2
                exit 1
            fi
            gsettings set "${mutter}" attach-modal-dialogs "$(bool "$2")"
            if [[ "$2" == on ]]; then
                echo "$(gettext "A dialog stays attached to its window and moves with it.")"
            else
                echo "$(gettext "A dialog is a window of its own.")"
            fi
            ;;
        *) usage ;;
    esac

# Pick how long the clipboard history (Super+Shift+V) keeps what you copied, sealed: always, restart (until the computer restarts) or session (until you log out); pinned items are always kept
[positional-arguments]
clipboard KEEP="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-clipboard
    keep="${1:-}"
    if [[ -z "${keep}" ]]; then
        current="$(gsettings get "${schema}" keep-history | tr -d "'")"
        echo "always   $(gettext "Everything you copied, until you clear it")"
        echo "restart  $(gettext "Pinned items, and the rest until the computer restarts")"
        echo "session  $(gettext "Pinned items, and the rest until you log out")"
        keep="$(gum choose --header "$(eval_gettext "Keep the clipboard history (current: \${current})")" --selected "${current}" \
            always restart session)" || exit 0
    fi
    case "${keep}" in
        always | restart | session) ;;
        *)
            echo "usage: ame desktop clipboard always|restart|session"
            exit 64
            ;;
    esac
    gsettings set "${schema}" keep-history "${keep}"
    case "${keep}" in
        always) echo "$(gettext "The clipboard history is kept until you clear it.")" ;;
        restart) echo "$(gettext "Pinned items are kept, and the rest of the clipboard history until the computer restarts.")" ;;
        session) echo "$(gettext "Pinned items are kept, and the rest of the clipboard history until you log out.")" ;;
    esac
    echo "$(gettext "It is sealed on disk, under a key in your login keyring.")"

# Empty the trash of what was deleted more than DAYS days ago (1 to 3650), checked every hour while you are logged in, or never (off): ame desktop privacy trash DAYS|off
[positional-arguments]
privacy WHAT="" VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.desktop.privacy
    usage() {
        echo "usage: ame desktop privacy trash DAYS|off"
        exit 64
    }
    [[ "${1:-trash}" == trash ]] || usage
    value="${2:-}"
    if [[ -z "${value}" ]]; then
        current=off
        if [[ "$(gsettings get "${schema}" remove-old-trash-files)" == true ]]; then
            current="$(gsettings get "${schema}" old-files-age | sed 's/^uint32 //')"
        fi
        value="$(gum choose --header "$(eval_gettext "Empty the trash of what was deleted more than this many days ago (current: \${current})")" \
            --selected "${current}" 7 14 30 60 off)" || exit 0
    fi
    if [[ "${value}" == off ]]; then
        gsettings set "${schema}" remove-old-trash-files false
        gsettings set "${schema}" remove-old-temp-files false
    elif [[ "${value}" =~ ^[0-9]+$ ]] && ((10#${value} >= 1 && 10#${value} <= 3650)); then
        gsettings set "${schema}" old-files-age "$((10#${value}))"
        gsettings set "${schema}" remove-old-trash-files true
        gsettings set "${schema}" remove-old-temp-files true
    else
        usage
    fi
    # GNOME's housekeeping reads these keys when it starts and when a key of its own schema changes, never
    # when they do (gnome-settings-daemon 50, gsd-disk-space.c): one of its own keys changed and put back
    # makes the running session take them now
    housekeeping=org.gnome.settings-daemon.plugins.housekeeping
    was="$(dconf read /org/gnome/settings-daemon/plugins/housekeeping/min-notify-period)"
    period="$(gsettings get "${housekeeping}" min-notify-period | sed 's/^[a-z0-9]* //')"
    gsettings set "${housekeeping}" min-notify-period "$((period > 1 ? period - 1 : period + 1))"
    if [[ -n "${was}" ]]; then
        gsettings set "${housekeeping}" min-notify-period "${was}"
    else
        gsettings reset "${housekeeping}" min-notify-period
    fi
    if [[ "${value}" == off ]]; then
        echo "$(gettext "The trash is never emptied by itself.")"
    else
        days=$((10#${value}))
        echo "$(eval_gettext "The trash is emptied of what was deleted more than \${days} days ago, checked every hour while you are logged in.")"
        echo "$(gettext "What it empties stays in the snapshots and the backup, where they are on.")"
    fi

# The top bar's widgets, which the AI agent can make: list, make, new, run, fix, on, off, remove (amethystora-widgets)
[no-cd]
[no-exit-message]
[positional-arguments]
widgets *ARGS:
    @amethystora-widgets "$@"

# Show the tiling, workspace and launcher keybindings (Super+Alt+Space, then Keybindings)
keybindings:
    #!/usr/bin/bash
    # In the session's language when the manual has the page translated (manual/<language>/), as the
    # Manual itself shows it
    file=/usr/share/amethystora/keybindings.md
    locale="${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}"
    locale="${locale%%[.@]*}"
    for language in "${locale/_/-}" "${locale%%_*}"; do
        if [[ -n "${language}" && -f "/usr/share/amethystora/manual/${language}/keybindings.md" ]]; then
            file="/usr/share/amethystora/manual/${language}/keybindings.md"
            break
        fi
    done
    # The page is wrapped at 100 columns and glow keeps its line ends, so a paragraph's lines are joined
    # first for glow to wrap to the terminal
    page="$(mktemp --suffix=.md)"
    trap 'rm -f "${page}"' EXIT
    awk '/^```/ { fence = !fence }
        { block = fence || $0 == "" || $0 ~ /^(\||#|```)/
          item = $0 ~ /^ *([-*+]|[0-9]+[.)]) / }
        !block && !item && open { printf " %s", $0; next }
        { if (NR > 1) print ""; printf "%s", $0; open = !block }
        END { print "" }' "${file}" >"${page}"
    glow --pager "${page}"

# Put the Amethystora artwork on the GRUB boot menu, or take it off again
[positional-arguments]
boot-menu ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # The choices are shown in the session's language, and what gum prints back is matched against the same words
    # Translators: a choice in a menu: to put the artwork on the boot menu
    turn_on="$(gettext "Turn on")"
    # Translators: a choice in a menu: to take the artwork off the boot menu
    turn_off="$(gettext "Turn off")"

    action="${1:-}"
    amethystora-grub-theme status
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Boot menu")" "${turn_on}" "${turn_off}" "$(gettext "Cancel")")" || exit 0
    fi
    case "${action}" in
        "${turn_on}" | "Turn on" | install | on) sudo amethystora-grub-theme install ;;
        "${turn_off}" | "Turn off" | remove | off) sudo amethystora-grub-theme remove ;;
        *) exit 0 ;;
    esac

# The logo and system summary shown in new terminals (ACTION: on or off; without it, it flips)
[positional-arguments]
greeting ACTION="":
    #!/usr/bin/bash
    # shellcheck source=/dev/null
    source gettext.sh
    FLAG="${XDG_CONFIG_HOME:-$HOME/.config}/amethystora/no-greeting"
    action="${1:-}"
    if [[ -z "${action}" ]]; then
        if [ -e "${FLAG}" ]; then action=on; else action=off; fi
    fi
    case "${action}" in
        on)
            rm -f "${FLAG}"
            echo "$(gettext "Terminal greeting enabled")"
            ;;
        off)
            mkdir -p "$(dirname "${FLAG}")"
            touch "${FLAG}"
            echo "$(gettext "Terminal greeting disabled")"
            ;;
        *)
            echo "usage: ame desktop greeting [on|off]"
            exit 64
            ;;
    esac

# A long command (ten seconds or more) that ends while its terminal is not in front says so in a notification, which
# brings the terminal forward when clicked: kitty's own setting, on as the image ships (ACTION: on or off; without
# it, it says which). Written in ~/.config/amethystora/kitty.conf, which the image's kitty.conf reads last
[positional-arguments]
command-done ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    FILE="${XDG_CONFIG_HOME:-$HOME/.config}/amethystora/kitty.conf"
    LINE="notify_on_cmd_finish never"
    off() { [[ -f "${FILE}" ]] && grep -qx "${LINE}" "${FILE}"; }
    case "${1:-}" in
        "")
            if off; then echo off; else echo on; fi
            exit 0
            ;;
        on | off) ;;
        *)
            echo "usage: ame desktop command-done [on|off]"
            exit 64
            ;;
    esac
    mkdir -p "$(dirname "${FILE}")"
    kept=""
    [[ -f "${FILE}" ]] && kept="$(grep -vx "${LINE}" "${FILE}" || true)"
    [[ -n "${kept}" ]] || kept="# Written by `ame desktop command-done`. Your own settings go in ~/.config/kitty/kitty.conf."
    if [[ "$1" == off ]]; then
        printf '%s\n%s\n' "${kept}" "${LINE}" >"${FILE}.new"
    else
        printf '%s\n' "${kept}" >"${FILE}.new"
    fi
    mv "${FILE}.new" "${FILE}"
    # The account's open kitty windows read their settings again, as a theme switch has them do
    pkill -USR1 -U "$(id -u)" -x kitty 2>/dev/null || true
    if [[ "$1" == on ]]; then
        echo "$(gettext "A long command that ends while its terminal is behind now says so in a notification.")"
    else
        echo "$(gettext "A long command that ends no longer says so in a notification.")"
    fi

# Where a missing command can come from: type a command this computer lacks, in bash, zsh or fish, and the terminal
# says where it can come from on this system (Homebrew, a container, Flathub, an `ame` command) instead of a bare
# "command not found"; it never installs anything. Off as the image ships (ACTION: on or off; without it, it says
# which). New terminals follow it
[positional-arguments]
missing-command ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    FLAG="${XDG_CONFIG_HOME:-$HOME/.config}/amethystora/missing-command"
    case "${1:-}" in
        "") if [[ -e "${FLAG}" ]]; then echo on; else echo off; fi ;;
        on)
            mkdir -p "$(dirname "${FLAG}")"
            touch "${FLAG}"
            echo "$(gettext "A command this computer lacks now says where it can come from, in new terminals. Nothing is installed by it.")"
            ;;
        off)
            rm -f "${FLAG}"
            echo "$(gettext "A command this computer lacks says only that it was not found, in new terminals.")"
            ;;
        *)
            echo "usage: ame desktop missing-command [on|off]"
            exit 64
            ;;
    esac

# Your Android phone through Amethystora Connect, which speaks KDE Connect's protocol: notifications, files, texts and
# calls (ACTION: on or off; without it, it flips). Off until turned on: the program listens only while its user
# service is enabled. `phone clipboard send|receive [on|off]` shares the clipboard one way or the other, both off;
# `phone photos [pictures|downloads]` says where pictures a phone sends go, Pictures/Phone as the image ships;
# `phone computers [on|off]` lets other computers with the phone link pair too, for files and the clipboard, off;
# `phone ring` rings the paired phones, even on silent; `phone music [on|off]` shows the music of either side on the
# other, and `phone clicker [on|off]` lets a phone go to the next or previous slide while Present mode presents, both off
[positional-arguments]
phone *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    UNIT=amethystora-connect.service
    on() { [[ "$(systemctl --user is-enabled "${UNIT}" 2>/dev/null)" == enabled ]]; }
    # on or off for a word that may be missing, which flips the current state
    flip() { if [[ -n "$1" ]]; then echo "$1"; elif [[ "$2" == true ]]; then echo off; else echo on; fi; }
    if [[ "${1:-}" == clipboard ]]; then
        case "${2:-}" in
            send | receive) key="clipboard-$2" ;;
            *)
                echo "usage: ame desktop phone clipboard send|receive [on|off]"
                exit 64
                ;;
        esac
        case "$(flip "${3:-}" "$(gsettings get org.amethystora.Connect "${key}")")" in
            on)
                gsettings set org.amethystora.Connect "${key}" true
                if [[ "$2" == send ]]; then
                    echo "$(gettext "What you copy here is sent to the phones you paired.")"
                else
                    echo "$(gettext "What a paired phone copies is put on this computer's clipboard, where any program can paste it.")"
                fi
                ;;
            off)
                gsettings set org.amethystora.Connect "${key}" false
                echo "$(gettext "The clipboard is not shared that way")"
                ;;
            *)
                echo "usage: ame desktop phone clipboard send|receive [on|off]"
                exit 64
                ;;
        esac
        exit 0
    fi
    if [[ "${1:-}" == photos ]]; then
        # Without a value it flips between the two
        where="${2:-}"
        if [[ -z "${where}" ]]; then
            [[ "$(gsettings get org.amethystora.Connect photo-folder)" == "'downloads'" ]] && where=pictures || where=downloads
        fi
        case "${where}" in
            pictures)
                gsettings set org.amethystora.Connect photo-folder pictures
                echo "$(gettext "Photos from the phone go to Pictures/Phone.")"
                ;;
            downloads)
                gsettings set org.amethystora.Connect photo-folder downloads
                echo "$(gettext "Photos from the phone go to Downloads, with the phone's other files.")"
                ;;
            *)
                echo "usage: ame desktop phone photos [pictures|downloads]"
                exit 64
                ;;
        esac
        exit 0
    fi
    if [[ "${1:-}" == ring ]]; then
        # The Phone app rings them without its window, and shows it only to say why it could not
        [[ $# -eq 1 ]] || { echo "usage: ame desktop phone ring"; exit 64; }
        exec /usr/bin/amethystora-phone ring
    fi
    if [[ "${1:-}" == music || "${1:-}" == clicker ]]; then
        key="$1"
        case "$(flip "${2:-}" "$(gsettings get org.amethystora.Connect "${key}")")" in
            on)
                gsettings set org.amethystora.Connect "${key}" true
                if [[ "${key}" == music ]]; then
                    echo "$(gettext "Music from either side is on: what plays here shows on your paired phone, and what plays on the phone shows in the clock's pill.")"
                else
                    echo "$(gettext "While Present mode presents, your paired phone can go to the next or previous slide, or blank the screen, and nothing else.")"
                fi
                ;;
            off)
                gsettings set org.amethystora.Connect "${key}" false
                if [[ "${key}" == music ]]; then
                    echo "$(gettext "Music from either side is off")"
                else
                    echo "$(gettext "The phone is no slide clicker now: it can send no keys at all.")"
                fi
                ;;
            *)
                echo "usage: ame desktop phone ${key} [on|off]"
                exit 64
                ;;
        esac
        exit 0
    fi
    if [[ "${1:-}" == computers ]]; then
        case "$(flip "${2:-}" "$(gsettings get org.amethystora.Connect computers)")" in
            on)
                gsettings set org.amethystora.Connect computers true
                echo "$(gettext "Other computers with the phone link on can now pair with this one, on the networks named home. Turn this on on both.")"
                ;;
            off)
                gsettings set org.amethystora.Connect computers false
                echo "$(gettext "Only phones and tablets pair with this computer now. Paired computers are kept, and connect again once this is on.")"
                ;;
            *)
                echo "usage: ame desktop phone computers [on|off]"
                exit 64
                ;;
        esac
        exit 0
    fi
    case "$(flip "${1:-}" "$(on && echo true)")" in
        on)
            if ! systemctl --user enable --now "${UNIT}" 2>/dev/null; then
                # Outside a session there is no manager to start it: the next login does
                systemctl --user enable "${UNIT}"
            fi
            echo "$(gettext "Phone pairing is on: install KDE Connect on the phone, and pair them on a network you named home.")"
            ;;
        off)
            if ! systemctl --user disable --now "${UNIT}" 2>/dev/null; then
                systemctl --user disable "${UNIT}"
            fi
            echo "$(gettext "Phone pairing is off")"
            ;;
        *)
            echo "usage: ame desktop phone [on|off]"
            exit 64
            ;;
    esac

# Live captions: words under the speech this computer plays, made on this computer by the Vosk recognizer
# (ACTION: on, off, remove, status or language; without it, it flips). `on` fetches the speech model of the captions'
# language once, after saying what it downloads and asking; `remove` turns captions and dictation off and deletes every
# model; `language LANG` picks the language captions and dictation hear, fetching its model the same way (without
# LANG, it asks which)
[positional-arguments]
captions *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.captions
    helper=/usr/libexec/amethystora-captions
    usage() {
        echo "usage: ame desktop captions [on|off|remove|status|language [en|fr|es|de|it|tr|pt|nl|ru|pl]]"
        exit 64
    }
    [[ $# -le 1 || ( $# -eq 2 && "$1" == language ) ]] || usage
    action="${1:-}"
    lang="$(gsettings get "${schema}" language | tr -d "'")"
    # What the helper says of one language: its name, or its download in MiB
    about() {
        "${helper}" languages | jq -r --arg code "$1" --arg what "$2" '.[] | select(.code == $code) |
            if $what == "size" then (.download / 1048576 | round | tostring) else .name end'
    }
    # Fetches the model of a language once, after saying what it downloads and asking; fails when it is not here after
    fetch_model() {
        local name size
        case "$("${helper}" status "$1")" in
            no-engine)
                echo "$(gettext "The speech recognizer is not installed in this image.")"
                return 1
                ;;
            no-model)
                name="$(about "$1" name)"
                size="$(about "$1" size)"
                echo "$(eval_gettext "Captions and dictation in \${name} need a speech model of \${size} MiB, downloaded once from alphacephei.com, the makers of the Vosk recognizer. After that, they are made on this computer: what you hear or say is never recorded or sent.")"
                echo "$(gettext "While they are on, they hold about 230 MB of memory, and use part of a processor core while speech plays.")"
                gum confirm "$(gettext "Download it now?")" || return 1
                if ! "${helper}" fetch "$1"; then
                    echo "$(gettext "The download failed, and nothing was kept.")"
                    return 1
                fi
                ;;
        esac
    }
    if [[ -z "${action}" ]]; then
        if [[ "$(gsettings get "${schema}" enabled)" == true ]]; then action=off; else action=on; fi
    fi
    case "${action}" in
        on)
            fetch_model "${lang}" || exit 1
            gsettings set "${schema}" enabled true
            echo "$(gettext "Live captions are on: they show under speech the computer plays. Quick settings turns them off.")"
            ;;
        off)
            gsettings set "${schema}" enabled false
            echo "$(gettext "Live captions are off.")"
            ;;
        remove)
            # The model goes first: the shell's watch of its folder takes the toggle away, and the key then says off
            had="$("${helper}" languages | jq '[.[] | select(.here)] | length')"
            "${helper}" remove
            gsettings set "${schema}" enabled false
            gsettings set "${schema}" dictation false
            if [[ "${had}" != 0 ]]; then
                echo "$(gettext "The speech models are removed, and live captions and dictation are off.")"
            else
                echo "$(gettext "Live captions are off, and no speech model is left.")"
            fi
            ;;
        status)
            if [[ "$(gsettings get "${schema}" enabled)" == true ]]; then
                echo "$(gettext "Live captions are on.")"
            else
                echo "$(gettext "Live captions are off.")"
            fi
            case "$("${helper}" status "${lang}")" in
                ready) echo "$(gettext "The speech model is on this computer.")" ;;
                no-model) echo "$(gettext "The speech model is not downloaded.")" ;;
                *) echo "$(gettext "The speech recognizer is not installed in this image.")" ;;
            esac
            name="$(about "${lang}" name)"
            echo "$(eval_gettext "Language: \${name}")"
            ;;
        language)
            code="${2:-}"
            if [[ -z "${code}" ]]; then
                code="$("${helper}" languages | jq -r '.[] | .code + "  " + .name' |
                    gum choose --header "$(gettext "Captions language")" --selected "${lang}  $(about "${lang}" name)")" || exit 0
                code="${code%% *}"
            fi
            [[ -n "$(about "${code}" name)" ]] || usage
            fetch_model "${code}" || exit 1
            gsettings set "${schema}" language "${code}"
            name="$(about "${code}" name)"
            echo "$(eval_gettext "Captions and dictation hear \${name} now.")"
            ;;
        *) usage ;;
    esac

# Dictation: Super+Ctrl+Shift+D listens to the microphone and types what is said into the text field in use, made on
# this computer by the captions' recognizer, in their language (ACTION: on or off; without it, it says which). `on`
# fetches that language's speech model first if it is not here, after saying what it downloads and asking
[positional-arguments]
dictation ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.captions
    helper=/usr/libexec/amethystora-captions
    lang="$(gsettings get "${schema}" language | tr -d "'")"
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" dictation)" == true ]]; then
                echo "$(gettext "Dictation is on: Super+Ctrl+Shift+D starts and stops it.")"
            else
                echo "$(gettext "Dictation is off.")"
            fi
            ;;
        on)
            if [[ "$("${helper}" status "${lang}")" != ready ]]; then
                # The model comes as the captions' does: said first, asked, checked
                ame desktop captions language "${lang}" || exit 1
                [[ "$("${helper}" status "${lang}")" == ready ]] || exit 1
            fi
            gsettings set "${schema}" dictation true
            echo "$(gettext "Dictation is on. Click into a text field, press Super+Ctrl+Shift+D and speak; press it again to stop. It listens on this computer only, and what you say is never recorded or sent.")"
            ;;
        off)
            gsettings set "${schema}" dictation false
            echo "$(gettext "Dictation is off.")"
            ;;
        *)
            echo "usage: ame desktop dictation [on|off]"
            exit 64
            ;;
    esac

# The live pill: the clock shows what is happening now (ame desktop pill [on|off]); the privacy dots and the
# recording glow beside it (ame desktop pill dots|glow [on|off]); a meeting about to start, with Join (ame desktop pill
# meetings [on|off]). Without on or off, it asks
[positional-arguments]
pill *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pill
    usage() {
        echo "usage: ame desktop pill [dots|glow|meetings] [on|off]"
        exit 64
    }
    what=live
    case "${1:-}" in
        dots) what=privacy-dots; shift ;;
        glow) what=glow; shift ;;
        meetings) what=meetings; shift ;;
    esac
    [[ $# -le 1 ]] || usage
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" "${what}")" == true ]]; then current=on; else current=off; fi
        state="$(gum choose --header "$(eval_gettext "\${what} (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    case "${what}:${state}" in
        live:on) echo "$(gettext "The live pill is on: the clock shows what is happening now.")" ;;
        live:off) echo "$(gettext "The live pill is off: the clock is GNOME's own.")" ;;
        privacy-dots:on) echo "$(gettext "The privacy dots are on: a dot in the top bar while the microphone, the camera, the screen or the location is in use.")" ;;
        privacy-dots:off) echo "$(gettext "The privacy dots are off: GNOME's own icons show what is in use.")" ;;
        glow:on) echo "$(gettext "The recording glow is on: a red glow round the screen while it is recorded or shared. It shows in a recording of the whole screen.")" ;;
        glow:off) echo "$(gettext "The recording glow is off.")" ;;
        meetings:on) echo "$(gettext "Meetings in the pill are on: five minutes before an event with a video-call link, the pill shows it with Join, while the live pill is on.")" ;;
        meetings:off) echo "$(gettext "Meetings in the pill are off.")" ;;
        *) usage ;;
    esac
    if [[ "${state}" == on ]]; then gsettings set "${schema}" "${what}" true; else gsettings set "${schema}" "${what}" false; fi

# Quick settings: put every tile back where it was and show them all (reset), or show or hide the theme block (theme on|off)
[positional-arguments]
quick-settings ACTION="" VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.quick
    usage() {
        echo "usage: ame desktop quick-settings reset|theme [on|off]"
        exit 64
    }
    # The tiles put away, one id a line (an id is lower-case letters, digits and hyphens)
    hidden() {
        gsettings get "${schema}" tile-hidden | sed -e 's/^@as //' -e "s/[][',]/ /g" | tr ' ' '\n' |
            grep -E '^[a-z0-9-]{1,40}$' || true
    }
    theme_state() { if hidden | grep -qx theme; then echo off; else echo on; fi; }
    action="${1:-}"
    if [[ -z "${action}" ]]; then
        [[ -t 0 && -t 1 ]] || usage
        current="$(theme_state)"
        echo "reset  $(gettext "Every tile back in its first order, and every tile shown")"
        echo "theme  $(eval_gettext "Show or hide the theme block (now \${current})")"
        action="$(gum choose reset theme)" || exit 0
    fi
    case "${action}" in
        reset)
            [[ -z "${2:-}" ]] || usage
            gsettings reset "${schema}" tile-order
            gsettings reset "${schema}" tile-hidden
            echo "$(gettext "Quick settings have every tile again, in their first order.")"
            ;;
        theme)
            value="${2:-}"
            if [[ -z "${value}" ]]; then
                [[ -t 0 && -t 1 ]] || usage
                current="$(theme_state)"
                value="$(gum choose --header "$(eval_gettext "The theme block (current: \${current})")" --selected "${current}" on off)" || exit 0
            fi
            # The other tiles put away stay so
            others="$(hidden | grep -vx theme || true)"
            case "${value}" in
                on) list="${others}" ;;
                off) list="${others}"$'\n'theme ;;
                *) usage ;;
            esac
            gsettings set "${schema}" tile-hidden "[$(grep -v '^$' <<<"${list}" | sed "s/.*/'&'/" | paste -sd, -)]"
            if [[ "${value}" == on ]]; then
                echo "$(gettext "The theme block is in quick settings.")"
            else
                echo "$(gettext "The theme block is hidden from quick settings.")"
            fi
            ;;
        *) usage ;;
    esac

# Quiet: banners wait in the list while a window is fullscreen or the screen is shared or recorded (ame desktop quiet
# [on|off]), and between Night Light's times (ame desktop quiet night [on|off]). Without on or off, it asks
[positional-arguments]
quiet *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pill
    color=org.gnome.settings-daemon.plugins.color
    usage() {
        echo "usage: ame desktop quiet [night] [on|off]"
        exit 64
    }
    what=quiet
    if [[ "${1:-}" == night ]]; then what=quiet-night; shift; fi
    [[ $# -le 1 ]] || usage
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" "${what}")" == true ]]; then current=on; else current=off; fi
        state="$(gum choose --header "$(eval_gettext "\${what} (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    # Night Light's times as HH:MM, from its decimal hours
    hhmm() { awk -v h="$1" 'BEGIN { m = int(h * 60 + 0.5) % 1440; printf "%02d:%02d", int(m / 60), m % 60 }'; }
    case "${what}:${state}" in
        quiet:on)
            gsettings set "${schema}" quiet true
            echo "$(gettext "Quiet is on: banners wait in the list while a window is fullscreen or the screen is shared or recorded. Calls, alarms and urgent ones still come through.")"
            ;;
        quiet:off)
            gsettings set "${schema}" quiet false
            echo "$(gettext "Quiet while fullscreen or sharing is off.")"
            ;;
        quiet-night:on)
            gsettings set "${schema}" quiet-night true
            if [[ "$(gsettings get "${color}" night-light-schedule-automatic)" == true ]]; then
                echo "$(gettext "Quiet at night is on, from sunset to sunrise.")"
            else
                from="$(hhmm "$(gsettings get "${color}" night-light-schedule-from)")"
                to="$(hhmm "$(gsettings get "${color}" night-light-schedule-to)")"
                echo "$(eval_gettext "Quiet at night is on, between Night Light's times: \${from} to \${to}.")"
            fi
            ;;
        quiet-night:off)
            gsettings set "${schema}" quiet-night false
            echo "$(gettext "Quiet at night is off.")"
            ;;
        *) usage ;;
    esac

# The digest: banners that are not urgent wait and come together at the digest times (ame desktop digest [on|off];
# ame desktop digest times HH:MM [HH:MM]..., one to six of them; ame desktop digest through|wait [APP.desktop...], the
# apps that always come through and the apps that always wait). Without on or off, it asks
[positional-arguments]
digest *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pill
    # The times as the key has them, sorted, each once
    times() { gsettings get "${schema}" digest-times | tr -d "[]',@as" | xargs | sed 's/ /, /g'; }
    if [[ "${1:-}" == times ]]; then
        shift
        if [[ $# -lt 1 || $# -gt 6 ]]; then
            echo "usage: ame desktop digest times HH:MM [HH:MM]..."
            exit 64
        fi
        for t in "$@"; do
            if [[ ! "${t}" =~ ^([01][0-9]|2[0-3]):[0-5][0-9]$ ]]; then
                echo "usage: ame desktop digest times HH:MM [HH:MM]..."
                exit 64
            fi
        done
        list="$(printf '%s\n' "$@" | sort -u | sed "s/.*/'&'/" | paste -sd, -)"
        gsettings set "${schema}" digest-times "[${list}]"
        shown="$(times)"
        echo "$(eval_gettext "The digest comes at \${shown}.")"
        exit 0
    fi
    # The two lists (R5-33): apps whose banners always come through while the digest is on, and apps whose banners
    # always wait for its next time, also while it is off. By launcher ID; an app is on one list at most. Without IDs,
    # in a terminal, pick from the launchers a person sees.
    if [[ "${1:-}" == through || "${1:-}" == wait ]]; then
        which="$1"
        key="digest-${which}"
        other="digest-through"
        [[ "${which}" == through ]] && other="digest-wait"
        shift
        usage() { echo "usage: ame desktop digest through|wait [APP.desktop...] | through|wait reset" >&2; exit 64; }
        if [[ "${1:-}" == reset ]]; then
            [[ $# -eq 1 ]] || usage
            gsettings reset "${schema}" "${key}"
            if [[ "${which}" == through ]]; then
                echo "$(gettext "No app comes through the digest by itself any more.")"
            else
                echo "$(gettext "No app waits for the digest by itself any more.")"
            fi
            exit 0
        fi
        dirs=("${XDG_DATA_HOME:-${HOME}/.local/share}/applications" "${HOME}/.local/share/flatpak/exports/share/applications"
            /var/lib/flatpak/exports/share/applications /usr/local/share/applications /usr/share/applications)
        apps=("$@")
        if ((${#apps[@]} == 0)); then
            [[ -t 0 ]] || usage
            declare -A seen=()
            labels=() ids=()
            for dir in "${dirs[@]}"; do
                for file in "${dir}"/*.desktop; do
                    [[ -f "${file}" ]] || continue
                    id="${file##*/}"
                    [[ -z "${seen[${id}]:-}" ]] || continue
                    seen[${id}]=1
                    grep -qx 'NoDisplay=true' "${file}" && continue
                    label="$(sed -n 's/^Name=//p' "${file}" | head -n1)"
                    [[ -n "${label}" ]] || continue
                    labels+=("${label//,/ } (${id})")
                    ids+=("${id}")
                done
            done
            current="$(gsettings get "${schema}" "${key}")"
            selected=()
            for i in "${!ids[@]}"; do
                [[ "${current}" == *"'${ids[i]}'"* ]] && selected+=("${labels[i]}")
            done
            if [[ "${which}" == through ]]; then
                header="$(gettext "Which apps always come through while the digest is on? (space to pick, enter when done)")"
            else
                header="$(gettext "Which apps always wait for the digest? (space to pick, enter when done)")"
            fi
            if ! chosen="$(printf '%s\n' "${labels[@]}" | sort | gum choose --no-limit --header "${header}" \
                --selected "$(IFS=,; echo "${selected[*]}")")"; then
                echo "$(gettext "Nothing changed.")"
                exit 0
            fi
            while IFS= read -r line; do
                [[ -n "${line}" ]] || continue
                id="${line##*(}"
                apps+=("${id%)}")
            done <<<"${chosen}"
        fi
        for app in "${apps[@]}"; do
            [[ "${app}" =~ ^[A-Za-z0-9._+-]+\.desktop$ ]] || usage
        done
        list="$(printf '%s\n' "${apps[@]}" | awk 'NF && !seen[$0]++' | sed "s/.*/'&'/" | paste -sd, -)"
        gsettings set "${schema}" "${key}" "@as [${list}]"
        # Off the other list
        # A list as words: the IDs hold no space, quote or comma
        words() { gsettings get "${schema}" "$1" | sed -e 's/^@as //' -e "s/[][',]/ /g" | xargs -r -n1; }
        rest="$(words "${other}" | grep -vxF -f <(printf '%s\n' "${apps[@]}") | sed "s/.*/'&'/" | paste -sd, - || true)"
        gsettings set "${schema}" "${other}" "@as [${rest}]"
        shown="$(words "${key}" | paste -sd, - | sed 's/,/, /g')"
        [[ -n "${shown}" ]] || shown="$(gettext "none")"
        if [[ "${which}" == through ]]; then
            echo "$(eval_gettext "Always come through while the digest is on: \${shown}")"
        else
            echo "$(eval_gettext "Always wait for the digest: \${shown}")"
        fi
        exit 0
    fi
    [[ $# -le 1 ]] || { echo "usage: ame desktop digest [on|off]"; exit 64; }
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" digest)" == true ]]; then current=on; else current=off; fi
        state="$(gum choose --header "$(eval_gettext "Digest (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" digest true
            shown="$(times)"
            echo "$(eval_gettext "The digest is on: banners that are not urgent wait and come together at \${shown}.")"
            ;;
        off)
            gsettings set "${schema}" digest false
            echo "$(gettext "The digest is off: banners show as they come.")"
            ;;
        *)
            echo "usage: ame desktop digest [on|off]"
            exit 64
            ;;
    esac

# Later and mute on banners: Later brings a notification back in an hour, this evening, tomorrow morning or at the next
# unlock, and Mute holds an app's banners for an hour (ame desktop notifications later [on|off]); this evening's and
# tomorrow morning's times (ame desktop notifications later-times EVENING MORNING). Without on or off, it asks
[positional-arguments]
notifications *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pill
    usage() {
        echo "usage: ame desktop notifications later [on|off] | later-times HH:MM HH:MM"
        exit 64
    }
    case "${1:-}" in
        later-times)
            [[ $# -eq 3 ]] || usage
            for t in "$2" "$3"; do
                [[ "${t}" =~ ^([01][0-9]|2[0-3]):[0-5][0-9]$ ]] || usage
            done
            [[ "$2" != "$3" ]] || usage
            gsettings set "${schema}" later-times "['$2', '$3']"
            evening="$2" morning="$3"
            echo "$(eval_gettext "Later brings a notification back this evening at \${evening}, and tomorrow morning at \${morning}.")"
            ;;
        later)
            [[ $# -le 2 ]] || usage
            state="${2:-}"
            if [[ -z "${state}" ]]; then
                if [[ "$(gsettings get "${schema}" later)" == true ]]; then current=on; else current=off; fi
                state="$(gum choose --header "$(eval_gettext "Later and mute (current: \${current})")" --selected "${current}" on off)" || exit 0
            fi
            case "${state}" in
                on)
                    gsettings set "${schema}" later true
                    echo "$(gettext "Later and mute are on: a banner's Later brings it back when you choose, and Mute holds an app's banners for an hour. Nothing of them is written to disk.")"
                    ;;
                off)
                    gsettings set "${schema}" later false
                    echo "$(gettext "Later and mute are off: what waited is in the list.")"
                    ;;
                *) usage ;;
            esac
            ;;
        *) usage ;;
    esac

# What's new: the first session after an update, the pill says Updated and shows what the update brought (ame desktop
# whats-new [on|off]). Without on or off, it asks
[positional-arguments]
whats-new *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.pill
    [[ $# -le 1 ]] || { echo "usage: ame desktop whats-new [on|off]"; exit 64; }
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" whats-new)" == true ]]; then current=on; else current=off; fi
        state="$(gum choose --header "$(eval_gettext "What's new (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" whats-new true
            echo "$(gettext "After an update, the pill says Updated once and shows what it brought. The Manual's What's new page has it all.")"
            ;;
        off)
            gsettings set "${schema}" whats-new false
            echo "$(gettext "The pill says nothing after an update. The Manual's What's new page still has it.")"
            ;;
        *)
            echo "usage: ame desktop whats-new [on|off]"
            exit 64
            ;;
    esac

# Getting set: for an account's first four weeks, the calendar menu lists the steps that protect the machine and make it
# yours, each ticked by what the machine says (ame desktop getting-set [on|off]; Hide in the menu is off). Without on or
# off, it asks
[positional-arguments]
getting-set *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.today
    [[ $# -le 1 ]] || { echo "usage: ame desktop getting-set [on|off]"; exit 64; }
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" getting-set)" == true ]]; then current=on; else current=off; fi
        state="$(gum choose --header "$(eval_gettext "Getting set (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" getting-set true
            echo "$(gettext "Getting set is in the calendar menu for an account's first four weeks, while something on it is left to do.")"
            ;;
        off)
            gsettings set "${schema}" getting-set false
            echo "$(gettext "Getting set is hidden.")"
            ;;
        *)
            echo "usage: ame desktop getting-set [on|off]"
            exit 64
            ;;
    esac

# Power you can see: the battery's menu says when it runs out, the watts, what used the most and its health (ame desktop
# power battery [on|off]); a chip names an app that keeps the processor busy for two minutes (ame desktop power busy
# [on|off]); the same menu lists every device's battery, with a notice once at 15% (ame desktop power devices [on|off]).
# Without on or off, it asks
[positional-arguments]
power *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.power
    usage() {
        echo "usage: ame desktop power battery|busy|devices [on|off]"
        exit 64
    }
    [[ $# -ge 1 && $# -le 2 ]] || usage
    case "$1" in
        battery) key=battery-details ;;
        busy) key=busy-app ;;
        devices) key=device-batteries ;;
        *) usage ;;
    esac
    state="${2:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" "${key}")" == true ]]; then current=on; else current=off; fi
        if [[ "${key}" == battery-details ]]; then
            header="$(eval_gettext "Battery details (current: \${current})")"
        elif [[ "${key}" == device-batteries ]]; then
            header="$(eval_gettext "Your devices' batteries (current: \${current})")"
        else
            header="$(eval_gettext "Say when an app is busy (current: \${current})")"
        fi
        state="$(gum choose --header "${header}" --selected "${current}" on off)" || exit 0
    fi
    case "${state}" in
        on) gsettings set "${schema}" "${key}" true ;;
        off) gsettings set "${schema}" "${key}" false ;;
        *) usage ;;
    esac
    case "${key}:${state}" in
        battery-details:on) echo "$(gettext "The battery's menu says when it runs out or is full, the watts, the apps that used the most (an estimate from processor time) and its health.")" ;;
        battery-details:off) echo "$(gettext "The battery's menu no longer explains the battery.")" ;;
        busy-app:on) echo "$(gettext "When the processor has been busy for two minutes, a chip beside the meters names the app, once an hour at most.")" ;;
        busy-app:off) echo "$(gettext "No chip names a busy app.")" ;;
        device-batteries:on) echo "$(gettext "The battery's menu lists the battery of the mouse, the keyboard, the headphones and the rest, and says once when one is at 15%.")" ;;
        device-batteries:off) echo "$(gettext "The battery's menu no longer lists other devices' batteries.")" ;;
    esac

# The shelf: put files, text or a picture down while moving between apps, then drag them out (STATE: off, edge for the
# screen's right edge, shake for a shake of the pointer while dragging; without it, it says which)
[positional-arguments]
shelf STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.shelf
    case "${1:-}" in
        "")
            case "$(gsettings get "${schema}" shelf)" in
                "'edge'") echo "$(gettext "The shelf opens at the edge: drag something toward the right edge of the screen to put it down.")" ;;
                "'shake'") echo "$(gettext "The shelf opens where you shake the pointer while dragging.")" ;;
                *) echo "$(gettext "The shelf is off.")" ;;
            esac
            ;;
        off)
            gsettings set "${schema}" shelf off
            echo "$(gettext "The shelf: off. What it holds now stays until you drag it out or clear it.")"
            ;;
        edge)
            gsettings set "${schema}" shelf edge
            echo "$(gettext "The shelf: at the edge. Drag something toward the right edge of the screen and let go on the glow.")"
            ;;
        shake)
            gsettings set "${schema}" shelf shake
            echo "$(gettext "The shelf: shake the pointer while dragging, and let go on the card that opens.")"
            ;;
        *)
            echo "usage: ame desktop shelf off|edge|shake"
            exit 64
            ;;
    esac

# Desks: a set of screens brings back the sound output, the dock's place, a Focus profile and the power mode (ame
# desktop desks on|off, learn on|off, list, rename ID NAME..., saved ID NAME|none, forget ID, ask). Without anything it
# says whether they are on and lists them. The desks are in ~/.config/amethystora/desks.json, which the shell keeps.
[positional-arguments]
desks *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.desks
    folder="${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora"
    file="${folder}/desks.json"
    usage() {
        {
            echo "usage: ame desktop desks [on|off|list]"
            echo "       ame desktop desks learn on|off"
            echo "       ame desktop desks rename ID NAME..."
            echo "       ame desktop desks saved ID NAME|none"
            echo "       ame desktop desks forget ID"
            echo "       ame desktop desks ask"
        } >&2
        exit 64
    }
    # Text a desk's name or a saved set's may be: 1 to MOST characters, no control or format characters (a right-to-left
    # override would show it reversed), not only spaces
    plain() {
        [[ ${#1} -ge 1 && ${#1} -le $2 && -n "${1// /}" ]] && ! grep -qP '[\p{Cc}\p{Cf}]' <<<"$1"
    }
    # The file as the shell writes it: whole, through a file beside it, then the same text as the copy; one writer of
    # this recipe at a time (the shell reads the file again before it changes it)
    write() {
        local text="$1" tmp
        tmp="$(mktemp "${folder}/.desks.XXXXXX")"
        printf '%s' "${text}" >"${tmp}"
        chmod 600 "${tmp}"
        cp -p "${tmp}" "${tmp}.bak"
        mv -f "${tmp}" "${file}"
        mv -f "${tmp}.bak" "${file}.bak"
    }
    current() {
        if [[ -s "${file}" ]] && jq -e 'type == "object"' "${file}" >/dev/null 2>&1; then
            cat "${file}"
        elif [[ -s "${file}.bak" ]] && jq -e 'type == "object"' "${file}.bak" >/dev/null 2>&1; then
            cat "${file}.bak"
        else
            echo '{"version": 1, "desks": []}'
        fi
    }
    known() {
        [[ "$1" =~ ^[0-9a-f]{12}$ ]] && current | jq -e --arg id "$1" '[.desks[]? | select(.id == $id)] | length == 1' >/dev/null
    }
    list() {
        local count=0 id name screens first sound edge where focus power saved line
        # Apart by the unit separator, which is no white space: an empty field stays a field
        while IFS=$'\x1f' read -r id name screens first sound edge where focus power saved; do
            count=$((count + 1))
            if [[ -z "${name}" ]]; then
                case "${screens}" in
                    0) name="$(gettext "The laptop alone")" ;;
                    1) name="${first}" ;;
                    *) name="$(eval_ngettext "\${screens} screen" "\${screens} screens" "${screens}")" ;;
                esac
            fi
            line=()
            [[ -n "${sound}" ]] && line+=("$(eval_gettext "sound: \${sound}")")
            case "${edge}:${where}" in
                left:primary) line+=("$(gettext "the dock: on the left, on the main screen")") ;;
                left:*) line+=("$(gettext "the dock: on the left, on every screen")") ;;
                right:primary) line+=("$(gettext "the dock: on the right, on the main screen")") ;;
                right:*) line+=("$(gettext "the dock: on the right, on every screen")") ;;
                bottom:primary) line+=("$(gettext "the dock: at the bottom, on the main screen")") ;;
                bottom:*) line+=("$(gettext "the dock: at the bottom, on every screen")") ;;
            esac
            case "${focus}" in
                work) line+=("$(gettext "Focus: Work")") ;;
                games) line+=("$(gettext "Focus: Games")") ;;
                sleep) line+=("$(gettext "Focus: Sleep")") ;;
                off) line+=("$(gettext "Focus: off")") ;;
            esac
            case "${power}" in
                balanced) line+=("$(gettext "power: Balanced")") ;;
                performance) line+=("$(gettext "power: Performance")") ;;
                power-saver) line+=("$(gettext "power: Power saver")") ;;
            esac
            [[ -n "${saved}" ]] && line+=("$(eval_gettext "asks to open: \${saved}")")
            printf '%s  %s\n' "${id}" "${name}"
            if ((${#line[@]})); then
                printf '              %s\n' "$(IFS=';'; echo "${line[*]}" | sed 's/;/; /g')"
            fi
        done < <(current | jq -r '.desks[]? | [.id, (.name // ""), (.screens | length), (.screens[0].name // ""),
            (.set.sound.label // ""), (.set.dock.edge // ""), (.set.dock.screens // ""), (.set.focus // ""),
            (.set.power // ""), (.set.saved // "")] | map(tostring | gsub("[\\t\\n\\r\u001f]"; " ")) | join("\u001f")')
        ((count > 0)) || echo "$(gettext "No desk is set up yet: the first time the computer is at one, a card asks.")"
    }
    case "${1:-}" in
        rename | saved | forget | ask)
            mkdir -p "${folder}"
            chmod 700 "${folder}"
            exec 9>"${folder}/.desks.lock"
            flock 9
            ;;
    esac
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" desks)" == true ]]; then
                echo "$(gettext "Desks are on.")"
            else
                echo "$(gettext "Desks are off. ame desktop desks on turns them on.")"
            fi
            list
            ;;
        list)
            list
            ;;
        on)
            gsettings set "${schema}" desks true
            echo "$(gettext "Desks: on. The first time the computer is at a set of screens, a card asks whether to remember how things are set there.")"
            ;;
        off)
            gsettings set "${schema}" desks false
            echo "$(gettext "Desks: off. What a desk set stays as it is, and the desks are kept.")"
            ;;
        learn)
            case "${2:-}" in
                on) gsettings set "${schema}" desks-learn true
                    echo "$(gettext "At a desk, choosing another sound output or moving the dock asks whether the desk should remember it.")" ;;
                off) gsettings set "${schema}" desks-learn false
                    echo "$(gettext "Desks no longer ask to remember what you change.")" ;;
                *) usage ;;
            esac
            ;;
        rename)
            id="${2:-}"
            shift 2 2>/dev/null || usage
            name="$*"
            known "${id}" && plain "${name}" 40 || usage
            write "$(current | jq -c --arg id "${id}" --arg name "${name}" '(.desks[] | select(.id == $id) | .name) = $name')"
            echo "$(eval_gettext "The desk is called \${name} now.")"
            ;;
        saved)
            id="${2:-}"
            saved="${3:-}"
            known "${id}" && [[ $# -eq 3 ]] || usage
            if [[ "${saved}" == none ]]; then
                write "$(current | jq -c --arg id "${id}" '(.desks[] | select(.id == $id) | .set) |= del(.saved)')"
                echo "$(gettext "The desk offers no saved set.")"
            elif plain "${saved}" 24; then
                write "$(current | jq -c --arg id "${id}" --arg saved "${saved}" '(.desks[] | select(.id == $id) | .set.saved) = $saved')"
                echo "$(eval_gettext "Arriving at the desk asks whether to open the saved set \${saved}.")"
            else
                usage
            fi
            ;;
        forget)
            id="${2:-}"
            known "${id}" && [[ $# -eq 2 ]] || usage
            write "$(current | jq -c --arg id "${id}" 'del(.desks[] | select(.id == $id)) | if .current.desk == $id then del(.current) else . end')"
            echo "$(gettext "The desk is forgotten. What it set stays as it is.")"
            ;;
        ask)
            [[ $# -eq 1 ]] || usage
            write "$(current | jq -c 'del(.declined)')"
            echo "$(gettext "Desks you said Not now to will ask again the next time the computer is at them.")"
            ;;
        *)
            usage
            ;;
    esac

# Focus profiles: Work, Games or Sleep laid over your settings until it ends (ame desktop focus work|games|sleep
# [hour|tomorrow]; ame desktop focus off), the two triggers (ame desktop focus trigger games|sleep on|off) and what each
# profile does (ame desktop focus edit PROFILE SETTING VALUE..., or reset). Without anything it says what is on
[positional-arguments]
focus *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    SCHEMA=org.gnome.shell.extensions.amethystora-ui.focus
    usage() {
        {
            echo "usage: ame desktop focus [off|work|games|sleep [hour|tomorrow]]"
            echo "       ame desktop focus trigger games|sleep on|off"
            echo "       ame desktop focus edit work|games|sleep banners shown|apps|held"
            echo "       ame desktop focus edit work|games|sleep apps [APP.desktop...]"
            echo "       ame desktop focus edit work|games|sleep sounds|lighter|grey on|off"
            echo "       ame desktop focus edit work|games|sleep dock as-set|touch"
            echo "       ame desktop focus edit work|games|sleep night-light as-set|paused|warmest"
            echo "       ame desktop focus edit work|games|sleep power as-set|performance|power-saver"
            echo "       ame desktop focus edit work|games|sleep accent theme|ACCENT"
            echo "       ame desktop focus edit work|games|sleep wallpaper as-set|night"
            echo "       ame desktop focus edit work|games|sleep set NAME|none"
            echo "       ame desktop focus edit work|games|sleep reset"
        } >&2
        exit 64
    }
    get() { gsettings get "${SCHEMA}" "$1" | sed "s/^'\(.*\)'$/\1/; s/^int64 //"; }
    now() { date +%s; }
    # The next 06:00, as the Focus tile's "tomorrow"
    morning() {
        local t
        t="$(date -d 'today 06:00' +%s)"
        ((t > $(now))) || t="$(date -d 'tomorrow 06:00' +%s)"
        echo "${t}"
    }
    name() {
        case "$1" in
            work) gettext "Work" ;;
            games) gettext "Games" ;;
            sleep) gettext "Sleep" ;;
        esac
    }
    ACCENTS=(amethyst iris sky lagoon jade moss amber ember coral rose orchid slate)
    # Launchers as GNOME finds them, the account's first
    LAUNCHER_DIRS=("${XDG_DATA_HOME:-${HOME}/.local/share}/applications" "${HOME}/.local/share/flatpak/exports/share/applications"
        /var/lib/flatpak/exports/share/applications /usr/local/share/applications /usr/share/applications)
    launcher_file() {
        local dir
        for dir in "${LAUNCHER_DIRS[@]}"; do
            if [[ -f "${dir}/$1" ]]; then echo "${dir}/$1"; return 0; fi
        done
        return 1
    }
    one_of() {
        local value="$1"
        shift
        [[ -n "${value}" && "${value}" != *'|'* ]] || return 1
        local IFS='|'
        [[ "|$*|" == *"|${value}|"* ]]
    }
    case "${1:-}" in
        "")
            profile="$(get profile)"
            if [[ "${profile}" == off ]]; then
                echo "$(gettext "No profile is picked; a trigger may turn one on (the Focus tile in quick settings says).")"
            else
                shown="$(name "${profile}")"
                ends="$(get ends)"
                if [[ "${ends}" == 0 ]]; then
                    echo "$(eval_gettext "Focus: \${shown}, until you turn it off.")"
                else
                    when="$(date -d "@${ends}" '+%a %H:%M')"
                    echo "$(eval_gettext "Focus: \${shown}, until \${when}.")"
                fi
            fi
            for trigger in games sleep; do
                if [[ "$(get "trigger-${trigger}")" == true ]]; then state=on; else state=off; fi
                case "${trigger}" in
                    games) echo "$(eval_gettext "Games while a game is in front: \${state}")" ;;
                    sleep) echo "$(eval_gettext "Sleep at Night Light's times: \${state}")" ;;
                esac
            done
            ;;
        off)
            [[ $# -eq 1 ]] || usage
            # Off first, so that a trigger on now waits for its next start
            gsettings set "${SCHEMA}" off-at "$(now)"
            gsettings set "${SCHEMA}" ends 0
            gsettings set "${SCHEMA}" profile off
            echo "$(gettext "Focus is off: notifications, sounds and the desktop are as you set them.")"
            ;;
        work | games | sleep)
            [[ $# -le 2 ]] || usage
            case "${2:-}" in
                "") ends=0 ;;
                hour) ends=$(($(now) + 3600)) ;;
                tomorrow) ends="$(morning)" ;;
                *) usage ;;
            esac
            gsettings set "${SCHEMA}" ends "${ends}"
            gsettings set "${SCHEMA}" profile "$1"
            shown="$(name "$1")"
            if [[ "${ends}" == 0 ]]; then
                echo "$(eval_gettext "\${shown} is on until you turn it off (ame desktop focus off).")"
            else
                when="$(date -d "@${ends}" '+%a %H:%M')"
                echo "$(eval_gettext "\${shown} is on until \${when}.")"
            fi
            ;;
        trigger)
            [[ $# -eq 3 ]] && one_of "$2" games sleep && one_of "$3" on off || usage
            if [[ "$3" == on ]]; then gsettings set "${SCHEMA}" "trigger-$2" true; else gsettings set "${SCHEMA}" "trigger-$2" false; fi
            case "$2:$3" in
                games:on) echo "$(gettext "Games turns on when a game gets the keyboard, and ends when that game closes.")" ;;
                sleep:on) echo "$(gettext "Sleep turns on at Night Light's evening time and ends at its morning time.")" ;;
                games:off) echo "$(gettext "Games turns on only when you pick it.")" ;;
                sleep:off) echo "$(gettext "Sleep turns on only when you pick it.")" ;;
            esac
            ;;
        edit)
            [[ $# -ge 3 ]] && one_of "$2" work games sleep || usage
            profile="$2" field="$3"
            shift 3
            key="${profile}-${field}"
            case "${field}" in
                reset)
                    [[ $# -eq 0 ]] || usage
                    for each in banners apps sounds dock lighter night-light power accent wallpaper grey set; do
                        gsettings reset "${SCHEMA}" "${profile}-${each}"
                    done
                    shown="$(name "${profile}")"
                    echo "$(eval_gettext "\${shown} is as the image ships it again.")"
                    exit 0
                    ;;
                banners) [[ $# -eq 1 ]] && one_of "$1" shown apps held || usage ;;
                dock) [[ $# -eq 1 ]] && one_of "$1" as-set touch || usage ;;
                night-light) [[ $# -eq 1 ]] && one_of "$1" as-set paused warmest || usage ;;
                power) [[ $# -eq 1 ]] && one_of "$1" as-set performance power-saver || usage ;;
                accent) [[ $# -eq 1 ]] && one_of "$1" theme "${ACCENTS[@]}" || usage ;;
                wallpaper) [[ $# -eq 1 ]] && one_of "$1" as-set night || usage ;;
                sounds | lighter | grey)
                    [[ $# -eq 1 ]] && one_of "$1" on off || usage
                    value="$1"
                    if [[ "${value}" == on ]]; then gsettings set "${SCHEMA}" "${key}" true; else gsettings set "${SCHEMA}" "${key}" false; fi
                    profile="$(name "${profile}")"
                    echo "$(eval_gettext "\${profile} \${field}: \${value}")"
                    exit 0
                    ;;
                set)
                    [[ $# -eq 1 ]] || usage
                    value="$1"
                    [[ "${value}" == none ]] && value=""
                    # A saved set's name: some text, at most 24 characters, no control character
                    if ((${#value} > 24)) || [[ "${value}" =~ [[:cntrl:]] ]]; then usage; fi
                    quoted="${value//\\/\\\\}"
                    quoted="${quoted//\'/\\\'}"
                    gsettings set "${SCHEMA}" "${key}" "'${quoted}'"
                    profile="$(name "${profile}")"
                    echo "$(eval_gettext "\${profile} set: \${value}")"
                    exit 0
                    ;;
                apps)
                    apps=("$@")
                    if ((${#apps[@]} == 0)); then
                        # In a terminal: pick from the launchers a person sees
                        [[ -t 0 ]] || usage
                        declare -A seen=()
                        labels=() ids=()
                        for dir in "${LAUNCHER_DIRS[@]}"; do
                            for file in "${dir}"/*.desktop; do
                                [[ -f "${file}" ]] || continue
                                id="${file##*/}"
                                [[ -z "${seen[${id}]:-}" ]] || continue
                                seen[${id}]=1
                                grep -qx 'NoDisplay=true' "${file}" && continue
                                label="$(sed -n 's/^Name=//p' "${file}" | head -n1)"
                                [[ -n "${label}" ]] || continue
                                labels+=("${label} (${id})")
                                ids+=("${id}")
                            done
                        done
                        current="$(gsettings get "${SCHEMA}" "${key}")"
                        selected=()
                        for i in "${!ids[@]}"; do
                            [[ "${current}" == *"'${ids[i]}'"* ]] && selected+=("${labels[i]}")
                        done
                        # Escape or Ctrl+C changes nothing; Enter with nothing picked is a real "none"
                        if ! chosen="$(printf '%s\n' "${labels[@]}" | sort | gum choose --no-limit \
                            --header "$(gettext "Whose banners come through? (space to pick, enter when done)")" \
                            --selected "$(IFS=,; echo "${selected[*]}")")"; then
                            echo "$(gettext "Nothing changed.")"
                            exit 0
                        fi
                        picked=()
                        [[ -z "${chosen}" ]] || mapfile -t picked <<<"${chosen}"
                        for label in "${picked[@]}"; do
                            id="${label##*(}"
                            apps+=("${id%)}")
                        done
                        # The calendar's reminders have no launcher of their own to pick: they stay
                        [[ "${current}" == *"'org.gnome.Evolution-alarm-notify.desktop'"* ]] && apps+=(org.gnome.Evolution-alarm-notify.desktop)
                    fi
                    list="" names=""
                    for app in "${apps[@]}"; do
                        [[ "${app}" =~ ^[A-Za-z0-9._+-]+\.desktop$ ]] || usage
                        if ! launcher_file "${app}" >/dev/null && [[ "${app}" != org.gnome.Evolution-alarm-notify.desktop ]]; then
                            echo "$(eval_gettext "No launcher \${app} is installed.")" >&2
                            exit 1
                        fi
                        [[ "${list}" != *"'${app}'"* ]] || continue
                        list+="${list:+, }'${app}'"
                        # Said by the name the app grid shows
                        label=""
                        file="$(launcher_file "${app}")" && label="$(sed -n 's/^Name=//p' "${file}" | head -n1)"
                        names+="${names:+, }${label:-${app}}"
                    done
                    gsettings set "${SCHEMA}" "${key}" "@as [${list}]"
                    shown="$(name "${profile}")"
                    if [[ -z "${names}" ]]; then
                        echo "$(eval_gettext "\${shown} lets through no app's banners.")"
                    else
                        list="${names}"
                        echo "$(eval_gettext "\${shown} lets through banners from: \${list}")"
                    fi
                    exit 0
                    ;;
                *) usage ;;
            esac
            value="$1"
            gsettings set "${SCHEMA}" "${key}" "${value}"
            profile="$(name "${profile}")"
            echo "$(eval_gettext "\${profile} \${field}: \${value}")"
            ;;
        *) usage ;;
    esac

# The top bar on every screen (ame desktop top-bar screens [on|off]), and over a fullscreen window when you push the
# pointer against the top edge (ame desktop top-bar peek [on|off]); its size (size compact|regular|roomy), its edge
# (position top|bottom), hidden until touched (autohide on|off), what it holds where (item ITEM ZONE [POSITION], layout
# show|reset), and each meter on or off (meter processor|memory|network [on|off]). Without on or off, it asks
[positional-arguments]
top-bar *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.bars
    usage() {
        echo "usage: ame desktop top-bar screens|peek|autohide [on|off]"
        echo "       ame desktop top-bar size compact|regular|roomy"
        echo "       ame desktop top-bar position top|bottom"
        echo "       ame desktop top-bar item ITEM left|center|right|hidden [POSITION]"
        echo "       ame desktop top-bar layout show|reset"
        echo "       ame desktop top-bar meter processor|memory|network [on|off]"
        exit 64
    }
    [[ $# -ge 1 ]] || usage
    case "$1" in
        meter)
            [[ $# -ge 2 && $# -le 3 ]] || usage
            case "$2" in
                processor | memory | network) ;;
                *) usage ;;
            esac
            state="${3:-}"
            if [[ -z "${state}" ]]; then
                if /usr/libexec/amethystora-bar-layout show | grep -qE "^hidden:.* $2( |\$)"; then current=off; else current=on; fi
                what="$2"
                state="$(gum choose --header "$(eval_gettext "Top bar \${what} (current: \${current})")" --selected "${current}" on off)" || exit 0
            fi
            case "$2:${state}" in
                processor:on) echo "$(gettext "The processor meter is on the top bar.")" ;;
                processor:off) echo "$(gettext "The processor meter is off, and nothing is read for it.")" ;;
                memory:on) echo "$(gettext "The memory meter is on the top bar.")" ;;
                memory:off) echo "$(gettext "The memory meter is off, and nothing is read for it.")" ;;
                network:on) echo "$(gettext "The network meter is on the top bar.")" ;;
                network:off) echo "$(gettext "The network meter is off, and nothing is read for it.")" ;;
                *) usage ;;
            esac
            if [[ "${state}" == on ]]; then
                /usr/libexec/amethystora-bar-layout unhide "$2"
            else
                /usr/libexec/amethystora-bar-layout place "$2" hidden
            fi
            exit 0
            ;;
        size)
            [[ $# -eq 2 ]] || usage
            case "$2" in
                compact) echo "$(gettext "The top bar is compact: 28 pixels high, with everything on it smaller.")" ;;
                regular) echo "$(gettext "The top bar is its regular size, 32 pixels high.")" ;;
                roomy) echo "$(gettext "The top bar is roomy: 40 pixels high, with larger targets.")" ;;
                *) usage ;;
            esac
            gsettings set "${schema}" size "$2"
            exit 0
            ;;
        position)
            [[ $# -eq 2 ]] || usage
            case "$2" in
                top) echo "$(gettext "The top bar is at the top of the screen.")" ;;
                bottom) echo "$(gettext "The bar is at the bottom of the screen; its menus open upwards and a dock at the bottom sits above it.")" ;;
                *) usage ;;
            esac
            gsettings set "${schema}" position "$2"
            exit 0
            ;;
        item)
            [[ $# -ge 3 && $# -le 4 ]] || usage
            /usr/libexec/amethystora-bar-layout place "$2" "$3" "${4:-}"
            exit 0
            ;;
        layout)
            [[ $# -eq 2 ]] || usage
            case "$2" in
                show) /usr/libexec/amethystora-bar-layout show ;;
                reset)
                    /usr/libexec/amethystora-bar-layout reset
                    echo "$(gettext "The top bar holds what the image puts on it, where it puts it.")"
                    ;;
                *) usage ;;
            esac
            exit 0
            ;;
        screens) key=every-screen ;;
        peek) key=peek ;;
        autohide) key=autohide ;;
        *) usage ;;
    esac
    [[ $# -le 2 ]] || usage
    state="${2:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" "${key}")" == true ]]; then current=on; else current=off; fi
        what="$1"
        state="$(gum choose --header "$(eval_gettext "Top bar \${what} (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    case "$1:${state}" in
        screens:on) echo "$(gettext "Every screen has a top bar with the workspace it shows.")" ;;
        screens:off) echo "$(gettext "Only the main screen has a top bar.")" ;;
        peek:on) echo "$(gettext "Push the pointer against the top edge to see the top bar over a fullscreen window.")" ;;
        peek:off) echo "$(gettext "A fullscreen window keeps the whole screen.")" ;;
        autohide:on) echo "$(gettext "The bar hides until you push the pointer against its edge, and gives its room to the windows.")" ;;
        autohide:off) echo "$(gettext "The bar stays in view.")" ;;
        *) usage ;;
    esac
    if [[ "${state}" == on ]]; then gsettings set "${schema}" "${key}" true; else gsettings set "${schema}" "${key}" false; fi

# Today's tasks from Notes in the calendar menu, while Notes is open and unlocked (ame desktop today tasks [on|off]).
# Without on or off, it asks
[positional-arguments]
today *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.today
    usage() {
        echo "usage: ame desktop today tasks [on|off]"
        exit 64
    }
    [[ $# -ge 1 && $# -le 2 && "$1" == tasks ]] || usage
    state="${2:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" tasks)" == true ]]; then current=on; else current=off; fi
        state="$(gum choose --header "$(eval_gettext "Today's tasks (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    case "${state}" in
        on) echo "$(gettext "The calendar menu lists today's tasks while Notes is open and unlocked.")" ;;
        off) echo "$(gettext "The calendar menu shows no tasks.")" ;;
        *) usage ;;
    esac
    if [[ "${state}" == on ]]; then gsettings set "${schema}" tasks true; else gsettings set "${schema}" tasks false; fi

# The weather beside the clock and in the calendar menu, for the place you chose in Weather. It sends that place to a
# weather service, so it is off until you turn it on (ame desktop weather [on|off]). Without on or off, it asks
[positional-arguments]
weather *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.today
    usage() {
        echo "usage: ame desktop weather [on|off]"
        exit 64
    }
    [[ $# -le 1 ]] || usage
    state="${1:-}"
    # What it sends, said once before it can be turned on: before the question, or before turning it on straight away
    if [[ -z "${state}" || "${state}" == on ]]; then
        echo "$(gettext "The place you chose in Weather goes to the weather services GNOME uses, each time the weather is shown.")"
    fi
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" weather)" == true ]]; then current=on; else current=off; fi
        state="$(gum choose --header "$(eval_gettext "Weather (current: \${current})")" --selected "${current}" on off)" || exit 0
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" weather true
            echo "$(gettext "The weather is on.")"
            if [[ "$(gsettings get org.gnome.shell.weather locations 2>/dev/null)" == "@av []" ]]; then
                # Weather is a Flatpak (org.gnome.Weather): opened by its launcher
                if gtk-launch org.gnome.Weather >/dev/null 2>&1; then
                    echo "$(gettext "Choose a place in Weather, which opens now.")"
                else
                    echo "$(gettext "Choose a place in Weather, from the app grid.")"
                fi
            fi
            ;;
        off)
            gsettings set "${schema}" weather false
            echo "$(gettext "The weather is off: nothing is sent for it.")"
            ;;
        *) usage ;;
    esac

# Rest reminders, GNOME's own: look away (on or eyesight), stand up (movement), both, or off. The times are in
# Settings, Wellbeing (ame desktop breaks [on|eyesight|movement|both|off]). Without a value, it asks
[positional-arguments]
breaks *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.desktop.break-reminders
    usage() {
        echo "usage: ame desktop breaks on|eyesight|movement|both|off"
        exit 64
    }
    [[ $# -le 1 ]] || usage
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        case "$(gsettings get "${schema}" selected-breaks)" in
            *eyesight*movement* | *movement*eyesight*) current=both ;;
            *eyesight*) current=eyesight ;;
            *movement*) current=movement ;;
            *) current=off ;;
        esac
        state="$(gum choose --header "$(eval_gettext "Rest reminders (current: \${current})")" --selected "${current}" eyesight movement both off)" || exit 0
    fi
    # GNOME's times for one kind of break, as the account has them, in seconds: between breaks, and a break's length
    seconds_of() { gsettings get "${schema}.$1" "$2" | awk '{print $NF}'; }
    # How long a break lasts, in whole minutes where it is some, else in seconds: a sentence of its own
    lasts() {
        local seconds="$1" minutes=$(($1 / 60))
        if ((seconds >= 60 && seconds % 60 == 0)); then
            echo "$(eval_ngettext "Each break lasts \${minutes} minute." "Each break lasts \${minutes} minutes." "${minutes}")"
        else
            echo "$(eval_ngettext "Each break lasts \${seconds} second." "Each break lasts \${seconds} seconds." "${seconds}")"
        fi
    }
    case "${state}" in
        on | eyesight)
            gsettings set "${schema}" selected-breaks "['eyesight']"
            minutes=$(($(seconds_of eyesight interval-seconds) / 60))
            echo "$(eval_ngettext "A reminder to look away from the screen comes every \${minutes} minute." "A reminder to look away from the screen comes every \${minutes} minutes." "${minutes}")"
            lasts "$(seconds_of eyesight duration-seconds)"
            echo "$(gettext "Change the times in Settings, Wellbeing.")"
            ;;
        movement)
            gsettings set "${schema}" selected-breaks "['movement']"
            minutes=$(($(seconds_of movement interval-seconds) / 60))
            echo "$(eval_ngettext "A reminder to stand up and move comes every \${minutes} minute." "A reminder to stand up and move comes every \${minutes} minutes." "${minutes}")"
            lasts "$(seconds_of movement duration-seconds)"
            echo "$(gettext "Change the times in Settings, Wellbeing.")"
            ;;
        both)
            gsettings set "${schema}" selected-breaks "['eyesight', 'movement']"
            echo "$(gettext "Both reminders are on. Change the times in Settings, Wellbeing.")"
            ;;
        off)
            gsettings set "${schema}" selected-breaks "[]"
            echo "$(gettext "Rest reminders are off.")"
            ;;
        *) usage ;;
    esac

# The dock: how it grows and starts apps, what marks a running app, what it holds and where it lives
# (ame desktop dock SETTING VALUE; without a value it asks; without anything it shows them all)
[positional-arguments]
dock *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    SCHEMA=org.gnome.shell.extensions.amethystora-ui.dock
    get() { gsettings get "${SCHEMA}" "$1" | tr -d "'"; }
    onoff() { if [[ "$(get "$1")" == true ]]; then echo on; else echo off; fi; }
    percent() { awk -v a="$(get magnify-amount)" 'BEGIN { printf "%.0f", a * 100 }'; }
    # on or off for a word that may be missing, which flips the current state
    flip() { if [[ -n "$1" ]]; then echo "$1"; elif [[ "$2" == on ]]; then echo off; else echo on; fi; }
    # A value from a fixed list: the one given, or one chosen; anything else is refused with the setting's usage
    pick() {
        local setting="$1" current="$2" value="$3"
        shift 3
        if [[ -z "${value}" ]]; then
            value="$(gum choose --header "$(eval_gettext "\${setting} (current: \${current})")" --selected "${current}" "$@")" || exit 0
        fi
        local IFS='|'
        if [[ "|$*|" != *"|${value}|"* ]]; then
            echo "usage: ame desktop dock ${setting} $*" >&2
            exit 64
        fi
        echo "${value}"
    }
    # The folders and stacks are lists, read and written in python, which answers with one word
    lists() {
        python3 -I - "$@" <<'PY'
    import ast, json, os, re, subprocess, sys, unicodedata
    SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.dock'
    XDG = ['DESKTOP', 'DOCUMENTS', 'DOWNLOAD', 'MUSIC', 'PICTURES', 'PUBLICSHARE', 'TEMPLATES', 'VIDEOS']
    APP = re.compile(r'^[A-Za-z0-9._+-]+\.desktop$')
    # A name as the dock takes one: some text, at most 40 characters, no control or format character
    def good_name(name):
        return bool(name.strip()) and len(name) <= 40 and not any(unicodedata.category(c) in ("Cc", "Cf") for c in name)
    def read(schema, key):
        text = subprocess.run(['gsettings', 'get', schema, key], capture_output=True, text=True, check=True).stdout
        return ast.literal_eval(re.sub(r'^@[a-z{}]+ ', '', text.strip()))
    def write(key, value, kind):
        empty = '{}' if kind == 'a{sas}' else '[]'
        text = json.dumps(value, ensure_ascii=True) if value else f'@{kind} {empty}'
        subprocess.run(['gsettings', 'set', SCHEMA, key, text], check=True)
    def special(name):
        out = subprocess.run(['xdg-user-dir', name], capture_output=True, text=True).stdout.strip()
        return out if out and out != os.path.expanduser('~') else None
    def resolve(value):
        if value.startswith('xdg:'):
            return special(value[4:]) if value[4:] in XDG else None
        if value.startswith('~/'):
            return None if '..' in value.split('/') else os.path.join(os.path.expanduser('~'), value[2:])
        return value if value.startswith('/') else None
    def launchers():
        home = os.environ.get('XDG_DATA_HOME') or os.path.expanduser('~/.local/share')
        dirs = [home] + (os.environ.get('XDG_DATA_DIRS') or '/usr/local/share:/usr/share').split(':')
        return [os.path.join(d, 'applications') for d in dirs if d]
    what, *rest = sys.argv[1:]
    if what == 'stack-list':
        for value in read(SCHEMA, 'stacks'):
            print(value)
    elif what in ('stack-add', 'stack-remove'):
        path = os.path.realpath(os.path.expanduser(rest[0]))
        try:
            path.encode("utf-8")
        except UnicodeEncodeError:
            print("not-text")
            sys.exit(0)
        if what == 'stack-add' and not os.path.isdir(path):
            print('not-a-folder')
            sys.exit(0)
        home = os.path.expanduser('~')
        value = next((f'xdg:{name}' for name in XDG if special(name) == path), None)
        if value is None:
            value = '~/' + os.path.relpath(path, home) if path.startswith(home + '/') else path
        stacks = [v for v in read(SCHEMA, 'stacks') if resolve(v) != path]
        if what == 'stack-add':
            if len(stacks) >= 12:
                print('full')
                sys.exit(0)
            stacks.append(value)
        write('stacks', stacks, 'as')
        print('ok')
    elif what == 'folder-list':
        for name, ids in read(SCHEMA, 'folders').items():
            print(name + ': ' + ' '.join(ids))
    elif what == 'folder':
        name, ids = rest[0], rest[1:]
        folders = read(SCHEMA, 'folders')
        if not good_name(name):
            print('bad-name')
            sys.exit(0)
        if not ids:
            folders.pop(name, None)
            write('folders', folders, 'a{sas}')
            print('apart')
            sys.exit(0)
        if name in folders:
            print('taken')
            sys.exit(0)
        pinned = read('org.gnome.shell', 'favorite-apps')
        dirs = launchers()
        for app in ids:
            if not APP.match(app) or app not in pinned or not any(os.path.isfile(os.path.join(d, app)) for d in dirs):
                print('bad-app ' + app)
                sys.exit(0)
        ids = list(dict.fromkeys(ids))
        if len(ids) < 2:
            print('too-few')
            sys.exit(0)
        # An app is in one folder at most; a folder left with one app is taken apart
        others = {n: [a for a in v if a not in ids] for n, v in folders.items() if n != name}
        folders = {n: v for n, v in others.items() if len(v) >= 2}
        folders[name] = ids
        write('folders', folders, 'a{sas}')
        print('ok')
    PY
    }
    say() {
        case "$1" in
            magnify)
                case "$(get magnify)" in
                    swell)
                        amount="$(percent)%"
                        echo "$(eval_gettext "The dock swells under the pointer, up to \${amount}.")" ;;
                    lift) echo "$(gettext "The dock lifts the icon under the pointer.")" ;;
                    *) echo "$(gettext "The dock stays still.")" ;;
                esac ;;
            launch)
                case "$(get launch)" in
                    halo) echo "$(gettext "Apps starting show a ring.")" ;;
                    hop) echo "$(gettext "Apps starting hop.")" ;;
                    breathe) echo "$(gettext "Apps starting breathe.")" ;;
                    *) echo "$(gettext "Apps start without a sign.")" ;;
                esac ;;
            indicator)
                case "$(get indicator)" in
                    bar) echo "$(gettext "A running app is marked with a bar.")" ;;
                    dot) echo "$(gettext "A running app is marked with a dot for each window.")" ;;
                    *) echo "$(gettext "A running app is marked with a glow.")" ;;
                esac ;;
            recent)
                if [[ "$(onoff recent)" == on ]]; then
                    echo "$(gettext "Recent apps are shown after the running ones.")"
                else
                    echo "$(gettext "Recent apps are not shown.")"
                fi ;;
            previews)
                if [[ "$(onoff previews)" == on ]]; then
                    echo "$(gettext "Resting on an app shows its windows.")"
                else
                    echo "$(gettext "Resting on an app shows its name only.")"
                fi ;;
            edge)
                case "$(get edge)" in
                    left) edge="$(gettext "left")" ;;
                    right) edge="$(gettext "right")" ;;
                    *) edge="$(gettext "bottom")" ;;
                esac
                echo "$(eval_gettext "The dock is at the \${edge} of the screen.")" ;;
            screens)
                if [[ "$(get screens)" == every ]]; then
                    echo "$(gettext "There is a dock on every screen.")"
                else
                    echo "$(gettext "There is a dock on the main screen only.")"
                fi ;;
            autohide)
                if [[ "$(get autohide)" == touch ]]; then
                    echo "$(gettext "The dock hides while a window touches it; push the pointer against the edge to bring it back.")"
                else
                    echo "$(gettext "The dock is always shown.")"
                fi ;;
            workspace-apps)
                if [[ "$(onoff workspace-apps)" == on ]]; then
                    echo "$(gettext "The dock shows only the apps of the workspace you are on, and the pinned ones.")"
                else
                    echo "$(gettext "The dock shows every running app.")"
                fi ;;
            updating)
                if [[ "$(onoff updating)" == on ]]; then
                    echo "$(gettext "An app's icon goes grey while Flatpak updates it.")"
                else
                    echo "$(gettext "An app's icon stays as it is while Flatpak updates it.")"
                fi ;;
        esac
    }
    setting="${1:-}"
    value="${2:-}"
    case "${setting}" in
        "")
            echo "magnify $(get magnify)"
            echo "amount $(percent)"
            echo "launch $(get launch)"
            echo "indicator $(get indicator)"
            echo "recent $(onoff recent)"
            echo "previews $(onoff previews)"
            echo "edge $(get edge)"
            echo "screens $(get screens)"
            echo "autohide $(get autohide)"
            echo "workspace-apps $(onoff workspace-apps)"
            echo "updating $(onoff updating)"
            echo "folders $(gsettings get "${SCHEMA}" folders)"
            echo "stacks $(gsettings get "${SCHEMA}" stacks)"
            echo "size $(gsettings get "${SCHEMA}" icon-size)"
            ;;
        # The icons' size at rest (R6-28), 40 to 64 px; reset gives the dock GNOME's own choice again
        size)
            if [[ -z "${value}" ]]; then
                value="$(gum input --header "$(gettext "The dock's icon size, 40 to 64 pixels")" \
                    --value "$(gsettings get "${SCHEMA}" icon-size)")" || exit 0
            fi
            if [[ "${value}" == reset ]]; then
                gsettings reset "${SCHEMA}" icon-size
                echo "$(gettext "The dock's icons are the size it picks itself again.")"
                exit 0
            fi
            if [[ ! "${value}" =~ ^[0-9]{1,3}$ ]] || ((10#${value} < 40 || 10#${value} > 64)); then
                echo "usage: ame desktop dock size 40..64|reset" >&2
                exit 64
            fi
            value="$((10#${value}))"
            gsettings set "${SCHEMA}" icon-size "${value}"
            echo "$(eval_gettext "The dock's icons are \${value} pixels.")"
            ;;
        magnify | launch | indicator | edge | screens | autohide)
            case "${setting}" in
                magnify) values=(swell lift off) ;;
                launch) values=(halo hop breathe off) ;;
                indicator) values=(bar dot glow) ;;
                edge) values=(bottom left right) ;;
                screens) values=(every primary) ;;
                autohide) values=(off touch) ;;
            esac
            value="$(pick "${setting}" "$(get "${setting}")" "${value}" "${values[@]}")"
            gsettings set "${SCHEMA}" "${setting}" "${value}"
            say "${setting}"
            ;;
        amount)
            if [[ -z "${value}" ]]; then
                value="$(gum input --header "$(gettext "How far Swell grows, 120 to 220 per cent")" --value "$(percent)")" || exit 0
            fi
            # Per cent (120 to 220), or the multiple itself (1.2 to 2.2)
            amount="$(awk -v v="${value}" 'BEGIN {
                if (v ~ /^[0-9]+$/ && v + 0 >= 120 && v + 0 <= 220) printf "%.2f", v / 100
                else if (v ~ /^[12]\.[0-9]+$/ && v + 0 >= 1.2 && v + 0 <= 2.2) printf "%.2f", v
            }')"
            if [[ -z "${amount}" ]]; then
                echo "usage: ame desktop dock amount 120..220" >&2
                exit 64
            fi
            gsettings set "${SCHEMA}" magnify-amount "${amount}"
            say magnify
            ;;
        recent | previews | workspace-apps | updating)
            case "$(flip "${value}" "$(onoff "${setting}")")" in
                on) gsettings set "${SCHEMA}" "${setting}" true ;;
                off) gsettings set "${SCHEMA}" "${setting}" false ;;
                *)
                    echo "usage: ame desktop dock ${setting} on|off" >&2
                    exit 64
                    ;;
            esac
            say "${setting}"
            ;;
        stack)
            path="${3:-}"
            case "${value}" in
                list) lists stack-list ;;
                add | remove)
                    if [[ -z "${path}" ]]; then
                        echo "usage: ame desktop dock stack add|remove PATH" >&2
                        exit 64
                    fi
                    case "$(lists "stack-${value}" "${path}")" in
                        ok)
                            if [[ "${value}" == add ]]; then
                                echo "$(eval_gettext "\${path} is in the dock.")"
                            else
                                echo "$(eval_gettext "\${path} is no longer in the dock.")"
                            fi ;;
                        not-text)
                            echo "$(eval_gettext "\${path} cannot be kept in the dock: its name is not text.")" >&2
                            exit 1 ;;
                        not-a-folder)
                            echo "$(eval_gettext "\${path} is not a folder.")" >&2
                            exit 1 ;;
                        full)
                            echo "$(gettext "The dock holds twelve stacks at most.")" >&2
                            exit 1 ;;
                    esac
                    ;;
                *)
                    echo "usage: ame desktop dock stack add|remove|list [PATH]" >&2
                    exit 64
                    ;;
            esac
            ;;
        folder)
            name="${value}"
            if [[ -z "${name}" ]]; then
                echo "usage: ame desktop dock folder NAME [APP.desktop APP.desktop...] | folder list" >&2
                exit 64
            fi
            if [[ "${name}" == list && $# -eq 2 ]]; then
                lists folder-list
                exit 0
            fi
            shift 2
            result="$(lists folder "${name}" "$@")"
            case "${result}" in
                ok)
                    apps="$*"
                    echo "$(eval_gettext "The folder \${name} holds \${apps}.")" ;;
                apart) echo "$(eval_gettext "The folder \${name} is taken apart.")" ;;
                taken)
                    echo "$(eval_gettext "There is a folder called \${name} already.")" >&2
                    exit 64 ;;
                bad-name)
                    echo "$(gettext "A folder's name is 1 to 40 characters, without control characters.")" >&2
                    exit 64 ;;
                too-few)
                    echo "$(gettext "A folder holds two pinned apps or more.")" >&2
                    exit 64 ;;
                bad-app*)
                    app="${result#bad-app }"
                    echo "$(eval_gettext "\${app} is not a pinned app.")" >&2
                    exit 64 ;;
            esac
            ;;
        *)
            echo "usage: ame desktop dock [magnify|amount|launch|indicator|recent|previews|edge|screens|autohide|workspace-apps|updating|stack|folder] [VALUE]" >&2
            exit 64
            ;;
    esac

# What the search (Super+Space, and the overview's) finds besides GNOME's own: sums and units, the time in other places,
# reminders into Notes, Control's settings, the manual, open windows, the clipboard after clip:, the agent row, Glance
# on a file, the words people already know, files by what is remembered of them, and, offered and off, Firefox's
# bookmarks, Notes while it is unlocked and files by meaning (WHAT: answers, times, reminders, settings, manual, windows,
# clipboard, agent, glance, old-words, remembered, bookmarks, notes or meaning; STATE: on or off).
# Without STATE it says what WHAT is; without WHAT, all fourteen
[positional-arguments]
search WHAT="" STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.search
    parts=(answers times reminders settings manual windows clipboard agent glance old-words remembered bookmarks notes meaning)
    usage() {
        echo "usage: ame desktop search [answers|times|reminders|settings|manual|windows|clipboard|agent|glance|old-words|remembered|bookmarks|notes|meaning] [on|off]"
        exit 2
    }
    state_of() {
        if [[ "$(gsettings get "${schema}" "$1")" == true ]]; then echo on; else echo off; fi
    }
    what="${1:-}"
    state="${2:-}"
    if [[ -z "${what}" ]]; then
        for part in "${parts[@]}"; do
            printf '%-10s %s\n' "${part}" "$(state_of "${part}")"
        done
        exit 0
    fi
    # Each part by the name Control gives it
    case "${what}" in
        answers) title="$(gettext "Sums and units")" ;;
        times) title="$(gettext "Time in other places")" ;;
        reminders) title="$(gettext "Reminders into Notes")" ;;
        settings) title="$(gettext "Settings from Control")" ;;
        manual) title="$(gettext "The manual")" ;;
        windows) title="$(gettext "Open windows")" ;;
        clipboard) title="$(gettext "Clipboard history")" ;;
        agent) title="$(gettext "Ask the agent")" ;;
        glance) title="$(gettext "Glance at files")" ;;
        old-words) title="$(gettext "Words you already know")" ;;
        remembered) title="$(gettext "Files by what you remember")" ;;
        bookmarks) title="$(gettext "Bookmarks")" ;;
        notes) title="$(gettext "Notes")" ;;
        meaning) title="$(gettext "Files by meaning")" ;;
        *) usage ;;
    esac
    case "${state}" in
        "") state_of "${what}" ;;
        on)
            gsettings set "${schema}" "${what}" true
            # Files by meaning reads Documents' text from the indexer once a day on mains power, starting now
            [[ "${what}" != meaning ]] || /usr/libexec/amethystora-file-meaning on
            echo "${title}: $(gettext "On")"
            ;;
        off)
            gsettings set "${schema}" "${what}" false
            # ...and off, its daily reading stops and what it worked out is deleted
            [[ "${what}" != meaning ]] || /usr/libexec/amethystora-file-meaning off
            echo "${title}: $(gettext "Off")"
            ;;
        *) usage ;;
    esac

# The emoji and symbols picker (Super+.): the skin tone of the hands and people it shows (ame desktop emoji tone
# none|light|medium-light|medium|medium-dark|dark). Without anything, the tone and the key
[positional-arguments]
emoji ACTION="" VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.emoji
    usage() {
        echo "usage: ame desktop emoji [tone none|light|medium-light|medium|medium-dark|dark]"
        exit 2
    }
    case "${1:-}" in
        "")
            tone="$(gsettings get "${schema}" skin-tone | tr -d "'")"
            keys="$(gsettings get "${schema}" emoji-picker)"
            echo "$(eval_gettext "Skin tone: \${tone}")"
            echo "$(eval_gettext "Key: \${keys}")"
            ;;
        tone)
            tone="${2:-}"
            case "${tone}" in
                none)
                    gsettings set "${schema}" skin-tone none
                    echo "$(gettext "The emoji picker shows hands and people without a skin tone, in plain yellow.")"
                    ;;
                light | medium-light | medium | medium-dark | dark)
                    gsettings set "${schema}" skin-tone "${tone}"
                    echo "$(eval_gettext "The emoji picker shows hands and people in the \${tone} skin tone.")"
                    ;;
                *) usage ;;
            esac
            ;;
        *) usage ;;
    esac

# Force quit: Super+Ctrl+Shift+Esc lists the running apps with what each uses, and ends the one you pick after asking once
# more (STATE: on or off; without it, it flips)
[positional-arguments]
force-quit STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.quit
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" force-quit)" == *"'<"* ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings reset "${schema}" force-quit
            echo "$(gettext "Force quit: on. Press Super+Ctrl+Shift+Esc.")"
            ;;
        off)
            gsettings set "${schema}" force-quit "[]"
            echo "$(gettext "Force quit: off.")"
            ;;
        *)
            echo "usage: ame desktop force-quit on|off"
            exit 64
            ;;
    esac

# When an app stops responding, the question over it in the image's look: Wait first, and Ask the agent why while the
# agentic features are on; off, GNOME's own (STATE: on or off; without it, it flips)
[positional-arguments]
frozen-dialog STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.quit
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" frozen-dialog)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" frozen-dialog true
            echo "$(gettext "The not-responding question: Amethystora's.")"
            ;;
        off)
            gsettings set "${schema}" frozen-dialog false
            echo "$(gettext "The not-responding question: GNOME's own.")"
            ;;
        *)
            echo "usage: ame desktop frozen-dialog on|off"
            exit 64
            ;;
    esac

# Text shortcuts: a shortcut you type, then a space, becomes its text in apps that use GNOME's input method (STATE: on or
# off; without it, it flips). Off until you turn it on, since it looks at what you type
[positional-arguments]
text-shortcuts STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.typing
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" expand)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" expand true
            echo "$(gettext "Text shortcuts: on. Type a shortcut, then a space, in an app that uses GNOME's input method.")"
            ;;
        off)
            gsettings set "${schema}" expand false
            echo "$(gettext "Text shortcuts: off.")"
            ;;
        *)
            echo "usage: ame desktop text-shortcuts on|off"
            exit 64
            ;;
    esac

# Your text shortcuts: add ABBR (a new one, or a new text for one you have, the text typed or piped in after it; add ABBR
# TEXT works too, but other accounts can see TEXT in the process list while it runs), remove ABBR, list, show ABBR. Each
# value is handed on as it is, never through a shell; the shortcuts are ~/.config/amethystora/text-shortcuts.json
[positional-arguments]
text-shortcut *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    helper=/usr/libexec/amethystora-text-shortcuts
    usage() {
        echo "usage: ame desktop text-shortcut add ABBR [TEXT] | remove ABBR | list | show ABBR"
        exit 64
    }
    case "${1:-}:$#" in
        add:2 | add:3)
            # Without TEXT the helper reads it from this recipe's stdin, which is ame's
            "${helper}" add "${@:2}"
            abbr="$2"
            echo "$(eval_gettext "The shortcut \${abbr} is saved.")"
            ;;
        remove:2)
            "${helper}" remove "$2"
            abbr="$2"
            echo "$(eval_gettext "The shortcut \${abbr} is removed.")"
            ;;
        show:2) "${helper}" show "$2" ;;
        list:1)
            list="$("${helper}" list)"
            if [[ -n "${list}" ]]; then
                printf '%s\n' "${list}"
            else
                echo "$(gettext "No text shortcuts yet. Add one with ame desktop text-shortcut add ';addr', then type its text.")"
            fi
            ;;
        *) usage ;;
    esac

# Every keyboard shortcut, GNOME's, the desktop's, the tiling's and the custom ones (Control's Keyboard page): list them,
# set one to a combination (set SCHEMA KEY ACCEL, such as set org.gnome.shell.extensions.amethystora-ui.keys plain-paste
# '<Super><Alt>v'), take one off (clear SCHEMA KEY) or put it back as the image ships it (reset SCHEMA KEY). SCHEMA is the
# shortcut's GSettings schema, or custom with KEY a custom keybinding's id
[positional-arguments]
shortcut *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    helper=/usr/libexec/amethystora-keybindings
    case "${1:-}:$#" in
        :0 | list:1) "${helper}" list ;;
        set:4)
            "${helper}" set "$2" "$3" "$4"
            key="$3"
            echo "$(eval_gettext "The shortcut \${key} is set.")"
            ;;
        clear:3)
            "${helper}" clear "$2" "$3"
            key="$3"
            echo "$(eval_gettext "The shortcut \${key} is off.")"
            ;;
        reset:3)
            "${helper}" reset "$2" "$3"
            key="$3"
            echo "$(eval_gettext "The shortcut \${key} is as the image ships it.")"
            ;;
        *)
            echo "usage: ame desktop shortcut list | set SCHEMA KEY ACCEL | clear SCHEMA KEY | reset SCHEMA KEY"
            exit 64
            ;;
    esac

# The keyboard settings GNOME keeps out of sight (Control's Keyboard page): what Caps Lock does (caps normal, escape, ctrl,
# backspace or none), the Compose key (compose off, ralt, rctrl, menu, rwin or caps), what Super on its own opens (super
# overview, search or nothing), middle-click paste (middle-paste on|off), Num Lock on at start (numlock on|off), how long a
# key is held before it repeats (repeat-delay 150 to 1000 ms) and whether each window keeps its keyboard layout
# (per-window on|off). Without WHAT, how each is set now
[positional-arguments]
keyboard WHAT="" VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    input=org.gnome.desktop.input-sources
    keyboard=org.gnome.desktop.peripherals.keyboard
    keys=org.gnome.shell.extensions.amethystora-ui.keys
    what="${1:-}"
    value="${2:-}"
    usage() {
        echo "usage: ame desktop keyboard caps normal|escape|ctrl|backspace|none | compose off|ralt|rctrl|menu|rwin|caps"
        echo "       | super overview|search|nothing | middle-paste on|off | numlock on|off | repeat-delay MS | per-window on|off"
        exit 64
    }
    # xkb-options, one option a line: names such as caps:escape, which hold no quote or comma
    options() {
        local raw
        raw="$(gsettings get "${input}" xkb-options)"
        raw="${raw#@as }"
        raw="${raw#[}"
        raw="${raw%]}"
        tr ',' '\n' <<<"${raw}" | sed -E "s/^ *'//; s/' *$//" | grep -v '^$' || true
    }
    # The options but those matching $1 (an extended expression), with $2 added unless it is empty
    replace() {
        local list=() option out="[" first=true
        while IFS= read -r option; do
            [[ "${option}" =~ $1 ]] || list+=("${option}")
        done < <(options)
        [[ -n "$2" ]] && list+=("$2")
        for option in "${list[@]}"; do
            ${first} || out+=", "
            out+="'${option}'"
            first=false
        done
        gsettings set "${input}" xkb-options "${out}]"
    }
    onoff() {
        [[ "${value}" == on || "${value}" == off ]] || usage
    }
    case "${what}" in
        "")
            echo "xkb-options: $(options | paste -sd ' ')"
            echo "super: $(gsettings get "${keys}" super-alone 2>/dev/null | tr -d "'")"
            echo "middle-paste: $(gsettings get org.gnome.desktop.interface gtk-enable-primary-paste)"
            echo "numlock: remember $(gsettings get "${keyboard}" remember-numlock-state), on $(gsettings get "${keyboard}" numlock-state)"
            echo "repeat-delay: $(gsettings get "${keyboard}" delay | sed 's/^uint32 //')"
            echo "per-window: $(gsettings get "${input}" per-window)"
            ;;
        caps)
            case "${value}" in
                normal) option="" ;;
                escape) option=caps:escape ;;
                ctrl) option=ctrl:nocaps ;;
                backspace) option=caps:backspace ;;
                none) option=caps:none ;;
                *) usage ;;
            esac
            replace '^(caps:.*|ctrl:nocaps|ctrl:swapcaps)$' "${option}"
            echo "$(eval_gettext "Caps Lock: \${value}.")"
            ;;
        compose)
            case "${value}" in
                off) option="" ;;
                ralt | rctrl | menu | rwin | caps) option="compose:${value}" ;;
                *) usage ;;
            esac
            replace '^compose:.*$' "${option}"
            echo "$(eval_gettext "Compose key: \${value}.")"
            ;;
        super)
            case "${value}" in
                overview | search | nothing) gsettings set "${keys}" super-alone "${value}" ;;
                *) usage ;;
            esac
            echo "$(eval_gettext "Super on its own opens: \${value}.")"
            ;;
        middle-paste)
            onoff
            gsettings set org.gnome.desktop.interface gtk-enable-primary-paste "$([[ "${value}" == on ]] && echo true || echo false)"
            echo "$(eval_gettext "Middle-click paste: \${value}. Apps that are open take it when they next start.")"
            ;;
        numlock)
            onoff
            if [[ "${value}" == on ]]; then
                gsettings set "${keyboard}" remember-numlock-state false
                gsettings set "${keyboard}" numlock-state true
                echo "$(gettext "Num Lock is on at every start.")"
            else
                gsettings set "${keyboard}" remember-numlock-state true
                echo "$(gettext "Num Lock starts as it was left.")"
            fi
            ;;
        repeat-delay)
            [[ "${value}" =~ ^[0-9]{3,4}$ ]] && ((10#${value} >= 150 && 10#${value} <= 1000)) || usage
            gsettings set "${keyboard}" delay "$((10#${value}))"
            ms="$((10#${value}))"
            echo "$(eval_gettext "A held key repeats after \${ms} ms.")"
            ;;
        per-window)
            onoff
            gsettings set "${input}" per-window "$([[ "${value}" == on ]] && echo true || echo false)"
            if [[ "${value}" == on ]]; then
                echo "$(gettext "Each window keeps its own keyboard layout.")"
            else
                echo "$(gettext "One keyboard layout for every window.")"
            fi
            ;;
        *) usage ;;
    esac

# Hold a key for accents: a letter held shows its accented forms over the text cursor, and a number picks one (STATE: on
# or off; without it, it flips). Off until you turn it on, since it looks at what you type
[positional-arguments]
accents STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.typing
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" accents)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" accents true
            echo "$(gettext "Hold a key for accents: on. Hold a letter in an app, then press a number.")"
            ;;
        off)
            gsettings set "${schema}" accents false
            echo "$(gettext "Hold a key for accents: off.")"
            ;;
        *)
            echo "usage: ame desktop accents on|off"
            exit 64
            ;;
    esac

# The terminal's own tools in place of the usual ones: eza for ls, bat for cat, ugrep for grep, with zoxide, direnv and mise (ACTION: on or off)
[positional-arguments]
bling ACTION="":
    @amethystora-bling ${1:+"$1"}

# The desktop's own sounds: soft, Amethystora's lower and rounder set; glass, bright short chimes; wood, warm knocks and
# taps; retro, eight-bit blips; or off (apps still make their own; SET: soft, glass, wood, retro or off; without it, it
# asks)
[positional-arguments]
sounds SET="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.desktop.sound
    current=""
    if [[ "$(gsettings get "${schema}" event-sounds)" == false ]]; then
        current=off
    else
        case "$(gsettings get "${schema}" theme-name)" in
            "'amethystora'") current=soft ;;
            "'amethystora-glass'") current=glass ;;
            "'amethystora-wood'") current=wood ;;
            "'amethystora-retro'") current=retro ;;
        esac
    fi
    choice="${1:-}"
    if [[ -z "${choice}" ]]; then
        echo "soft   $(gettext "Lower and rounder, for a quiet room")"
        echo "glass  $(gettext "Bright, short chimes")"
        echo "wood   $(gettext "Warm knocks and taps")"
        echo "retro  $(gettext "Eight-bit blips")"
        echo "off    $(gettext "The desktop makes no sound of its own; apps still can")"
        selected=()
        [[ -n "${current}" ]] && selected=(--selected "${current}")
        choice="$(gum choose --header "$(gettext "Sounds")" "${selected[@]}" soft glass wood retro off)" || exit 0
    fi
    case "${choice}" in
        soft | glass | wood | retro)
            theme=amethystora
            [[ "${choice}" == soft ]] || theme="amethystora-${choice}"
            gsettings set "${schema}" theme-name "${theme}"
            gsettings set "${schema}" event-sounds true
            case "${choice}" in
                soft) echo "$(gettext "Sounds: soft.")" ;;
                glass) echo "$(gettext "Sounds: glass.")" ;;
                wood) echo "$(gettext "Sounds: wood.")" ;;
                retro) echo "$(gettext "Sounds: retro.")" ;;
            esac
            ;;
        off)
            # GNOME's volume keys play their step whatever event-sounds says: only this theme's silent step stops it
            gsettings set "${schema}" theme-name amethystora-silent
            gsettings set "${schema}" event-sounds false
            echo "$(gettext "Sounds: off. Apps can still make sounds of their own.")"
            ;;
        *)
            echo "usage: ame desktop sounds soft|glass|wood|retro|off"
            exit 64
            ;;
    esac

# Pause when the headphones come off (the audio module): the players pause when the headphones or a headset in use go
# away, so that nothing plays out loud from the speakers (STATE: on or off; without it, it flips)
[positional-arguments]
pause-on-unplug STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.audio
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" pause-on-unplug)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" pause-on-unplug true
            echo "$(gettext "Pause when the headphones come off: on. The players pause when the headphones in use go away.")"
            ;;
        off)
            gsettings set "${schema}" pause-on-unplug false
            echo "$(gettext "Pause when the headphones come off: off.")"
            ;;
        *)
            echo "usage: ame desktop pause-on-unplug on|off"
            exit 64
            ;;
    esac

# Clear voice: the microphone through PipeWire's own echo cancel and noise suppression, as an input of its own made the
# one in use; it works only while an app records, and nothing is sent or saved (STATE: on or off; without it, it flips)
[positional-arguments]
clear-voice STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(/usr/libexec/amethystora-sound clear-voice status)" == on ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            /usr/libexec/amethystora-sound clear-voice on
            echo "$(gettext "Clear voice: on. Calls hear your voice without the background noise and the echo.")"
            ;;
        off)
            /usr/libexec/amethystora-sound clear-voice off
            echo "$(gettext "Clear voice: off. The microphone is as it comes.")"
            ;;
        *)
            echo "usage: ame desktop clear-voice on|off"
            exit 64
            ;;
    esac

# Sound presets: the output menu offers Flat, Laptop, Speech and Bass per output, as PipeWire's own equaliser before
# that output (STATE: on or off; without it, it flips). Off, every output plays as it comes; the choices are kept.
[positional-arguments]
sound-presets STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(/usr/libexec/amethystora-sound presets status | head -n1)" == on ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            /usr/libexec/amethystora-sound presets on
            echo "$(gettext "Sound presets: on. Pick one for each output in the output menu of quick settings, or with ame desktop sound-preset.")"
            ;;
        off)
            /usr/libexec/amethystora-sound presets off
            echo "$(gettext "Sound presets: off. Every output plays as it comes.")"
            ;;
        *)
            echo "usage: ame desktop sound-presets on|off"
            exit 64
            ;;
    esac

# The preset of the output in use: flat, laptop (fills out thin speakers), speech (voices and podcasts clearer) or bass
# (PRESET; without it, it asks). Kept for that output only; it plays while sound presets are on.
[positional-arguments]
sound-preset PRESET="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    preset="${1:-}"
    if [[ -z "${preset}" ]]; then
        echo "flat    $(gettext "As it comes")"
        echo "laptop  $(gettext "Fills out thin speakers")"
        echo "speech  $(gettext "Voices and podcasts clearer")"
        echo "bass    $(gettext "More bass, for music")"
        preset="$(gum choose --header "$(gettext "Preset for the output in use")" flat laptop speech bass)" || exit 0
    fi
    case "${preset}" in
        flat | laptop | speech | bass) ;;
        *)
            echo "usage: ame desktop sound-preset flat|laptop|speech|bass"
            exit 64
            ;;
    esac
    /usr/libexec/amethystora-sound preset "${preset}"
    if [[ "$(/usr/libexec/amethystora-sound presets status | head -n1)" == on ]]; then
        echo "$(eval_gettext "Preset for the output in use: \${preset}.")"
    else
        echo "$(eval_gettext "Preset for the output in use: \${preset}. It plays once sound presets are on: ame desktop sound-presets on")"
    fi

# The brightness of external screens in quick settings, over DDC/CI with ddcutil (STATE: on or off; without it, it
# flips). On gives the person at the machine the displays' i2c buses (a udev rule, asking for an administrator's
# password); off takes them back.
[positional-arguments]
screen-brightness STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.brightness
    rule=/etc/udev/rules.d/60-amethystora-ddc.rules
    modules=/etc/modules-load.d/amethystora-ddc.conf
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ -e "${rule}" ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            echo "$(gettext "External screens' brightness: each screen that speaks DDC/CI gets a slider in quick settings.")"
            echo "$(gettext "It gives the person at the machine the i2c buses of the displays, which ddcutil talks to the screens on. Such a bus can also reach other chips of the graphics card, so it is offered rather than on. Your administrator's password is asked.")"
            sudo install -Dm0644 /usr/share/amethystora/ddc/60-amethystora-ddc.rules "${rule}"
            sudo install -Dm0644 /usr/share/amethystora/ddc/amethystora-ddc.conf "${modules}"
            sudo modprobe i2c-dev
            sudo udevadm control --reload
            sudo udevadm trigger --subsystem-match=i2c-dev --action=change
            gsettings set "${schema}" external-screens true
            echo "$(gettext "External screens' brightness: on. Open quick settings to see a slider for each screen.")"
            ;;
        off)
            gsettings set "${schema}" external-screens false
            if [[ -e "${rule}" || -e "${modules}" ]]; then
                sudo rm -f "${rule}" "${modules}"
                sudo udevadm control --reload
                sudo udevadm trigger --subsystem-match=i2c-dev --action=change
            fi
            echo "$(gettext "External screens' brightness: off. The displays' i2c buses are root's again.")"
            ;;
        *)
            echo "usage: ame desktop screen-brightness on|off"
            exit 64
            ;;
    esac

# Rounded corners on a laptop's own square screen, a small black curve in each (STATE: on or off; without it, it flips)
[positional-arguments]
screen-corners STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.bezel
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" round-corners)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" round-corners true
            echo "$(gettext "Rounded screen corners: on.")"
            # Only a screen built into the computer is rounded: Mutter says which ones are
            screens="$(gdbus call --session --dest org.gnome.Mutter.DisplayConfig \
                --object-path /org/gnome/Mutter/DisplayConfig \
                --method org.gnome.Mutter.DisplayConfig.GetCurrentState 2>/dev/null || true)"
            if [[ "${screens}" != *"'is-builtin': <true>"* ]]; then
                echo "$(gettext "This computer has no screen of its own.")"
            fi
            ;;
        off)
            gsettings set "${schema}" round-corners false
            echo "$(gettext "Rounded screen corners: off.")"
            ;;
        *)
            echo "usage: ame desktop screen-corners on|off"
            exit 64
            ;;
    esac

# Tips (the hints module): one the first time a feature can help, once each (ACTION: on, off, or reset to bring back the tips seen)
[positional-arguments]
hints ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.hints
    action="${1:-}"
    if [[ -z "${action}" ]]; then
        if [[ "$(gsettings get "${schema}" enabled)" == true ]]; then action=off; else action=on; fi
    fi
    case "${action}" in
        on)
            gsettings set "${schema}" enabled true
            echo "$(gettext "Tips: on.")"
            ;;
        off)
            gsettings set "${schema}" enabled false
            echo "$(gettext "Tips: off.")"
            ;;
        reset)
            # What was shown is forgotten, the first day kept; each file written whole and moved into place, which the
            # running desktop notices
            file="${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/hints.json"
            for path in "${file}" "${file}.bak"; do
                [[ -f "${path}" ]] || continue
                next="$(mktemp "${path}.XXXXXX")"
                if jq -c '(if type == "object" then . else {} end) | .shown = {}' "${path}" >"${next}" 2>/dev/null; then
                    mv -f -- "${next}" "${path}"
                else
                    rm -f -- "${next}"
                    jq -nc '{version: 1, since: 0, last: 0, shown: {}}' >"${path}"
                fi
            done
            echo "$(gettext "Tips you have seen can come back.")"
            ;;
        *)
            echo "usage: ame desktop hints on|off|reset"
            exit 64
            ;;
    esac

# The app grid: recent [on|off], the row of the apps used most at its top (without on or off, it flips); folders reset, its folders as the image has them; or order custom|name|use|kind, where you put each icon, A to Z, by use or by kind
[positional-arguments]
app-grid WHAT="" VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.appgrid
    case "${1:-}/${2:-}" in
        recent/*)
            state="${2:-}"
            if [[ -z "${state}" ]]; then
                if [[ "$(gsettings get "${schema}" recent)" == true ]]; then state=off; else state=on; fi
            fi
            case "${state}" in
                on)
                    gsettings set "${schema}" recent true
                    echo "$(gettext "Recently used apps: on.")"
                    ;;
                off)
                    gsettings set "${schema}" recent false
                    echo "$(gettext "Recently used apps: off.")"
                    ;;
                *)
                    echo "usage: ame desktop app-grid recent on|off"
                    exit 64
                    ;;
            esac
            ;;
        folders/reset)
            echo "$(gettext "The app grid's folders go back to the image's: Office, Graphics, Games, System, Development and Utilities, filled from the apps' own categories. Folders you made or renamed, and where you put the icons, are forgotten.")"
            gum confirm "$(gettext "Reset the folders?")" || exit 0
            gsettings reset org.gnome.desktop.app-folders folder-children
            dconf reset -f /org/gnome/desktop/app-folders/folders/
            gsettings reset org.gnome.shell app-picker-layout
            echo "$(gettext "The app grid's folders are the image's again.")"
            ;;
        order/custom | order/name | order/use | order/kind)
            /usr/libexec/amethystora-app-look order "$2"
            case "$2" in
                custom) echo "$(gettext "The app grid is in your own order again: where you put each icon.")" ;;
                name) echo "$(gettext "The app grid is in order from A to Z, after your folders.")" ;;
                use) echo "$(gettext "The app grid has the apps you use most first, after your folders.")" ;;
                kind) echo "$(gettext "The app grid has its apps by kind, after your folders.")" ;;
            esac
            ;;
        order/)
            echo "$(gsettings get "${schema}" order | tr -d "'")"
            ;;
        *)
            echo "usage: ame desktop app-grid recent [on|off] | folders reset | order [custom|name|use|kind]"
            exit 64
            ;;
    esac

# One app in the app grid and the search: hide ID, show ID, rename ID [NAME] (without a name, its own again), icon ID
# PACK|reset|picture FILE. ID is its launcher's, such as org.gnome.Calculator.desktop. Nothing is written but the app
# grid's own settings: no launcher of your own, which the security watcher would read as planted.
[positional-arguments]
app *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    usage() {
        echo "usage: ame desktop app hide ID | show ID | rename ID [NAME] | icon ID PACK|reset | icon ID picture FILE"
        exit 64
    }
    [[ $# -ge 2 ]] || usage
    what="$1" id="$2"
    case "${what}" in
        hide)
            [[ $# -eq 2 ]] || usage
            /usr/libexec/amethystora-app-look hide "${id}"
            echo "$(eval_gettext "\${id} is hidden from the app grid and the search. It still opens from a terminal and from Open With.")"
            ;;
        show)
            [[ $# -eq 2 ]] || usage
            /usr/libexec/amethystora-app-look show "${id}"
            echo "$(eval_gettext "\${id} is in the app grid and the search again.")"
            ;;
        rename)
            [[ $# -le 3 ]] || usage
            /usr/libexec/amethystora-app-look rename "${id}" "${3:-}"
            if [[ -n "${3:-}" ]]; then
                echo "$(eval_gettext "\${id} has a name of your own in the app grid.")"
            else
                echo "$(eval_gettext "\${id} has its own name again.")"
            fi
            ;;
        icon)
            [[ $# -ge 3 ]] || usage
            if [[ "$3" == picture ]]; then
                [[ $# -eq 4 ]] || usage
                file="$(realpath -e -- "$4" 2>/dev/null)" || { echo "$(gettext "There is no such picture.")"; exit 1; }
                if ! /usr/libexec/amethystora-app-look icon "${id}" picture "${file}"; then
                    echo "$(gettext "That picture could not be used. Pick a PNG, JPEG, SVG or WebP picture.")"
                    exit 1
                fi
                echo "$(eval_gettext "\${id} shows your picture as its icon.")"
            else
                [[ $# -eq 3 ]] || usage
                /usr/libexec/amethystora-app-look icon "${id}" "$3"
                if [[ "$3" == reset ]]; then
                    echo "$(eval_gettext "\${id} has its own icon again.")"
                else
                    echo "$(eval_gettext "\${id} shows its icon from another pack.")"
                fi
            fi
            ;;
        *)
            usage
            ;;
    esac

# Downloads tidied once a day (off until turned on): installers opened more than a week ago to the Trash, other files
# older than a month into a folder for their month. on, off, undo (the last tidying put back), or now
[positional-arguments]
tidy-downloads ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    timer=amethystora-tidy-downloads.timer
    case "${1:-}" in
        on)
            systemctl --user enable --now "${timer}" >/dev/null 2>&1
            echo "$(gettext "Downloads keeps itself tidy: once a day, from now on. Nothing is deleted, and Undo puts the last tidying back.")"
            ;;
        off)
            systemctl --user disable --now "${timer}" >/dev/null 2>&1 || true
            echo "$(gettext "Downloads is left as it is.")"
            ;;
        undo)
            /usr/libexec/amethystora-tidy-downloads undo
            ;;
        now)
            /usr/libexec/amethystora-tidy-downloads now
            ;;
        "")
            if systemctl --user is-enabled --quiet "${timer}" 2>/dev/null; then echo on; else echo off; fi
            ;;
        *)
            echo "usage: ame desktop tidy-downloads on|off|undo|now"
            exit 64
            ;;
    esac

# When the disk your backup goes to is plugged in: Back up now?, then Safe to unplug (on as the image ships)
[positional-arguments]
backup-disk STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.backupdisk
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" ask)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" ask true
            echo "$(gettext "Plugging in your backup disk asks whether to back up now.")"
            ;;
        off)
            gsettings set "${schema}" ask false
            echo "$(gettext "Plugging in your backup disk asks nothing.")"
            ;;
        *)
            echo "usage: ame desktop backup-disk on|off"
            exit 64
            ;;
    esac

# Easier to see and read: colour filters over the whole screen, a reading ruler that follows the pointer
# (Super+Ctrl+R shows and hides it) and the selected text read aloud on this computer (Super+Ctrl+S), each off
# until chosen here.
# Colour filters, the reading ruler and reading aloud: how each is set, or choose one
[positional-arguments]
access *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.access
    usage() {
        echo "usage: ame desktop access [filter none|protan|deutan|tritan|grey|on|off | strength 25-100 | ruler off|band|dim|underline|show|hide | read-aloud on|off | read-aloud-speed 0.75|1|1.5|2 | letters on|off]"
        exit 64
    }
    value() { gsettings get "${schema}" "$1" | tr -d "'"; }
    if [[ $# -eq 0 ]]; then
        filter="$(value filter)"
        strength="$(value filter-strength)"
        if [[ "${filter}" != none && "$(value filter-on)" == false ]]; then
            echo "$(eval_gettext "Colour filter: \${filter}, \${strength}%, switched off in quick settings")"
        else
            echo "$(eval_gettext "Colour filter: \${filter}, \${strength}%")"
        fi
        ruler="$(value ruler)"
        echo "$(eval_gettext "Reading ruler: \${ruler} (Super+Ctrl+R shows and hides it)")"
        if [[ "$(value read-aloud)" == true ]]; then state=on; else state=off; fi
        speed="$(value read-aloud-speed)"
        echo "$(eval_gettext "Read aloud: \${state}, at \${speed}× (Super+Ctrl+S reads the selection)")"
        if [[ "$(value letters)" == true ]]; then state=on; else state=off; fi
        echo "$(eval_gettext "Click with the keyboard: \${state} (Super+Alt+K)")"
        exit 0
    fi
    [[ $# -eq 2 ]] || usage
    case "$1:$2" in
        filter:none)
            gsettings set "${schema}" filter none
            echo "$(gettext "Colour filters are off.")"
            ;;
        filter:protan | filter:deutan | filter:tritan | filter:grey)
            gsettings set "${schema}" filter "$2"
            gsettings set "${schema}" filter-on true
            echo "$(gettext "The colour filter is on. Its tile in quick settings switches it off and on, and picks another filter or strength. Screenshots are saved without it, and recordings and screen shares may show the apps' own colours.")"
            ;;
        filter:on | filter:off)
            [[ "$(value filter)" != none ]] || { echo "$(gettext "Choose a filter first: ame desktop access filter protan, deutan, tritan or grey.")"; exit 1; }
            if [[ "$2" == on ]]; then gsettings set "${schema}" filter-on true; else gsettings set "${schema}" filter-on false; fi
            ;;
        strength:*)
            [[ "$2" =~ ^[0-9]{2,3}$ ]] && (( $2 >= 25 && $2 <= 100 )) || usage
            gsettings set "${schema}" filter-strength "$((10#$2))"
            ;;
        ruler:off)
            gsettings set "${schema}" ruler off
            echo "$(gettext "The reading ruler is off.")"
            ;;
        ruler:band | ruler:dim | ruler:underline)
            gsettings set "${schema}" ruler "$2"
            gsettings set "${schema}" ruler-shown true
            echo "$(gettext "The reading ruler is on and follows the pointer. Super+Ctrl+R hides it and shows it again.")"
            ;;
        ruler:show | ruler:hide)
            [[ "$(value ruler)" != off ]] || { echo "$(gettext "Choose a ruler first: ame desktop access ruler band, dim or underline.")"; exit 1; }
            if [[ "$2" == show ]]; then gsettings set "${schema}" ruler-shown true; else gsettings set "${schema}" ruler-shown false; fi
            ;;
        read-aloud:on)
            gsettings set "${schema}" read-aloud true
            echo "$(gettext "Read aloud is on: select some text and press Super+Ctrl+S. It is read on this computer with the screen reader's voice, and nothing is sent anywhere.")"
            ;;
        read-aloud:off)
            gsettings set "${schema}" read-aloud false
            echo "$(gettext "Read aloud is off.")"
            ;;
        read-aloud-speed:0.75 | read-aloud-speed:1 | read-aloud-speed:1.5 | read-aloud-speed:2)
            gsettings set "${schema}" read-aloud-speed "$2"
            ;;
        # Click with the keyboard (plans/ui-ideas/spec-later.md, R5-36)
        letters:on)
            gsettings set "${schema}" letters true
            echo "$(gettext "Click with the keyboard is on: Super+Alt+K puts a letter on every button and link of the window in use, and typing it clicks. The window's controls are read from its accessibility tree, on this computer, only while the letters show.")"
            ;;
        letters:off)
            gsettings set "${schema}" letters false
            echo "$(gettext "Click with the keyboard is off.")"
            ;;
        *) usage ;;
    esac

# Touch and pen: touch mode sizes the shell for a fingertip (follow: while the computer is folded into a tablet), and
# the pen's tools open around the pen's tip from its side button (Super+Ctrl+P). Both are off until chosen here.
# pen-button on sets every pen this account has used, and every pen attached now that has no serial number of its own,
# to send the tools' shortcut from its side button, keeping what it was; off puts that back.
# Touch mode, the pen's tools and the pen's button: how each is set, or choose one
[positional-arguments]
touch *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.touch
    stylus=org.gnome.desktop.peripherals.tablet.stylus
    styli=/org/gnome/desktop/peripherals/stylus
    kept="${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/pen-button"
    tools=(scribble area note text right-click back)
    usage() {
        echo "usage: ame desktop touch [mode off|follow|on | pen-ring off|ring|strip | pen-tools TOOL... | pen-button on|off]"
        exit 64
    }
    value() { gsettings get "${schema}" "$1" | tr -d "'"; }
    # The pens whose settings Mutter reads, as "known ID": those GNOME has settings for and those the pen ring has
    # seen (each by its serial number, or by its tablet for a pen without one); and as "tablet ID", for each tablet
    # attached now, the settings its pens without a serial number of their own are read from
    pens() {
        {
            dconf list "${styli}/" 2>/dev/null | sed 's|/$||'
            cat "${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/pens" 2>/dev/null || true
        } | grep -E '^([0-9a-f]{1,16}|default-[0-9a-f]{4}:[0-9a-f]{4})$' | sort -u | sed 's/^/known /' || true
        local event props
        for event in /sys/class/input/event*; do
            [[ -e "${event}" ]] || continue
            props="$(udevadm info -q property -p "${event}" 2>/dev/null || true)"
            grep -qx 'ID_INPUT_TABLET=1' <<<"${props}" || continue
            grep -qx 'ID_INPUT_TABLET_PAD=1' <<<"${props}" && continue
            printf 'default-%s:%s\n' "$(cat "${event}/device/id/vendor" 2>/dev/null)" "$(cat "${event}/device/id/product" 2>/dev/null)"
        done | grep -E '^default-[0-9a-f]{4}:[0-9a-f]{4}$' | sort -u | sed 's/^/tablet /' || true
    }
    pen_button() {
        local pen kind binding action key n=0 t=0 seen=" "
        binding="$(gsettings get "${schema}" pen-ring-key | sed -n "s/^\['\([^']*\)'.*/\1/p")"
        if [[ "$1" == off ]]; then
            [[ -r "${kept}" ]] || { echo "$(gettext "No pen was set up for the tools.")"; return 0; }
            while IFS=$'\t' read -r pen action key; do
                [[ "${pen}" =~ ^([0-9a-f]{1,16}|default-[0-9a-f]{4}:[0-9a-f]{4})$ ]] || continue
                # Only a button still set as on left it: one changed since in Settings is the person's
                [[ "$(gsettings get "${stylus}:${styli}/${pen}/" button-action)" == "'keybinding'" &&
                    "$(gsettings get "${stylus}:${styli}/${pen}/" button-keybinding)" == "'${binding}'" ]] || continue
                gsettings set "${stylus}:${styli}/${pen}/" button-action "${action}"
                gsettings set "${stylus}:${styli}/${pen}/" button-keybinding "${key#-}"
            done <"${kept}"
            rm -f "${kept}" "${kept}.bak"
            echo "$(gettext "The pens' side buttons do what they did before.")"
            return 0
        fi
        [[ -n "${binding}" ]] || { echo "$(gettext "The tools have no shortcut to send.")"; return 1; }
        mkdir -p "$(dirname "${kept}")"
        [[ -e "${kept}" ]] && cp "${kept}" "${kept}.bak"
        cp "${kept}" "${kept}.tmp" 2>/dev/null || : >"${kept}.tmp"
        while read -r kind pen; do
            [[ "${seen}" == *" ${pen} "* ]] && continue
            seen+="${pen} "
            # What it was is kept once, so that a second run never keeps the tools' own shortcut as the old value
            if ! grep -q "^${pen}"$'\t' "${kept}.tmp"; then
                action="$(gsettings get "${stylus}:${styli}/${pen}/" button-action | tr -d "'")"
                key="$(gsettings get "${stylus}:${styli}/${pen}/" button-keybinding | tr -d "'")"
                printf '%s\t%s\t%s\n' "${pen}" "${action}" "${key:--}" >>"${kept}.tmp"
            fi
            gsettings set "${stylus}:${styli}/${pen}/" button-action keybinding
            gsettings set "${stylus}:${styli}/${pen}/" button-keybinding "${binding}"
            if [[ "${kind}" == known ]]; then n=$((n + 1)); else t=$((t + 1)); fi
        done < <(pens)
        mv -f "${kept}.tmp" "${kept}"
        [[ "${n}" -eq 0 ]] ||
            echo "$(eval_ngettext "The side button of \${n} pen used here now opens the tools." "The side buttons of \${n} pens used here now open the tools." "${n}")"
        [[ "${t}" -eq 0 ]] ||
            echo "$(eval_ngettext "On \${t} tablet attached now, a pen without a serial number of its own opens them too." "On \${t} tablets attached now, a pen without a serial number of its own opens them too." "${t}")"
        [[ "${n}" -gt 0 || "${t}" -gt 0 ]] || echo "$(gettext "No pen found.")"
        echo "$(gettext "A pen with a serial number of its own that was not used here yet: open the tools once with Super+Ctrl+P, touch them with the pen and run this again, or choose Super+Ctrl+P for its button in Settings, under Wacom Tablet.")"
    }
    if [[ $# -eq 0 ]]; then
        mode="$(value touch-mode)"
        ring="$(value pen-ring)"
        echo "$(eval_gettext "Touch mode: \${mode}")"
        echo "$(eval_gettext "The pen's tools: \${ring} (Super+Ctrl+P, or the pen's side button)")"
        exit 0
    fi
    case "$1" in
        mode)
            [[ $# -eq 2 ]] || usage
            case "$2" in
                off) echo "$(gettext "Touch mode is off.")" ;;
                follow) echo "$(gettext "Touch mode follows the hinge: on while the computer is folded into a tablet. The Touch mode tile in quick settings changes it for now.")" ;;
                on) echo "$(gettext "Touch mode is on: a taller top bar, a larger dock, roomier menus and Today from the right edge.")" ;;
                *) usage ;;
            esac
            gsettings set "${schema}" touch-mode "$2"
            ;;
        pen-ring)
            [[ $# -eq 2 ]] || usage
            case "$2" in
                off)
                    gsettings set "${schema}" pen-ring off
                    echo "$(gettext "The pen's tools are off.")"
                    ;;
                ring | strip)
                    gsettings set "${schema}" pen-ring "$2"
                    echo "$(gettext "The pen's tools are on: Super+Ctrl+P opens them.")"
                    if [[ -t 0 && -t 1 ]] && gum confirm "$(gettext "Make the pen's side button open them too?")"; then
                        pen_button on
                    else
                        echo "$(gettext "To open them from the pen's side button: ame desktop touch pen-button on")"
                    fi
                    ;;
                *) usage ;;
            esac
            ;;
        pen-tools)
            shift
            [[ $# -ge 1 && $# -le 6 ]] || usage
            list=()
            for tool in "$@"; do
                [[ " ${tools[*]} " == *" ${tool} "* ]] || usage
                [[ " ${list[*]} " != *" '${tool}' "* ]] || usage
                list+=("'${tool}'")
            done
            gsettings set "${schema}" pen-ring-tools "[$(IFS=,; echo "${list[*]}" | sed 's/,/, /g')]"
            ;;
        pen-button)
            [[ $# -eq 2 && ( "$2" == on || "$2" == off ) ]] || usage
            pen_button "$2"
            ;;
        *) usage ;;
    esac

# Present mode (offered, off): on|off offers it (a Present tile, a question when a screen is plugged in, Super+Alt+D to draw on the screen); mark halo|torch|none, the pointer while presenting; ask on|off, the question
[positional-arguments]
present WHAT="" VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.present
    usage() {
        echo "usage: ame desktop present on|off | mark halo|torch|none | ask on|off"
        exit 64
    }
    case "${1:-}/${2:-}" in
        /)
            if [[ "$(gsettings get "${schema}" enabled)" == true ]]; then
                echo "$(gettext "Present mode is offered: the Present tile is in quick settings, and Super+Alt+D draws on the screen.")"
            else
                echo "$(gettext "Present mode is off. Turn it on with: ame desktop present on")"
            fi
            ;;
        on/)
            gsettings set "${schema}" enabled true
            echo "$(gettext "Present mode is offered: the Present tile is in quick settings, and Super+Alt+D draws on the screen.")"
            echo "$(gettext "While you present, the screen stays awake, banners wait and the pointer is easy to follow.")"
            ;;
        off/)
            gsettings set "${schema}" enabled false
            echo "$(gettext "Present mode is off.")"
            ;;
        mark/halo | mark/torch | mark/none)
            gsettings set "${schema}" pointer-mark "$2"
            case "$2" in
                halo) echo "$(gettext "While presenting, a halo follows the pointer.")" ;;
                torch) echo "$(gettext "While presenting, everything but a circle round the pointer is dimmed.")" ;;
                none) echo "$(gettext "While presenting, the pointer is left as it is.")" ;;
            esac
            ;;
        ask/on)
            gsettings set "${schema}" ask-on-screen true
            echo "$(gettext "When a screen is plugged in, you are asked whether to present on it.")"
            ;;
        ask/off)
            gsettings set "${schema}" ask-on-screen false
            echo "$(gettext "Plugging in a screen asks nothing.")"
            ;;
        *) usage ;;
    esac

# Screenshots: thumbnail on|off, the thumbnail after a capture (off, GNOME's notification); pin on|off, pinning pictures on top
[positional-arguments]
screenshot WHAT="" STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.screenshot
    usage() {
        echo "usage: ame desktop screenshot thumbnail|pin|trim on|off"
        exit 64
    }
    case "${1:-}/${2:-}" in
        /)
            if [[ "$(gsettings get "${schema}" capture-thumbnail)" == true ]]; then thumbnail=on; else thumbnail=off; fi
            if [[ "$(gsettings get "${schema}" capture-pin)" == true ]]; then pin=on; else pin=off; fi
            echo "$(eval_gettext "The thumbnail after a screenshot: \${thumbnail}. Pinning pictures on top: \${pin}.")"
            ;;
        thumbnail/on)
            gsettings set "${schema}" capture-thumbnail true
            echo "$(gettext "After a screenshot, a thumbnail rests at the bottom right: edit, copy, pin, show or delete it.")"
            ;;
        thumbnail/off)
            gsettings set "${schema}" capture-thumbnail false
            echo "$(gettext "After a screenshot, GNOME's notification says it was taken.")"
            ;;
        pin/on)
            gsettings set "${schema}" capture-pin true
            echo "$(gettext "A screenshot can be pinned above every window, from its thumbnail or with Pin in the screenshot bar.")"
            ;;
        pin/off)
            gsettings set "${schema}" capture-pin false
            echo "$(gettext "Pinning pictures is off; pinned pictures are gone.")"
            ;;
        # Trim on the recording's notice (R6-19, offered, off)
        trim/on)
            gsettings set "${schema}" recording-trim true
            echo "$(gettext "After a screen recording, its notice offers Trim: keep a part of it as WebM or a GIF. The recording itself stays.")"
            ;;
        trim/off)
            gsettings set "${schema}" recording-trim false
            echo "$(gettext "A recording's notice offers no Trim.")"
            ;;
        *) usage ;;
    esac

# Widgets on the desktop (offered, off): widgets whose widget.json says "place": "desktop" drawn on the wallpaper, under the windows
[positional-arguments]
desktop-widgets STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.deskwidgets
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" enabled)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" enabled true
            echo "$(gettext "Widgets on the desktop: on. A widget placed on the desktop in its widget.json is drawn on the wallpaper.")"
            ;;
        off)
            gsettings set "${schema}" enabled false
            echo "$(gettext "Widgets on the desktop: off. Their commands do not run.")"
            ;;
        *)
            echo "usage: ame desktop desktop-widgets on|off"
            exit 64
            ;;
    esac

# Whether a new account's first login says hello in every language Amethystora speaks: on, off, or show to see it again
[positional-arguments]
hello CHOICE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.hello
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" greet)" == true ]]; then
                echo "$(gettext "A new account is greeted at its first login.")"
            else
                echo "$(gettext "The greeting is off.")"
            fi
            ;;
        on)
            gsettings set "${schema}" greet true
            echo "$(gettext "A new account is greeted at its first login, once. An account that has logged in before is not.")"
            ;;
        off)
            gsettings set "${schema}" greet false
            echo "$(gettext "The greeting is off.")"
            ;;
        show)
            gdbus call --session --dest org.gnome.Shell --object-path /org/amethystora/UI/Hello \
                --method org.amethystora.UI.Hello.Show >/dev/null 2>&1 ||
                { echo "$(gettext "It cannot be shown: the greeting is off, or Amethystora UI is not running.")" >&2; exit 1; }
            ;;
        *) echo "usage: ame desktop hello on|off|show" >&2; exit 64 ;;
    esac

# The resting screen (R6-06, offered, off): left alone, a large clock over the drifting wallpaper, the windows hidden,
# until a key or the pointer; the lock comes when it would anyway (ame desktop resting-screen on|off; after MINUTES,
# 1 to 30, how long alone first). Without anything it says how it stands
[positional-arguments]
resting-screen *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.rest
    usage() { echo "usage: ame desktop resting-screen on|off | after 1..30" >&2; exit 64; }
    minutes() { echo $(($(gsettings get "${schema}" after) / 60)); }
    case "${1:-}" in
        "")
            [[ $# -eq 0 ]] || usage
            m="$(minutes)"
            if [[ "$(gsettings get "${schema}" enabled)" == true ]]; then
                echo "$(eval_gettext "The resting screen comes after \${m} minutes alone.")"
            else
                echo "$(gettext "The resting screen is off.")"
            fi
            ;;
        on)
            [[ $# -eq 1 ]] || usage
            gsettings set "${schema}" enabled true
            m="$(minutes)"
            echo "$(eval_gettext "Left alone \${m} minutes, the desktop rests: a large clock, the windows hidden. Any key brings everything back; the screen locks when it would anyway.")"
            ;;
        off)
            [[ $# -eq 1 ]] || usage
            gsettings set "${schema}" enabled false
            echo "$(gettext "The resting screen is off.")"
            ;;
        after)
            [[ $# -eq 2 && "${2}" =~ ^[0-9]{1,2}$ ]] || usage
            m="$((10#${2}))"
            ((m >= 1 && m <= 30)) || usage
            gsettings set "${schema}" after "$((m * 60))"
            echo "$(eval_gettext "The resting screen comes after \${m} minutes alone.")"
            ;;
        *) usage ;;
    esac

# Living wallpapers (R6-07, offered, off): a slow picture in the theme's colours over the wallpaper, moving only while it
# can be seen, and still on battery, in the power saver, under Games and with reduced motion
# (ame desktop living-wallpaper off|aurora|facets|rain|dusk). Without anything it says which
[positional-arguments]
living-wallpaper KIND="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.living
    case "${1:-}" in
        "")
            kind="$(gsettings get "${schema}" wallpaper | tr -d "'")"
            echo "$(eval_gettext "Living wallpaper: \${kind}")"
            ;;
        off)
            gsettings set "${schema}" wallpaper off
            echo "$(gettext "The wallpaper holds still.")"
            ;;
        aurora | facets | rain | dusk)
            gsettings set "${schema}" wallpaper "$1"
            echo "$(gettext "The wallpaper moves slowly while you can see it, and holds still on battery, in the power saver, under Games and with reduced motion.")"
            ;;
        *)
            echo "usage: ame desktop living-wallpaper off|aurora|facets|rain|dusk" >&2
            exit 64
            ;;
    esac

# The share guard (the privacy module): while the whole screen is shared, the windows of chosen apps are covered,
# blurred or with a solid card (ame desktop share-guard blur|card|off), the apps by their launcher's ID
# (ame desktop share-guard apps [APP.desktop...], or reset), and Files windows in the Private folder (private on|off).
# Without anything it says how it stands
[positional-arguments]
share-guard *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    SCHEMA=org.gnome.shell.extensions.amethystora-ui.privacy
    usage() {
        {
            echo "usage: ame desktop share-guard [blur|card|off]"
            echo "       ame desktop share-guard apps [APP.desktop...] | apps reset"
            echo "       ame desktop share-guard private on|off"
        } >&2
        exit 64
    }
    LAUNCHER_DIRS=("${XDG_DATA_HOME:-${HOME}/.local/share}/applications" "${HOME}/.local/share/flatpak/exports/share/applications"
        /var/lib/flatpak/exports/share/applications /usr/local/share/applications /usr/share/applications)
    case "${1:-}" in
        "")
            look="$(gsettings get "${SCHEMA}" share-guard | tr -d "'")"
            apps="$(gsettings get "${SCHEMA}" share-guard-apps | tr -d "[]'" | sed 's/^@as //')"
            echo "$(eval_gettext "Share guard: \${look}")"
            echo "$(eval_gettext "Covered while the screen is shared: \${apps}")"
            if [[ "$(gsettings get "${SCHEMA}" share-guard-private)" == true ]]; then
                echo "$(gettext "Files windows in the Private folder are covered too.")"
            fi
            ;;
        blur | card | off)
            [[ $# -eq 1 ]] || usage
            gsettings set "${SCHEMA}" share-guard "$1"
            case "$1" in
                blur) echo "$(gettext "Share guard: chosen apps are blurred while the screen is shared.")" ;;
                card) echo "$(gettext "Share guard: chosen apps are covered with a card while the screen is shared.")" ;;
                off) echo "$(gettext "Share guard: off. A shared screen shows every window.")" ;;
            esac
            ;;
        private)
            [[ $# -eq 2 ]] || usage
            state="$2"
            case "${state}" in
                on) gsettings set "${SCHEMA}" share-guard-private true ;;
                off) gsettings set "${SCHEMA}" share-guard-private false ;;
                *) usage ;;
            esac
            echo "$(eval_gettext "Files windows in the Private folder covered while sharing: \${state}")"
            ;;
        apps)
            shift
            if [[ "${1:-}" == reset ]]; then
                [[ $# -eq 1 ]] || usage
                gsettings reset "${SCHEMA}" share-guard-apps
                echo "$(gettext "The apps covered while sharing are the image's again.")"
                exit 0
            fi
            apps=("$@")
            if ((${#apps[@]} == 0)); then
                # In a terminal: pick from the launchers a person sees, the ones covered now ticked
                [[ -t 0 ]] || usage
                declare -A seen=()
                labels=() ids=()
                for dir in "${LAUNCHER_DIRS[@]}"; do
                    for file in "${dir}"/*.desktop; do
                        [[ -f "${file}" ]] || continue
                        id="${file##*/}"
                        [[ -z "${seen[${id}]:-}" ]] || continue
                        seen[${id}]=1
                        grep -qx 'NoDisplay=true' "${file}" && continue
                        label="$(sed -n 's/^Name=//p' "${file}" | head -n1)"
                        [[ -n "${label}" ]] || continue
                        # gum's --selected is a list apart by commas: a name with one would tick the wrong app
                        labels+=("${label//,/ } (${id})")
                        ids+=("${id}")
                    done
                done
                current="$(gsettings get "${SCHEMA}" share-guard-apps)"
                selected=()
                for i in "${!ids[@]}"; do
                    [[ "${current}" == *"'${ids[i]}'"* ]] && selected+=("${labels[i]}")
                done
                # Escape or Ctrl+C changes nothing; Enter with nothing picked is a real "none"
                if ! chosen="$(printf '%s\n' "${labels[@]}" | sort | gum choose --no-limit \
                    --header "$(gettext "Which apps are covered while the screen is shared? (space to pick, enter when done)")" \
                    --selected "$(IFS=,; echo "${selected[*]}")")"; then
                    echo "$(gettext "Nothing changed.")"
                    exit 0
                fi
                picked=()
                [[ -z "${chosen}" ]] || mapfile -t picked <<<"${chosen}"
                for label in "${picked[@]}"; do
                    id="${label##*(}"
                    apps+=("${id%)}")
                done
                # The image's apps that are not installed here stay on the list, ready for when they are
                while IFS= read -r id; do
                    [[ -n "${id}" && -z "${seen[${id}]:-}" ]] && apps+=("${id}")
                done < <(tr -d "[]' " <<<"${current#@as }" | tr ',' '\n')
            fi
            ((${#apps[@]} <= 32)) || usage
            list=""
            for app in "${apps[@]}"; do
                [[ "${app}" =~ ^[A-Za-z0-9._+-]+\.desktop$ ]] || usage
                [[ "${list}" == *"'${app}'"* ]] || list+="${list:+, }'${app}'"
            done
            gsettings set "${SCHEMA}" share-guard-apps "@as [${list}]"
            echo "$(eval_gettext "Covered while the screen is shared: \${list}")"
            ;;
        *) usage ;;
    esac

# The shield (the privacy module): how the machine's protection stands, as a tile in quick settings, a mark in the top
# bar, the mark only when something needs a look, or nothing (STATE: tile, mark, attention or off). It switches nothing;
# every line opens Security. Without STATE it says which
[positional-arguments]
shield STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.privacy
    case "${1:-}" in
        "")
            state="$(gsettings get "${schema}" shield | tr -d "'")"
            echo "$(eval_gettext "The shield: \${state}")"
            ;;
        tile | mark | attention | off)
            gsettings set "${schema}" shield "$1"
            case "$1" in
                tile) echo "$(gettext "The shield is a tile in quick settings.")" ;;
                mark) echo "$(gettext "The shield is a mark in the top bar.")" ;;
                attention) echo "$(gettext "The shield shows in the top bar only when something needs a look.")" ;;
                off) echo "$(gettext "The shield is off. Security still says everything.")" ;;
            esac
            ;;
        *)
            echo "usage: ame desktop shield tile|mark|attention|off"
            exit 64
            ;;
    esac

# Network by app (the privacy module): the network meter's menu lists which apps are talking to the network and to
# where, only while it is open; nothing is sent anywhere (STATE: on or off; without it, it flips)
[positional-arguments]
network-apps STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.privacy
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" network-apps)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" network-apps true
            echo "$(gettext "Network by app: on. The network meter's menu lists the apps talking to the network while it is open.")"
            ;;
        off)
            gsettings set "${schema}" network-apps false
            echo "$(gettext "Network by app: off.")"
            ;;
        *)
            echo "usage: ame desktop network-apps on|off"
            exit 64
            ;;
    esac

# Convert pictures in Files (the tools module, plans/ui-ideas/spec-tools.md R5-19; offered and off): Files' Scripts menu
# gains Convert, a picture as PNG, JPEG or WebP, half the size, 1920 pixels wide or several in one PDF, each a new file
# beside it (STATE: on or off; without it, it says which)
[positional-arguments]
files-convert STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.tools
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" convert-in-files)" == true ]]; then
                echo "$(gettext "Convert in Files: on.")"
            else
                echo "$(gettext "Convert in Files: off.")"
            fi
            ;;
        on)
            gsettings set "${schema}" convert-in-files true
            /usr/libexec/amethystora-convert menu on
            echo "$(gettext "Convert in Files: on. Pick pictures in Files, right-click, and choose Scripts › Convert.")"
            ;;
        off)
            gsettings set "${schema}" convert-in-files false
            /usr/libexec/amethystora-convert menu off
            echo "$(gettext "Convert in Files: off.")"
            ;;
        *)
            echo "usage: ame desktop files-convert [on|off]"
            exit 64
            ;;
    esac

# Earlier versions in Files' Scripts menu (the tools module, plans/ui-ideas/r7/spec-files.md R7-33): each version the snapshots
# and the backup keep of a file (STATE: on or off; without it, it says which)
[positional-arguments]
earlier-versions STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.tools
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" earlier-versions)" == true ]]; then
                echo "$(gettext "Earlier versions in Files: on.")"
            else
                echo "$(gettext "Earlier versions in Files: off.")"
            fi
            ;;
        on | off)
            gsettings set "${schema}" earlier-versions "$([[ "$1" == on ]] && echo true || echo false)"
            /usr/libexec/amethystora-back-in-time menu "$1"
            if [[ "$1" == on ]]; then
                echo "$(gettext "Earlier versions in Files: on. Right-click a file in Files, and choose Scripts › Earlier versions.")"
            else
                echo "$(gettext "Earlier versions in Files: off.")"
            fi
            ;;
        *)
            echo "usage: ame desktop earlier-versions [on|off]"
            exit 64
            ;;
    esac

# A file nothing here opens says what it is and offers the app on Flathub that does (R7-40), for this account. Off, GNOME says
# there is no app for it (STATE: on or off; without it, it says which)
[positional-arguments]
open-anything STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.tools
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" open-anything)" == true ]]; then
                echo "$(gettext "A file nothing opens says what it is: on.")"
            else
                echo "$(gettext "A file nothing opens says what it is: off.")"
            fi
            ;;
        on | off)
            # The launcher first: one of the person's own under its name stays, and then the key says what is so
            if ! /usr/libexec/amethystora-open-anything switch "$1"; then
                echo "$(gettext "A launcher of your own has this name, in ~/.local/share/applications, so nothing was changed.")" >&2
                exit 1
            fi
            gsettings set "${schema}" open-anything "$([[ "$1" == on ]] && echo true || echo false)"
            if [[ "$1" == on ]]; then
                echo "$(gettext "A file nothing opens says what it is: on.")"
            else
                echo "$(gettext "A file nothing opens says what it is: off.")"
            fi
            ;;
        *)
            echo "usage: ame desktop open-anything [on|off]"
            exit 64
            ;;
    esac

# Uninstall says what an app leaves behind and takes that to the trash (R7-41). Off, it asks whether to keep or delete the data
# (STATE: on or off; without it, it says which)
[positional-arguments]
uninstall-data STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.tools
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" uninstall-takes-data)" == true ]]; then
                echo "$(gettext "Uninstall takes an app's data too: on.")"
            else
                echo "$(gettext "Uninstall takes an app's data too: off.")"
            fi
            ;;
        on | off)
            gsettings set "${schema}" uninstall-takes-data "$([[ "$1" == on ]] && echo true || echo false)"
            if [[ "$1" == on ]]; then
                echo "$(gettext "Uninstall takes an app's data too: on.")"
            else
                echo "$(gettext "Uninstall takes an app's data too: off.")"
            fi
            ;;
        *)
            echo "usage: ame desktop uninstall-data [on|off]"
            exit 64
            ;;
    esac

# When GNOME's low-space notice fires, say how much can be cleared and that none of it is the person's work (R7-58). Off, GNOME's
# notice shows as before (STATE: on or off; without it, it says which)
[positional-arguments]
low-space STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.tools
    case "${1:-}" in
        "")
            if [[ "$(gsettings get "${schema}" offer-room)" == true ]]; then
                echo "$(gettext "Offer to clear space when the disk runs low: on.")"
            else
                echo "$(gettext "Offer to clear space when the disk runs low: off.")"
            fi
            ;;
        on | off)
            gsettings set "${schema}" offer-room "$([[ "$1" == on ]] && echo true || echo false)"
            if [[ "$1" == on ]]; then
                echo "$(gettext "Offer to clear space when the disk runs low: on.")"
            else
                echo "$(gettext "Offer to clear space when the disk runs low: off.")"
            fi
            ;;
        *)
            echo "usage: ame desktop low-space [on|off]"
            exit 64
            ;;
    esac

# What the clock shows (GNOME's own clock keys): 24 or 12 hours (format 24h|12h), seconds, the weekday, the date and
# week numbers in the calendar (seconds|weekday|date|week-numbers on|off), and a second time zone after it (zone
# AREA/CITY|off, a name of the system's time-zone data such as Asia/Tokyo). Without on or off, it flips
[positional-arguments]
clock *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    IFACE=org.gnome.desktop.interface
    usage() {
        echo "usage: ame desktop clock format 24h|12h"
        echo "       ame desktop clock seconds|weekday|date|week-numbers [on|off]"
        echo "       ame desktop clock zone AREA/CITY|off"
        exit 64
    }
    [[ $# -ge 1 && $# -le 2 ]] || usage
    case "$1" in
        format)
            [[ $# -eq 2 ]] || usage
            case "$2" in
                24h) echo "$(gettext "The clock shows the 24-hour time.")" ;;
                12h) echo "$(gettext "The clock shows the 12-hour time, with AM and PM.")" ;;
                *) usage ;;
            esac
            gsettings set "${IFACE}" clock-format "$2"
            exit 0
            ;;
        zone)
            [[ $# -eq 2 ]] || usage
            if [[ "$2" == off ]]; then
                gsettings set org.gnome.shell.extensions.amethystora-ui.pill second-zone ''
                echo "$(gettext "The clock shows your own time only.")"
                exit 0
            fi
            # A zone of the system's own data, by its name: an area and a place, perhaps one part more
            if [[ ! "$2" =~ ^[A-Za-z]+(/[A-Za-z0-9_+-]{1,40}){1,2}$ || ! -f "/usr/share/zoneinfo/$2" ]]; then
                zone="$2"
                echo "$(eval_gettext "\${zone} is not a time zone this computer knows. Try one such as Europe/Paris or Asia/Tokyo.")" >&2
                exit 1
            fi
            gsettings set org.gnome.shell.extensions.amethystora-ui.pill second-zone "$2"
            city="${2##*/}"
            city="${city//_/ }"
            echo "$(eval_gettext "The time in \${city} shows after the clock.")"
            exit 0
            ;;
        seconds) schema="${IFACE}" key=clock-show-seconds ;;
        weekday) schema="${IFACE}" key=clock-show-weekday ;;
        date) schema="${IFACE}" key=clock-show-date ;;
        week-numbers) schema=org.gnome.desktop.calendar key=show-weekdate ;;
        *) usage ;;
    esac
    state="${2:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" "${key}")" == true ]]; then state=off; else state=on; fi
    fi
    case "$1:${state}" in
        seconds:on) echo "$(gettext "The clock shows seconds.")" ;;
        seconds:off) echo "$(gettext "The clock shows hours and minutes.")" ;;
        weekday:on) echo "$(gettext "The clock shows the day of the week.")" ;;
        weekday:off) echo "$(gettext "The clock leaves out the day of the week.")" ;;
        date:on) echo "$(gettext "The clock shows the date.")" ;;
        date:off) echo "$(gettext "The clock leaves out the date.")" ;;
        week-numbers:on) echo "$(gettext "The calendar shows week numbers.")" ;;
        week-numbers:off) echo "$(gettext "The calendar shows no week numbers.")" ;;
        *) usage ;;
    esac
    if [[ "${state}" == on ]]; then gsettings set "${schema}" "${key}" true; else gsettings set "${schema}" "${key}" false; fi

# How fast animations play: snappy, normal (the image's), relaxed, or off, which stills them without touching GNOME's
# own Animations switch. Reduce motion still turns movement into fades. Without a speed, it asks
[positional-arguments]
animation-speed SPEED="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.motion
    speed="${1:-}"
    if [[ -z "${speed}" ]]; then
        current="$(gsettings get "${schema}" speed | tr -d "'")"
        speed="$(gum choose --header "$(eval_gettext "Animation speed (current: \${current})")" --selected "${current}" snappy normal relaxed off)" || exit 0
    fi
    case "${speed}" in
        snappy) echo "$(gettext "Animations are snappy: shorter than the image's.")" ;;
        normal) echo "$(gettext "Animations play at the image's speed.")" ;;
        relaxed) echo "$(gettext "Animations are relaxed: slower and easier to follow.")" ;;
        off) echo "$(gettext "Animations are off: everything changes at once.")" ;;
        *)
            echo "usage: ame desktop animation-speed snappy|normal|relaxed|off"
            exit 64
            ;;
    esac
    gsettings set "${schema}" speed "${speed}"

# The size of text everywhere, from 90 to 150 percent, apart from the screen's scale (GNOME's text scaling; the
# Amethystora apps follow it). Without a size, it says the current one
[positional-arguments]
text-size PERCENT="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    percent="${1:-}"
    if [[ -z "${percent}" ]]; then
        percent="$(gsettings get org.gnome.desktop.interface text-scaling-factor | awk '{ printf "%d", $1 * 100 + 0.5 }')"
        echo "$(eval_gettext "Text size: \${percent} %")"
        exit 0
    fi
    if [[ ! "${percent}" =~ ^[0-9]{2,3}$ ]] || (( percent < 90 || percent > 150 )); then
        echo "usage: ame desktop text-size 90..150"
        exit 64
    fi
    gsettings set org.gnome.desktop.interface text-scaling-factor "$(awk -v p="${percent}" 'BEGIN { printf "%.2f", p / 100 }')"
    echo "$(eval_gettext "Text size: \${percent} %. Open apps that are not Amethystora's take it at their next start.")"

# The light desktop: solid glass, short and simple motion and no live window previews, for a computer that draws the
# screen without a graphics driver or has little memory (STATE: on or off; without it, it flips)
[positional-arguments]
light-desktop STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.comfort
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" light)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" light true
            echo "$(gettext "The light desktop is on: solid glass, short motion and no live window previews.")"
            ;;
        off)
            gsettings set "${schema}" light false
            echo "$(gettext "The light desktop is off: the glass and the motion are as the image has them.")"
            ;;
        *)
            echo "usage: ame desktop light-desktop on|off"
            exit 64
            ;;
    esac

# How a window opens and closes: pop, slide, fade, or gnome (GNOME's own, the image's). Closing plays it backwards.
# Without a style, it asks
[positional-arguments]
window-motion STYLE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.motion
    style="${1:-}"
    if [[ -z "${style}" ]]; then
        current="$(gsettings get "${schema}" open-close | tr -d "'")"
        style="$(gum choose --header "$(eval_gettext "Opening and closing (current: \${current})")" --selected "${current}" pop slide fade gnome)" || exit 0
    fi
    case "${style}" in
        pop) echo "$(gettext "Pop: a window grows into place from a little smaller, and shrinks away as it closes.")" ;;
        slide) echo "$(gettext "Slide: a window rises a little into place, and sinks away as it closes.")" ;;
        fade) echo "$(gettext "Fade: a window fades in, and out as it closes.")" ;;
        gnome) echo "$(gettext "Windows open and close as GNOME draws it.")" ;;
        *)
            echo "usage: ame desktop window-motion pop|slide|fade|gnome"
            exit 64
            ;;
    esac
    gsettings set "${schema}" open-close "${style}"
