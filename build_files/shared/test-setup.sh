#!/usr/bin/bash
# `ame setup` (/usr/libexec/amethystora-setup, or the copy named first) against stand-ins for flatpak,
# Homebrew, GNOME's settings, amethystora-pkg and the theme, for 20-tests.sh: a file `save` wrote is one
# `apply` finds nothing to do for, `diff` lists everything against a file that names nothing, `apply`
# installs what is missing and takes nothing away unless asked, the tiling's settings are kept as the schema
# types them, and a file that names a command, an address, a key the format does not know or a setting out
# of its range is refused before anything runs. `bake` writes the same file as an image's defaults, which GNOME
# reads back as the file has them, and refuses what the image cannot have.

set -euo pipefail

SETUP="${1:-/usr/libexec/amethystora-setup}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
mkdir -p "${T}/bin" "${T}/home"
export HOME="${T}/home" XDG_CONFIG_HOME="${T}/home/.config" XDG_DATA_HOME="${T}/home/.local/share"
export XDG_STATE_HOME="${T}/home/.local/state"
export PATH="${T}/bin:${PATH}" LOG="${T}/log"
: >"${LOG}"

# The tiling has taken Super+N out of GNOME's focus-active-notification, which is the account's own and was
# not changed by it, and says so in its state file
mkdir -p "${XDG_STATE_HOME}/amethystora"
echo '{"taken": [{"schema": "org.gnome.shell.keybindings", "key": "focus-active-notification",
    "removed": [{"accelerator": "<Super>n", "index": 0}], "wrote": []}]}' >"${XDG_STATE_HOME}/amethystora/tiling.json"

# The tiling's settings a file may hold are amethystora-setup's fixed list, with the type and range of each: they
# are the schema's own (the tiling's on/off, window rules, pictures, workspace names and folders aside), and the
# manifest saves the same keys
TILING=org.gnome.shell.extensions.amethystora-ui.tiling
TABLE="$(sed -n "/^TILING_KEYS='{\$/,/^}'\$/{s/^TILING_KEYS='//; s/^}'\$/}/; p}" "${SETUP}")"
listed="$(jq -r 'keys[]' <<<"${TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${TILING}" |
    grep -vxE 'enabled|winprops|workspace-backgrounds|default-background|workspace-names|workspace-directories' | sort)"
[[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
    { echo "the tiling's keys in TILING_KEYS and in its schema differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
saved="$(sed -n '/^TILING_KEYS=(/,/)$/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" |
    sed 's/^TILING_KEYS=(//; s/)$//' | tr -s ' ' '\n' | sed '/^$/d' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other tiling keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# A range as numbers, since GSettings prints 0.05 as 0.050000000000000003
while read -r key; do
    read -r _ type min max <<<"$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${TILING}" "${key}")"
    jq -e --arg k "${key}" --arg t "${type}" --arg min "${min:-}" --arg max "${max:-}" '.[$k] as [$type, $a, $b]
        | $type == $t and if $min == "" then ($a | type) != "number" else [($a, $b) + 0] == [($min, $max) | tonumber + 0] end' \
        <<<"${TABLE}" >/dev/null || { echo "TILING_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${TABLE}"), the schema as ${type} ${min:-} ${max:-}"; false; }
done <<<"${listed}"
# The pointer's two keys likewise: shake with the schema's three choices, dense a switch, and the manifest saves both
POINTER=org.gnome.shell.extensions.amethystora-ui.pointer
POINTER_TABLE="$(sed -n "s/^POINTER_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
[[ "$(jq -r 'keys | join(" ")' <<<"${POINTER_TABLE}")" == "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${POINTER}" | sort | xargs)" ]]
[[ "$(jq -c .shake <<<"${POINTER_TABLE}")" == "[\"s\",$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${POINTER}" shake |
    sed -n "2,\$p" | tr "'" '"' | jq -sc .)]" ]]
[[ "$(jq -c .dense <<<"${POINTER_TABLE}")" == '["b"]' && "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${POINTER}" dense)" == "type b" ]]
saved="$(sed -n 's/^POINTER_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort | xargs)"
[[ "${saved}" == "$(jq -r 'keys | join(" ")' <<<"${POINTER_TABLE}")" ]]
# The glass's two likewise: material with the schema's three choices, solid a switch, and the manifest saves both
GLASS=org.gnome.shell.extensions.amethystora-ui.glass
GLASS_TABLE="$(sed -n "s/^GLASS_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
[[ "$(jq -r 'keys | join(" ")' <<<"${GLASS_TABLE}")" == "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${GLASS}" | sort | xargs)" ]] ||
    { echo "GLASS_KEYS and the glass's schema differ"; false; }
[[ "$(jq -c .material <<<"${GLASS_TABLE}")" == "[\"s\",$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${GLASS}" material |
    sed -n "2,\$p" | tr "'" '"' | jq -sc .)]" ]] || { echo "GLASS_KEYS' material has other choices than the schema"; false; }
[[ "$(jq -c .solid <<<"${GLASS_TABLE}")" == '["b"]' && "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${GLASS}" solid)" == "type b" ]]
# Each glass's blur and tint (R6-28), with the schema's range
for key in blur-frost blur-prism blur-clear tint-frost tint-prism tint-clear; do
    want="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${GLASS}" "${key}" | xargs | awk '{print "[\"d\"," $3 + 0 "," $4 + 0 "]"}')"
    [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${GLASS_TABLE}")" == "${want}" ]] || { echo "GLASS_KEYS' ${key} is not the schema's"; false; }
done
saved="$(sed -n 's/^GLASS_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort | xargs)"
[[ "${saved}" == "$(jq -r 'keys | join(" ")' <<<"${GLASS_TABLE}")" ]] || { echo "the manifest saves other glass keys"; false; }
# The motion's two likewise, each with the schema's choices, and the manifest saves both
MOTION=org.gnome.shell.extensions.amethystora-ui.motion
MOTION_TABLE="$(sed -n "s/^MOTION_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
[[ "$(jq -r 'keys | join(" ")' <<<"${MOTION_TABLE}")" == "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${MOTION}" | sort | xargs)" ]] ||
    { echo "MOTION_KEYS and the motion's schema differ"; false; }
for key in minimize workspace-switch speed open-close; do
    [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${MOTION_TABLE}")" == "[\"s\",$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${MOTION}" "${key}" |
        sed -n "2,\$p" | tr "'" '"' | jq -sc .)]" ]] || { echo "MOTION_KEYS' ${key} has other choices than the schema"; false; }
done
saved="$(sed -n 's/^MOTION_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort | xargs)"
[[ "${saved}" == "$(jq -r 'keys | join(" ")' <<<"${MOTION_TABLE}")" ]] || { echo "the manifest saves other motion keys"; false; }
# The top bar's workspaces: auto-names, sets and resume, three switches, and none of its two keybindings
WORKSPACES=org.gnome.shell.extensions.amethystora-ui.workspaces
WORKSPACES_TABLE="$(sed -n "s/^WORKSPACES_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
[[ "$(jq -r 'keys | join(" ")' <<<"${WORKSPACES_TABLE}")" == "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${WORKSPACES}" |
    grep -vx 'move-workspace-left\|move-workspace-right' | sort | xargs)" ]] || { echo "WORKSPACES_KEYS and the schema differ"; false; }
for key in auto-names sets resume; do
    [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${WORKSPACES_TABLE}")" == '["b"]' &&
        "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${WORKSPACES}" "${key}")" == "type b" ]]
done
saved="$(sed -n 's/^WORKSPACES_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort | xargs)"
[[ "${saved}" == "$(jq -r 'keys | join(" ")' <<<"${WORKSPACES_TABLE}")" ]] || { echo "the manifest saves other workspaces keys"; false; }
# The windows module's keys likewise, every one but its keybinding: each with the schema's type, range and choices,
# and the manifest saves the same keys
WINDOWS=org.gnome.shell.extensions.amethystora-ui.windows
WINDOWS_TABLE="$(sed -n "/^WINDOWS_KEYS='{\$/,/^}'\$/{s/^WINDOWS_KEYS='//; s/^}'\$/}/; p}" "${SETUP}")"
listed="$(jq -r 'keys[]' <<<"${WINDOWS_TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${WINDOWS}" |
    grep -vxE 'snap-chooser|drop-down-key|keep-on-top-key|reopen-key' | sort)"
[[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
    { echo "WINDOWS_KEYS and the windows module's schema differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
while read -r key; do
    range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${WINDOWS}" "${key}" | xargs)"
    case "${range}" in
        "type b") want='["b"]' ;;
        "range i "*) want="$(awk '{print "[\"i\"," $3 "," $4 "]"}' <<<"${range}")" ;;
        # The apps whose windows open see-through, each a launcher ID with a percentage
        "type a{si}") want='["opacity"]' ;;
        enum*) want="[\"s\",$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${WINDOWS}" "${key}" |
            sed -n "2,\$p" | tr "'" '"' | jq -sc .)]" ;;
        *) want="? ${range}" ;;
    esac
    [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${WINDOWS_TABLE}")" == "${want}" ]] ||
        { echo "WINDOWS_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${WINDOWS_TABLE}"), the schema as ${want}"; false; }
done <<<"${listed}"
saved="$(sed -n '/^WINDOWS_KEYS=(/,/)$/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" |
    sed 's/^WINDOWS_KEYS=(//; s/)$//' | tr -s ' ' '\n' | sed '/^$/d' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other windows keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# The dock's keys likewise, all of them: a switch, a choice with the schema's choices, the amount with its range, the
# folders and the stacks with checks of their own; and the manifest saves the same keys
DOCK=org.gnome.shell.extensions.amethystora-ui.dock
DOCK_TABLE="$(sed -n "/^DOCK_KEYS='{\$/,/^}'\$/{s/^DOCK_KEYS='//; s/^}'\$/}/; p}" "${SETUP}")"
listed="$(jq -r 'keys[]' <<<"${DOCK_TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${DOCK}" | sort)"
[[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
    { echo "DOCK_KEYS and the dock's schema differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
while read -r key; do
    range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${DOCK}" "${key}" | xargs)"
    case "${key}:${range}" in
        *":type b") want='["b"]' ;;
        *":range d "*) want="$(awk '{print "[\"d\"," $3 + 0 "," $4 + 0 "]"}' <<<"${range}")" ;;
        *":range i "*) want="$(awk '{print "[\"i\"," $3 "," $4 "]"}' <<<"${range}")" ;;
        *":enum"*) want="[\"s\",$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${DOCK}" "${key}" |
            sed -n "2,\$p" | tr "'" '"' | jq -sc .)]" ;;
        "folders:type a{sas}") want='["folders"]' ;;
        "stacks:type as") want='["as","stacks"]' ;;
        *) want="? ${range}" ;;
    esac
    [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${DOCK_TABLE}")" == "${want}" ]] ||
        { echo "DOCK_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${DOCK_TABLE}"), the schema as ${want}"; false; }
done <<<"${listed}"
saved="$(sed -n 's/^DOCK_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other dock keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# The live pill's keys likewise, all of them: switches, and the digest's times with a check of their own; and the
# manifest saves the same keys
PILL=org.gnome.shell.extensions.amethystora-ui.pill
PILL_TABLE="$(sed -n "/^PILL_KEYS='{\$/,/^}'\$/{s/^PILL_KEYS='//; s/^}'\$/}/; p}" "${SETUP}")"
listed="$(jq -r 'keys[]' <<<"${PILL_TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${PILL}" | sort)"
[[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
    { echo "PILL_KEYS and the pill's schema differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
while read -r key; do
    range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${PILL}" "${key}" | xargs)"
    case "${key}:${range}" in
        *":type b") want='["b"]' ;;
        "digest-times:type as" | "later-times:type as") want='["as","times"]' ;;
        "digest-through:type as" | "digest-wait:type as") want='["as","launchers"]' ;;
        "second-zone:type s") want='["s-zone"]' ;;
        *) want="? ${range}" ;;
    esac
    [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${PILL_TABLE}")" == "${want}" ]] ||
        { echo "PILL_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${PILL_TABLE}"), the schema as ${want}"; false; }
done <<<"${listed}"
saved="$(sed -n 's/^PILL_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other pill keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# Quick settings' keys likewise: both lists of tiles' names, with a check of their own; and the manifest saves them
QUICK=org.gnome.shell.extensions.amethystora-ui.quick
QUICK_TABLE="$(sed -n "s/^QUICK_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
listed="$(jq -r 'keys[]' <<<"${QUICK_TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${QUICK}" | sort)"
[[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
    { echo "QUICK_KEYS and quick settings' schema differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
while read -r key; do
    range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${QUICK}" "${key}" | xargs)"
    [[ "${range}" == "type as" && "$(jq -c --arg k "${key}" '.[$k]' <<<"${QUICK_TABLE}")" == '["as","ids"]' ]] ||
        { echo "QUICK_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${QUICK_TABLE}"), the schema as ${range}"; false; }
done <<<"${listed}"
saved="$(sed -n 's/^QUICK_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other quick settings keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# A top bar on every screen, today's tasks, GNOME's rest reminders and the rounded screen corners likewise: every key of
# their schemas but the weather, which sends the place chosen in Weather and is never in a file; and the manifest saves
# the same keys
for table in BARS:org.gnome.shell.extensions.amethystora-ui.bars TODAY:org.gnome.shell.extensions.amethystora-ui.today \
    BREAKS:org.gnome.desktop.break-reminders BEZEL:org.gnome.shell.extensions.amethystora-ui.bezel \
    HINTS:org.gnome.shell.extensions.amethystora-ui.hints APPGRID:org.gnome.shell.extensions.amethystora-ui.appgrid \
    POWER:org.gnome.shell.extensions.amethystora-ui.power \
    SHELF:org.gnome.shell.extensions.amethystora-ui.shelf DESKS:org.gnome.shell.extensions.amethystora-ui.desks \
    BACKUPDISK:org.gnome.shell.extensions.amethystora-ui.backupdisk; do
    name="${table%%:*}" id="${table#*:}"
    TABLE="$(sed -n "s/^${name}_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
    listed="$(jq -r 'keys[]' <<<"${TABLE}" | sort)"
    # Never the app grid's names and icons of the account's own: a name is the person's text, a picture this machine's
    schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${id}" | grep -vx weather |
        { if [[ "${name}" == APPGRID ]]; then grep -vxE 'names|icons'; else cat; fi; } | sort)"
    [[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
        { echo "${name}_KEYS and ${id} differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
    while read -r key; do
        range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${id}" "${key}" | xargs)"
        case "${key}:${range}" in
            *":type b") want='["b"]' ;;
            "selected-breaks:type as") want='["as","breaks"]' ;;
            "hidden:type as") want='["as","launchers"]' ;;
            "layout:type a{sas}") want='["bar-layout"]' ;;
            *":enum "*) want="$(jq -cn --arg r "${range#enum }" '["s", ($r | split(" "))]')" ;;
            *) want="? ${range}" ;;
        esac
        [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${TABLE}")" == "${want}" ]] ||
            { echo "${name}_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${TABLE}"), the schema as ${want}"; false; }
    done <<<"${listed}"
    saved="$(sed -n "s/^${name}_KEYS=(\(.*\))\$/\1/p" "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
    [[ "${saved}" == "${listed}" ]] ||
        { echo "the manifest saves other ${name} keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
done
# GNOME's window settings (WM_KEYS, MUTTER_KEYS): each a key GNOME's schema has, a choice with exactly the schema's
# choices (the drag key the two the recipe offers), never attach-modal-dialogs, which the tiling writes; and the manifest
# saves the same keys
eval "$(sed -n '/^WM=org/,/^MUTTER_KEYS=/p' "${SETUP}")"
for table in WM:org.gnome.desktop.wm.preferences MUTTER:org.gnome.mutter; do
    name="${table%%:*}" id="${table#*:}"
    TABLE="$(eval "echo \"\${${name}_KEYS}\"" | jq -c .)"
    [[ -n "${TABLE}" ]] || { echo "${name}_KEYS is not JSON"; false; }
    jq -e 'has("attach-modal-dialogs") | not' <<<"${TABLE}" >/dev/null
    while read -r key; do
        range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${id}" "${key}" | xargs)"
        case "${key}:${range}" in
            *":type b") want='["b"]' ;;
            "mouse-button-modifier:type s") want='["s",["<Super>","<Alt>"]]' ;;
            *":enum "*) want="$(jq -cn --arg r "${range#enum }" '["s", ($r | split(" "))]')" ;;
            *) want="? ${range}" ;;
        esac
        [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${TABLE}")" == "${want}" ]] ||
            { echo "${name}_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${TABLE}"), the schema as ${want}"; false; }
    done < <(jq -r 'keys[]' <<<"${TABLE}")
    saved="$(awk -v start="${name}_KEYS=(" 'index($0, start) == 1 { on = 1 } on { print; if (/\)$/) exit }' \
        "$(dirname "${SETUP}")/amethystora-setup-manifest" | sed "s/^${name}_KEYS=(//; s/)\$//" | tr -s ' ' '\n' |
        sed '/^$/d' | sort)"
    [[ "${saved}" == "$(jq -r 'keys[]' <<<"${TABLE}" | sort)" ]] ||
        { echo "the manifest saves other ${name} keys than amethystora-setup reads"; false; }
done
# The desktop's sounds: GNOME's switch for them alone, never the sound theme's name (Settings points it at a folder of
# the account's), and the manifest saves the same key
[[ "$(sed -n "s/^SOUND_KEYS='\(.*\)'\$/\1/p" "${SETUP}" | jq -c .)" == '{"event-sounds":["b"]}' ]]
[[ "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range org.gnome.desktop.sound event-sounds | xargs)" == "type b" ]]
[[ "$(sed -n 's/^SOUND_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest")" == "event-sounds" ]]
# The keyboard GNOME keeps out of sight (spec-keys): exactly these keys of four schemas, each of the type its schema gives
# it (the repeat delay a uint32), never accents or the input sources themselves, and the manifest saves the same keys
for part in "INPUT org.gnome.desktop.input-sources per-window:b xkb-options:as" \
    "KEYBOARD org.gnome.desktop.peripherals.keyboard remember-numlock-state:b numlock-state:b delay:u" \
    "PASTE org.gnome.desktop.interface gtk-enable-primary-paste:b" \
    "KEYS org.gnome.shell.extensions.amethystora-ui.keys super-alone:s"; do
    read -r name schema_id pairs <<<"${part}"
    TABLE="$(sed -n "s/^${name}_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
    listed="$(jq -r 'keys[]' <<<"${TABLE}" | sort | paste -sd ' ')"
    wanted="$(tr ' ' '\n' <<<"${pairs}" | cut -d: -f1 | sort | paste -sd ' ')"
    [[ "${listed}" == "${wanted}" ]] || { echo "${name}_KEYS holds ${listed}, not ${wanted}"; false; }
    for pair in ${pairs}; do
        range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${schema_id}" "${pair%%:*}" | sed -n 1p | xargs)"
        [[ "${range}" == "type ${pair#*:}" || ("${pair#*:}" == s && "${range}" == enum) ]] ||
            { echo "${schema_id} ${pair%%:*} is ${range}"; false; }
    done
    saved="$(sed -n "s/^${name}_KEYS=(\(.*\))\$/\1/p" "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort | paste -sd ' ')"
    [[ "${saved}" == "${listed}" ]] || { echo "the manifest saves ${saved} of ${schema_id}, not ${listed}"; false; }
done
# GNOME's clock, text size and calendar week numbers: a few keys of GNOME's own schemas, each of the type and within the
# range GNOME gives it, and the manifest saves the same keys
for part in "INTERFACE org.gnome.desktop.interface" "CALENDAR org.gnome.desktop.calendar"; do
    read -r name schema_id <<<"${part}"
    table="$(sed -n "s/^${name}_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
    [[ -n "${table}" ]] || { echo "no ${name}_KEYS"; false; }
    while read -r key; do
        range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${schema_id}" "${key}" | xargs)"
        spec="$(jq -c --arg k "${key}" '.[$k]' <<<"${table}")"
        case "${range}" in
            "type b") [[ "${spec}" == '["b"]' ]] ;;
            "enum "*) [[ "$(jq -c '.[1] | sort' <<<"${spec}")" == "$(jq -cn --arg r "${range#enum }" '$r | split(" ") | sort')" ]] ;;
            "range d "*) read -r _ _ low high <<<"${range}"
                jq -e --argjson low "${low}" --argjson high "${high}" '.[0] == "d" and .[1] >= $low and .[2] <= $high' <<<"${spec}" >/dev/null ;;
            *) false ;;
        esac || { echo "${name}_KEYS has ${key} as ${spec}, GNOME as ${range}"; false; }
    done < <(jq -r 'keys[]' <<<"${table}")
    saved="$(sed -n "s/^${name}_KEYS=(\(.*\))\$/\1/p" "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
    [[ "${saved}" == "$(jq -r 'keys[]' <<<"${table}" | sort)" ]] || { echo "the manifest saves other ${name} keys"; false; }
done
# The search's switches, the emoji picker's skin tone and the not-responding question likewise, each schema less its
# keybinding (open, emoji-picker, force-quit), which the keybindings part keeps, and the audio module's switches less
# presets, which name this machine's outputs; and the manifest saves the same keys
for part in "SEARCH org.gnome.shell.extensions.amethystora-ui.search open" \
    "EMOJI org.gnome.shell.extensions.amethystora-ui.emoji emoji-picker" \
    "QUIT org.gnome.shell.extensions.amethystora-ui.quit force-quit" \
    "AUDIO org.gnome.shell.extensions.amethystora-ui.audio presets"; do
    read -r name schema_id binding <<<"${part}"
    table="$(sed -n "/^${name}_KEYS='{\$/,/^}'\$/{s/^${name}_KEYS='//; s/^}'\$/}/; p}" "${SETUP}")"
    [[ -n "${table}" ]] || table="$(sed -n "s/^${name}_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
    listed="$(jq -r 'keys[]' <<<"${table}" | sort)"
    schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${schema_id}" | grep -vx "${binding}" | sort)"
    [[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
        { echo "${name}_KEYS and ${schema_id} differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
    while read -r key; do
        range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${schema_id}" "${key}" | xargs)"
        case "${range}" in
            "type b") want='["b"]' ;;
            enum*) want="[\"s\",$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${schema_id}" "${key}" |
                sed -n "2,\$p" | tr "'" '"' | jq -sc .)]" ;;
            *) want="? ${range}" ;;
        esac
        [[ "$(jq -c --arg k "${key}" '.[$k]' <<<"${table}")" == "${want}" ]] ||
            { echo "${name}_KEYS has ${key} as $(jq -c --arg k "${key}" '.[$k]' <<<"${table}"), the schema as ${want}"; false; }
    done <<<"${listed}"
    saved="$(sed -n "s/^${name}_KEYS=(\(.*\))\$/\1/p" "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
    [[ "${saved}" == "${listed}" ]] ||
        { echo "the manifest saves other ${name} keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
done
# The look: the theme module's keys are LOOK_KEYS' schema keys (the rest, the drawn wallpaper's pattern among them, are the account's files), its accents are the
# twelve of the theme library's ACCENTS and theme, and the manifest saves the module's same keys
LOOK_TABLE="$(bash -c "$(sed -n '/^LOOK_ACCENTS=/p; /^LOOK_KEYS=/,/^}'"'"'$/p' "${SETUP}")"'; printf "%s" "${LOOK_KEYS}"')"
jq -e '.accent and .daylight and .colours' <<<"${LOOK_TABLE}" >/dev/null || { echo "LOOK_KEYS cannot be read"; false; }
listed="$(jq -r 'keys[] | select(. != "accent" and . != "daylight" and . != "colours" and . != "pattern")' <<<"${LOOK_TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys org.gnome.shell.extensions.amethystora-ui.theme | sort)"
[[ "${listed}" == "${schema}" ]] ||
    { echo "LOOK_KEYS and the theme module's schema differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
ids="$(bash -c 'source /usr/lib/amethystora/theme/lib.sh; for e in "${ACCENTS[@]}"; do echo "${e%% *}"; done; echo theme')"
[[ "$(jq -r '.accent[1][]' <<<"${LOOK_TABLE}")" == "${ids}" ]] ||
    { echo "LOOK_KEYS' accents are not the theme library's"; false; }
saved="$(sed -n 's/^THEME_UI_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other theme module keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# Focus profiles: every key of the schema but what is on now (profile, ends, off-at) and a profile's saved set, each of
# the schema's type and, for a choice, the schema's choices; the apps are launchers; and the manifest saves the same keys
FOCUS_ID=org.gnome.shell.extensions.amethystora-ui.focus
FOCUS_TABLE="$(bash -c "$(sed -n '/^LOOK_ACCENTS=/p; /^FOCUS_KEYS=/,/^}'"'"'$/p' "${SETUP}")"'; printf "%s" "${FOCUS_KEYS}"')"
listed="$(jq -r 'keys[]' <<<"${FOCUS_TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${FOCUS_ID}" | grep -vxE 'profile|ends|off-at|.*-set' | sort)"
[[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
    { echo "FOCUS_KEYS and ${FOCUS_ID} differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
while read -r key; do
    range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${FOCUS_ID}" "${key}" | tr '\n' ' ' | xargs)"
    case "${range}" in
        "type b") want='["b"]' ;;
        "type as") want='["as","launchers"]' ;;
        enum*) want="$(jq -cn --arg r "${range#enum }" '["s", ($r | split(" "))]')" ;;
        *) want="? ${range}" ;;
    esac
    got="$(jq -c --arg k "${key}" '.[$k] | if .[0] == "s" then [.[0], (.[1] | sort)] else . end' <<<"${FOCUS_TABLE}")"
    want="$(jq -c 'if .[0] == "s" then [.[0], (.[1] | sort)] else . end' <<<"${want}" 2>/dev/null || echo "${want}")"
    [[ "${got}" == "${want}" ]] || { echo "FOCUS_KEYS has ${key} as ${got}, the schema as ${want}"; false; }
done <<<"${listed}"
saved="$(sed -n 's/^FOCUS_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other focus keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# Seeing and reading, and touch and pen: every key of their schemas but whether a filter or the ruler is drawn this moment
# and the shortcuts, each of the schema's type, range or choices; the pen's tools by name. Present mode, the screenshot's
# thumbnail and pins, and widgets on the desktop likewise: every key but Scribble's keybinding and the drawing tools'
# colours. The manifest saves the same keys.
for table in 'ACCESS:org.gnome.shell.extensions.amethystora-ui.access:filter-on|ruler-shown|.*-key' \
    'TOUCH:org.gnome.shell.extensions.amethystora-ui.touch:filter-on|ruler-shown|.*-key' \
    PRESENT:org.gnome.shell.extensions.amethystora-ui.present:scribble \
    SCREENSHOT:org.gnome.shell.extensions.amethystora-ui.screenshot:tool-settings \
    DESKWIDGETS:org.gnome.shell.extensions.amethystora-ui.deskwidgets:- \
    REST:org.gnome.shell.extensions.amethystora-ui.rest:- \
    LIVING:org.gnome.shell.extensions.amethystora-ui.living:-; do
    IFS=: read -r name id left_out <<<"${table}"
    TABLE="$(sed -n "s/^${name}_KEYS='\(.*\)'\$/\1/p" "${SETUP}")"
    # A table over several lines is read as bash reads it
    [[ -n "${TABLE}" ]] ||
        TABLE="$(bash -c "$(sed -n "/^${name}_KEYS='/,/'\$/p" "${SETUP}")"'; printf "%s" "${'"${name}"'_KEYS}"')"
    listed="$(jq -r 'keys[]' <<<"${TABLE}" | sort)"
    schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${id}" | grep -vxE -- "${left_out}" | sort)"
    [[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
        { echo "${name}_KEYS and ${id} differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
    while read -r key; do
        range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${id}" "${key}" | tr '\n' ' ' | xargs)"
        case "${range}" in
            "type b") want='["b"]' ;;
            "type as") want='["as","pen-tools"]' ;;
            "range i "*) want="$(jq -cn --arg r "${range#range i }" '["i"] + ($r | split(" ") | map(tonumber))')" ;;
            enum*) want="$(jq -cn --arg r "${range#enum }" '["s", ($r | split(" ") | sort)]')" ;;
            *) want="? ${range}" ;;
        esac
        got="$(jq -c --arg k "${key}" '.[$k] | if .[0] == "s" then [.[0], (.[1] | sort)] else . end' <<<"${TABLE}")"
        [[ "${got}" == "${want}" ]] || { echo "${name}_KEYS has ${key} as ${got}, the schema as ${want}"; false; }
    done <<<"${listed}"
    saved="$(sed -n "s/^${name}_KEYS=(\(.*\))\$/\1/p" "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
    [[ "${saved}" == "${listed}" ]] ||
        { echo "the manifest saves other ${name} keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
done
# The privacy module: every key of the schema (walk-away lock is a security switch, not one of them), each of the
# schema's type and choices, the covered apps launchers; and the manifest saves the same keys
PRIVACY_ID=org.gnome.shell.extensions.amethystora-ui.privacy
PRIVACY_TABLE="$(bash -c "$(sed -n '/^PRIVACY_KEYS=/,/^}'"'"'$/p' "${SETUP}")"'; printf "%s" "${PRIVACY_KEYS}"')"
listed="$(jq -r 'keys[]' <<<"${PRIVACY_TABLE}" | sort)"
schema="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys "${PRIVACY_ID}" | sort)"
[[ -n "${listed}" && "${listed}" == "${schema}" ]] ||
    { echo "PRIVACY_KEYS and ${PRIVACY_ID} differ:"; diff <(echo "${listed}") <(echo "${schema}"); false; }
while read -r key; do
    range="$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range "${PRIVACY_ID}" "${key}" | tr '\n' ' ' | xargs)"
    case "${range}" in
        "type b") want='["b"]' ;;
        "type as") want='["as","launchers"]' ;;
        enum*) want="$(jq -cn --arg r "${range#enum }" '["s", ($r | split(" "))]')" ;;
        *) want="? ${range}" ;;
    esac
    got="$(jq -c --arg k "${key}" '.[$k] | if .[0] == "s" then [.[0], (.[1] | sort)] else . end' <<<"${PRIVACY_TABLE}")"
    want="$(jq -c 'if .[0] == "s" then [.[0], (.[1] | sort)] else . end' <<<"${want}" 2>/dev/null || echo "${want}")"
    [[ "${got}" == "${want}" ]] || { echo "PRIVACY_KEYS has ${key} as ${got}, the schema as ${want}"; false; }
done <<<"${listed}"
saved="$(sed -n 's/^PRIVACY_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest" | tr ' ' '\n' | sort)"
[[ "${saved}" == "${listed}" ]] ||
    { echo "the manifest saves other privacy keys than amethystora-setup reads:"; diff <(echo "${saved}") <(echo "${listed}"); false; }
# The tools module: Convert in Files, the schema's one key besides its two keybindings (the keybindings part keeps those),
# and the manifest saves the same key
[[ "$(sed -n "s/^TOOLS_KEYS='\(.*\)'\$/\1/p" "${SETUP}" | jq -c .)" == '{"convert-in-files":["b"]}' ]]
[[ "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings list-keys org.gnome.shell.extensions.amethystora-ui.tools |
    grep -vxE 'colour-picker|mute-microphone' | sort | xargs)" == "convert-in-files" ]]
[[ "$(GSETTINGS_BACKEND=memory /usr/bin/gsettings range org.gnome.shell.extensions.amethystora-ui.tools convert-in-files | xargs)" == "type b" ]]
[[ "$(sed -n 's/^TOOLS_KEYS=(\(.*\))$/\1/p' "$(dirname "${SETUP}")/amethystora-setup-manifest")" == "convert-in-files" ]]

# A machine with two apps, three things from Homebrew, a container, an extension and a theme, whose
# dock, close key, terminal key, tiling key, two tiling settings and window rules are not the image's. What
# would change it is only logged.
cat >"${T}/bin/stub" <<'EOF'
#!/usr/bin/bash
CUSTOM=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings
case "${0##*/}" in
    flatpak)
        case "$*" in
            "list --system --app --columns=application,origin") printf 'org.gnome.Calculator\tflathub\n' ;;
            "list --user --app --columns=application,origin") printf 'org.gimp.GIMP\tflathub\n' ;;
            "remotes --system --columns=name") echo flathub ;;
            remotes*) ;;
            *) echo "flatpak $*" >>"${LOG}" ;;
        esac
        ;;
    brew)
        if [[ "$*" == "bundle dump --file=-" ]]; then
            printf 'tap "homebrew/bundle"\ntap "someone/tools"\nbrew "ripgrep"\ncask "font-iosevka"\n'
        else
            echo "brew $*" >>"${LOG}"
        fi
        ;;
    gsettings)
        defaults="org.gnome.shell favorite-apps ['firefox.desktop']
org.gnome.shell enabled-extensions @as []
org.gnome.desktop.wm.keybindings close ['<Super>w']
org.gnome.desktop.wm.keybindings maximize ['<Super>Up']
org.gnome.shell.keybindings focus-active-notification ['<Super>n']
org.gnome.shell.extensions.amethystora-ui.tiling.keybindings cycle-width ['<Super>r']
org.gnome.shell.extensions.amethystora-ui.tiling enabled true
org.gnome.shell.extensions.amethystora-ui.tiling window-gap 8
org.gnome.shell.extensions.amethystora-ui.tiling animation-time 0.25
org.gnome.shell.extensions.amethystora-ui.tiling cycle-width-steps [0.38195000000000001, 0.5, 0.61804000000000003]
org.gnome.shell.extensions.amethystora-ui.tiling winprops @as []
org.gnome.shell.extensions.amethystora-ui.pointer shake 'off'
org.gnome.shell.extensions.amethystora-ui.pointer dense true
org.gnome.shell.extensions.amethystora-ui.glass material 'prism'
org.gnome.shell.extensions.amethystora-ui.glass solid false
org.gnome.shell.extensions.amethystora-ui.windows snap-chooser ['<Super>z']
org.gnome.shell.extensions.amethystora-ui.windows corner-top-right 'none'
org.gnome.shell.extensions.amethystora-ui.windows hold-super-delay 1000
org.gnome.shell.extensions.amethystora-ui.windows dim false
org.gnome.shell.extensions.amethystora-ui.dock magnify 'swell'
org.gnome.shell.extensions.amethystora-ui.dock folders @a{sas} {}
org.gnome.shell.extensions.amethystora-ui.dock stacks ['xdg:DOWNLOAD']
org.gnome.shell.extensions.amethystora-ui.bezel round-corners false
org.gnome.shell.extensions.amethystora-ui.audio pause-on-unplug true
org.gnome.shell.extensions.amethystora-ui.access filter 'none'
org.gnome.shell.extensions.amethystora-ui.access filter-strength 100
org.gnome.shell.extensions.amethystora-ui.access read-aloud false
org.gnome.shell.extensions.amethystora-ui.touch touch-mode 'off'
org.gnome.shell.extensions.amethystora-ui.touch pen-ring-tools ['scribble', 'area', 'note', 'text', 'right-click', 'back']
org.gnome.desktop.sound event-sounds true
org.gnome.shell.extensions.amethystora-ui.theme accent-per-workspace false
org.gnome.shell.extensions.amethystora-ui.theme workspace-accents ['theme', 'sky', 'coral', 'jade', 'amber', 'orchid']
org.gnome.shell.extensions.amethystora-transitions reveal true
org.gnome.shell.extensions.amethystora-ui.hints enabled true
org.gnome.shell.extensions.amethystora-ui.appgrid recent true
org.gnome.shell.extensions.amethystora-ui.appgrid order 'custom'
org.gnome.shell.extensions.amethystora-ui.appgrid hidden @as []
org.gnome.shell.extensions.amethystora-ui.appgrid names {'org.gnome.Calculator.desktop': 'Sums'}
org.gnome.shell.extensions.amethystora-ui.backupdisk ask true
org.gnome.shell.extensions.amethystora-ui.power busy-app true
org.gnome.shell.extensions.amethystora-ui.present enabled false
org.gnome.shell.extensions.amethystora-ui.present pointer-mark 'halo'
org.gnome.shell.extensions.amethystora-ui.present ask-on-screen true
org.gnome.shell.extensions.amethystora-ui.screenshot capture-thumbnail true
org.gnome.shell.extensions.amethystora-ui.screenshot capture-pin true
org.gnome.shell.extensions.amethystora-ui.deskwidgets enabled false
org.gnome.shell.extensions.amethystora-ui.quit frozen-dialog true
org.gnome.shell.extensions.amethystora-ui.privacy shield 'tile'
org.gnome.shell.extensions.amethystora-ui.privacy network-apps false
org.gnome.desktop.input-sources per-window false
org.gnome.desktop.input-sources xkb-options ['grp:alt_shift_toggle']
org.gnome.desktop.peripherals.keyboard remember-numlock-state true
org.gnome.desktop.peripherals.keyboard delay uint32 500
org.gnome.desktop.interface gtk-enable-primary-paste true
org.gnome.shell.extensions.amethystora-ui.keys plain-paste ['<Super><Alt>v']
org.gnome.shell.extensions.amethystora-ui.keys super-alone 'overview'"
        values="org.gnome.shell favorite-apps ['org.gnome.Nautilus.desktop', 'firefox.desktop']
org.gnome.shell enabled-extensions ['clipboard-indicator@tudmotu.com']
org.gnome.desktop.wm.keybindings close ['<Super>q']
org.gnome.shell.keybindings focus-active-notification @as []
org.gnome.shell.extensions.amethystora-ui.tiling.keybindings cycle-width ['<Super>e']
org.gnome.shell.extensions.amethystora-ui.tiling window-gap 12
org.gnome.shell.extensions.amethystora-ui.tiling cycle-width-steps [0.29999999999999999, 0.5]
org.gnome.shell.extensions.amethystora-ui.tiling winprops ['{\"wm_class\": \"Firefox\", \"scratch_layer\": true}']
org.gnome.shell.extensions.amethystora-ui.pointer shake 'beacon'
org.gnome.shell.extensions.amethystora-ui.glass material 'clear'
org.gnome.shell.extensions.amethystora-ui.windows dim true
org.gnome.shell.extensions.amethystora-ui.focus games-power 'performance'
org.gnome.shell.extensions.amethystora-ui.focus trigger-sleep false
org.gnome.shell.extensions.amethystora-ui.focus work-apps ['org.gnome.Calendar.desktop']
org.gnome.shell.extensions.amethystora-ui.shelf shelf 'off'
org.gnome.shell.extensions.amethystora-ui.desks desks false
org.gnome.shell.extensions.amethystora-ui.dock magnify 'lift'
org.gnome.shell.extensions.amethystora-ui.dock stacks ['xdg:DOWNLOAD', '~/Projects', '/srv/share']
org.gnome.shell.extensions.amethystora-ui.dock folders {\"Jeux d'enfants\": ['org.gnome.Nautilus.desktop', 'firefox.desktop']}"
        [[ "${GSETTINGS_BACKEND:-}" == memory ]] && values=""
        value() { grep "^$1 $2 " <<<"${values}" || grep "^$1 $2 " <<<"${defaults}"; }
        case "$1" in
            list-recursively)
                awk -v schema="$2" '$1 == schema { print $2 }' <<<"${defaults}" | while read -r key; do value "$2" "${key}"; done
                ;;
            get)
                if [[ "$2" == *custom-keybinding:* ]]; then echo "'<Super>t'"; else value "$2" "$3" | cut -d' ' -f3-; fi
                ;;
            range) echo "type as" ;;
            *) echo "gsettings $*" >>"${LOG}" ;;
        esac
        ;;
    dconf)
        case "$*" in
            "list ${CUSTOM}/") echo custom20/ ;;
            "read -d ${CUSTOM}/custom20/command") echo "'xdg-terminal-exec'" ;;
            "read -d ${CUSTOM}/custom20/binding") echo "'<Super>Return'" ;;
            "read ${CUSTOM}/custom20/binding") echo "'<Super>t'" ;;
        esac
        ;;
    amethystora-theme)
        case "$1" in
            current) echo nord ;;
            list) printf '  amethyst\n* nord\n' ;;
            *) echo "amethystora-theme $*" >>"${LOG}" ;;
        esac
        ;;
    amethystora-pkg)
        case "$*" in
            "containers list --json") echo '[{"name": "debian", "template": "debian", "init": false, "packages": ["htop"]}]' ;;
            "templates list --json") echo '[{"name": "debian"}, {"name": "fedora"}]' ;;
            "managers list --json") echo '[]' ;;
            *) echo "amethystora-pkg $*" >>"${LOG}" ;;
        esac
        ;;
    *) echo "${0##*/} $*" >>"${LOG}" ;;
esac
EOF
chmod +x "${T}/bin/stub"
for command in flatpak brew gsettings dconf gnome-extensions amethystora-theme amethystora-pkg ame; do
    ln -s stub "${T}/bin/${command}"
done

# save writes the setup, and nothing that is an address or a script: no remote, Brewfile or template
"${SETUP}" save >/dev/null
FILE="${XDG_CONFIG_HOME}/amethystora/setup.json"
jq -e '.version == 1 and .theme == "nord" and .flatpaks.user == [{"app": "org.gimp.GIMP", "origin": "flathub"}]
    and .brew == {"taps": ["someone/tools"], "formulae": ["ripgrep"], "casks": ["font-iosevka"]}
    and .containers == [{"name": "debian", "template": "debian", "init": false, "packages": ["htop"]}]
    and .extensions == ["clipboard-indicator@tudmotu.com"]
    and .settings == {"layout": "tiling", "favorites": ["org.gnome.Nautilus.desktop", "firefox.desktop"],
        "keybindings": {"org.gnome.desktop.wm.keybindings": {"close": ["<Super>q"]}, "custom": {"custom20": ["<Super>t"]},
            "org.gnome.shell.extensions.amethystora-ui.tiling.keybindings": {"cycle-width": ["<Super>e"]}},
        "tiling": {"window-gap": 12, "cycle-width-steps": [0.3, 0.5]}, "pointer": {"shake": "beacon"},
        "glass": {"material": "clear"}, "windows": {"dim": true},
        "dock": {"magnify": "lift", "stacks": ["xdg:DOWNLOAD", "~/Projects"],
            "folders": {"Jeux d'"'"'enfants": ["org.gnome.Nautilus.desktop", "firefox.desktop"]}}}
    and (keys - ["version", "image", "flatpaks", "brew", "containers", "extensions", "theme", "settings"] == [])' \
    "${FILE}" >/dev/null
# ...the tiling's settings as JSON writes them, and not its window rules, which the file never holds
[[ "$(jq -c .settings.tiling "${FILE}")" == '{"cycle-width-steps":[0.3,0.5],"window-gap":12}' ]]
"${SETUP}" save - | cmp -s - "${FILE}"

# Applying what save just wrote does nothing, and says so
grep -q "^Nothing to do" <<<"$("${SETUP}" apply --yes)"
grep -q "^This machine is as " <<<"$("${SETUP}" diff)"
[[ ! -s "${LOG}" ]]

# A file from before the tiling replaced PaperWM still holds its gaps, which are ignored beside the tiling's own
jq '.settings.paperwm = {"window-gap": 20}' "${FILE}" >"${T}/old.json"
grep -q "^Nothing to do" <<<"$("${SETUP}" apply --yes "${T}/old.json")"
[[ ! -s "${LOG}" ]]
# ...and read as the tiling's where the file has none, less a gap the tiling cannot take (PaperWM took up to 1000)
jq 'del(.settings.tiling) | .settings.paperwm = {"window-gap": 20, "horizontal-margin": 1000}' "${FILE}" >"${T}/old-gaps.json"
"${SETUP}" apply --yes "${T}/old-gaps.json" >/dev/null
grep -qx "gsettings set ${TILING} window-gap 20" "${LOG}"
grep -q "horizontal-margin" "${LOG}" && false
: >"${LOG}"
# ...and its keybindings, which this machine has no such keys for: the file is not refused for them, and what it
# holds besides still applies
jq '.settings.keybindings["org.gnome.shell.extensions.paperwm.keybindings"] = {"switch-left": ["<Super>h"]}
    | .flatpaks.user += [{"app": "org.gnome.Maps", "origin": "flathub"}]' "${FILE}" >"${T}/old-keys.json"
"${SETUP}" diff --json "${T}/old-keys.json" | jq -e 'any(.area == "keybinding" and .ok == false)
    and any(.area == "flatpak" and .action == "add")' >/dev/null

# Against a file that names nothing, diff lists everything here, as what apply --remove would take away
echo '{"version": 1}' >"${T}/empty.json"
"${SETUP}" diff --json "${T}/empty.json" | jq -e 'length == 19 and all(.action == "remove")
    and ([.[] | select(.area == "dock") | .key] | sort == ["folders", "magnify", "stacks"])
    and ([.[] | select(.area == "tiling") | .key] | sort == ["cycle-width-steps", "window-gap"])
    and ([.[] | select(.area == "pointer") | .key] == ["shake"])
    and ([.[] | select(.area == "glass") | .key] == ["material"])
    and ([.[] | select(.area == "windows") | .key] == ["dim"])' >/dev/null

# apply installs what is missing, and without --remove takes nothing away. A tiling setting this machine does
# not have is listed and left out.
jq '.flatpaks.system += [{"app": "org.gnome.Maps", "origin": "flathub"}] | .brew.formulae = ["fd", "someone/tools/lint"]
    | .settings.keybindings["org.gnome.desktop.wm.keybindings"].close = ["<Super>c"]
    | .settings.tiling += {"window-gap": 20, "animation-time": 0.5, "workspace-colors": ["", "#314e6c"]}
    | .settings.pointer = {"shake": "off", "dense": false}
    | .settings.glass = {"material": "frost", "solid": true}
    | .settings.windows = {"dim": false, "corner-top-right": "overview", "hold-super-delay": 1500}
    | .settings.dock += {"magnify": "off", "folders": {"Writing": ["org.gnome.Nautilus.desktop", "firefox.desktop"]}}
    | .settings.bezel = {"round-corners": true} | .settings.sound = {"event-sounds": false}
    | .settings.audio = {"pause-on-unplug": false}
    | .settings.transitions = {"reveal": false} | .settings.hints = {"enabled": false}
    | .settings.appgrid = {"recent": false, "order": "kind", "hidden": ["org.gnome.Calculator.desktop"]}
    | .settings.backupdisk = {"ask": false}
    | .settings.power = {"busy-app": false}
    | .settings.focus = {"games-power": "as-set", "trigger-sleep": true, "work-apps": ["org.gnome.Nautilus.desktop"]}
    | .settings.access = {"filter": "deutan", "filter-strength": 75}
    | .settings.touch = {"touch-mode": "follow", "pen-ring-tools": ["area", "back"]}
    | .settings.present = {"enabled": true, "pointer-mark": "torch"} | .settings.screenshot = {"capture-pin": false}
    | .settings.deskwidgets = {"enabled": true}
    | .settings.shelf = {"shelf": "edge"} | .settings.desks = {"desks": true}
    | .settings.quit = {"frozen-dialog": false}
    | .settings.privacy = {"shield": "attention", "network-apps": true}
    | .settings.input = {"per-window": true, "xkb-options": ["grp:alt_shift_toggle", "caps:escape"]}
    | .settings.keyboard = {"delay": 300} | .settings.paste = {"gtk-enable-primary-paste": false}
    | .settings.keys = {"super-alone": "search"}
    | .settings.keybindings["org.gnome.shell.extensions.amethystora-ui.keys"] = {"plain-paste": ["<Super><Shift>v"]}' \
    "${FILE}" >"${T}/more.json"
out="$("${SETUP}" diff "${T}/more.json")"
grep -qxF "  ~ the windows' dim: false, in place of true" <<<"${out}"
grep -qxF '  ~ the windows'"'"' corner-top-right: "overview", in place of "none"' <<<"${out}"
grep -qxF "  ~ the windows' hold-super-delay: 1500, in place of 1000" <<<"${out}"
grep -qxF '  ~ the dock'"'"'s magnify: "off", in place of "lift"' <<<"${out}"
grep -qxF '  ~ the dock'"'"'s folders: {"Writing":["org.gnome.Nautilus.desktop","firefox.desktop"]}, in place of {"Jeux d'"'"'enfants":["org.gnome.Nautilus.desktop","firefox.desktop"]}' <<<"${out}"
grep -qxF '  ~ the pointer'"'"'s shake: "off", in place of "beacon"' <<<"${out}"
grep -qxF "  ~ the pointer's dense: false, in place of true" <<<"${out}"
grep -qxF '  ~ the glass'"'"'s material: "frost", in place of "clear"' <<<"${out}"
grep -qxF "  ~ the glass's solid: true, in place of false" <<<"${out}"
grep -qxF "  ~ the tiling's window-gap: 20, in place of 12" <<<"${out}"
grep -qxF "  ~ the tiling's animation-time: 0.5, in place of 0.25" <<<"${out}"
grep -qxF "  ! the tiling's workspace-colors: [\"\",\"#314e6c\"]: this machine's tiling has no such setting" <<<"${out}"
grep -qxF "  ~ the screen's round-corners: true, in place of false" <<<"${out}"
grep -qxF "  ~ the audio module's pause-on-unplug: false, in place of true" <<<"${out}"
grep -qxF "  ~ the desktop's event-sounds: false, in place of true" <<<"${out}"
grep -qxF "  ~ the transitions' reveal: false, in place of true" <<<"${out}"
grep -qxF "  ~ the tips' enabled: false, in place of true" <<<"${out}"
grep -qxF "  ~ the app grid's recent: false, in place of true" <<<"${out}"
grep -qxF '  ~ the app grid'"'"'s order: "kind", in place of "custom"' <<<"${out}"
grep -qxF '  ~ the app grid'"'"'s hidden: ["org.gnome.Calculator.desktop"], in place of []' <<<"${out}"
grep -qxF "  ~ the backup disk's ask: false, in place of true" <<<"${out}"
grep -qF "app grid's names" <<<"${out}" && { echo "the app grid's names of the account's own reached a setup file's diff"; false; }
grep -qxF "  ~ the power module's busy-app: false, in place of true" <<<"${out}"
grep -qxF '  ~ focus profiles'"'"' games-power: "as-set", in place of "performance"' <<<"${out}"
grep -qxF "  ~ focus profiles' trigger-sleep: true, in place of false" <<<"${out}"
grep -qxF '  ~ seeing and reading'"'"'s filter: "deutan", in place of "none"' <<<"${out}"
grep -qxF "  ~ seeing and reading's filter-strength: 75, in place of 100" <<<"${out}"
grep -qxF '  ~ touch and pen'"'"'s touch-mode: "follow", in place of "off"' <<<"${out}"
grep -qxF "  ~ Present mode's enabled: true, in place of false" <<<"${out}"
grep -qxF '  ~ Present mode'"'"'s pointer-mark: "torch", in place of "halo"' <<<"${out}"
grep -qxF "  ~ the screenshot's capture-pin: false, in place of true" <<<"${out}"
grep -qxF "  ~ the desktop widgets' enabled: true, in place of false" <<<"${out}"
grep -qxF '  ~ the shelf'"'"'s shelf: "edge", in place of "off"' <<<"${out}"
grep -qxF "  ~ the desks' desks: true, in place of false" <<<"${out}"
grep -qxF "  ~ force quit's frozen-dialog: false, in place of true" <<<"${out}"
grep -qxF '  ~ the privacy settings'"'"' shield: "attention", in place of "tile"' <<<"${out}"
grep -qxF "  ~ the privacy settings' network-apps: true, in place of false" <<<"${out}"
# The keyboard GNOME keeps out of sight (spec-keys): each window's layout, the xkb options, the repeat delay (a uint32,
# which GSettings prints with its type), middle-click paste and what Super alone opens
grep -qxF "  ~ the input sources' per-window: true, in place of false" <<<"${out}"
grep -qxF '  ~ the input sources'"'"' xkb-options: ["grp:alt_shift_toggle","caps:escape"], in place of ["grp:alt_shift_toggle"]' <<<"${out}"
grep -qxF "  ~ the keyboard's delay: 300, in place of 500" <<<"${out}"
grep -qxF "  ~ middle-click paste's gtk-enable-primary-paste: false, in place of true" <<<"${out}"
grep -qxF '  ~ the Super key'"'"'s super-alone: "search", in place of "overview"' <<<"${out}"
"${SETUP}" apply --yes "${T}/more.json" >/dev/null
grep -qx "gsettings set org.gnome.desktop.input-sources per-window true" "${LOG}"
grep -qxF 'gsettings set org.gnome.desktop.input-sources xkb-options ["grp:alt_shift_toggle","caps:escape"]' "${LOG}"
grep -qx "gsettings set org.gnome.desktop.peripherals.keyboard delay 300" "${LOG}"
grep -qx "gsettings set org.gnome.desktop.interface gtk-enable-primary-paste false" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.keys super-alone "search"' "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.focus games-power "as-set"' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.focus trigger-sleep true" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.focus work-apps ["org.gnome.Nautilus.desktop"]' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.bezel round-corners true" "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.audio pause-on-unplug false" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.shelf shelf "edge"' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.desks desks true" "${LOG}"
grep -qx "gsettings set org.gnome.desktop.sound event-sounds false" "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-transitions reveal false" "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.hints enabled false" "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.appgrid recent false" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.appgrid order "kind"' "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.appgrid hidden ["org.gnome.Calculator.desktop"]' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.backupdisk ask false" "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.power busy-app false" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.access filter "deutan"' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.access filter-strength 75" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.touch pen-ring-tools ["area","back"]' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.present enabled true" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.present pointer-mark "torch"' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.screenshot capture-pin false" "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.deskwidgets enabled true" "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.quit frozen-dialog false" "${LOG}"
grep -qxF 'gsettings set org.gnome.shell.extensions.amethystora-ui.privacy shield "attention"' "${LOG}"
grep -qx "gsettings set org.gnome.shell.extensions.amethystora-ui.privacy network-apps true" "${LOG}"
grep -qx "flatpak install --system --noninteractive flathub org.gnome.Maps" "${LOG}"
grep -qx "brew install --formula fd someone/tools/lint" "${LOG}"
grep -qx "gsettings set org.gnome.desktop.wm.keybindings close \['<Super>c'\]" "${LOG}"
grep -qx "gsettings set ${TILING} window-gap 20" "${LOG}"
grep -qx "gsettings set ${TILING} animation-time 0.5" "${LOG}"
grep -qxF "gsettings set ${POINTER} shake \"off\"" "${LOG}"
grep -qx "gsettings set ${POINTER} dense false" "${LOG}"
grep -qxF "gsettings set ${GLASS} material \"frost\"" "${LOG}"
grep -qx "gsettings set ${GLASS} solid true" "${LOG}"
grep -qx "gsettings set ${WINDOWS} dim false" "${LOG}"
grep -qxF "gsettings set ${WINDOWS} corner-top-right \"overview\"" "${LOG}"
grep -qx "gsettings set ${WINDOWS} hold-super-delay 1500" "${LOG}"
grep -qxF "gsettings set ${DOCK} magnify \"off\"" "${LOG}"
grep -qxF "gsettings set ${DOCK} folders {\"Writing\":[\"org.gnome.Nautilus.desktop\",\"firefox.desktop\"]}" "${LOG}"
grep -q "${DOCK} stacks" "${LOG}" && false
grep -qE "workspace-colors|cycle-width-steps" "${LOG}" && false
grep -qE "uninstall|untap|reset|rm " "${LOG}" && false
# --remove asks first, which takes a terminal: without one, nothing is taken away
: >"${LOG}"
"${SETUP}" apply --yes --remove "${T}/more.json" </dev/null >/dev/null 2>&1 && false
grep -qE "uninstall|untap|reset|rm " "${LOG}" && false

# A file somebody shared must not run anything: a key the format does not know, a custom keybinding's
# command or the list of them, an app id that would be read as an option, an address for a tap, a formula
# from a tap the file does not list (Homebrew would tap it unannounced); and of the tiling's, a key it does not
# keep (its on/off, which is the layout; its window rules; one it does not have), a value of the wrong type, a
# fraction for a whole number, a number outside the schema's range, a step of 0, a colour that is not #rrggbb
: >"${LOG}"
for bad in '{"version": 1, "token": "x"}' '{"version": 2}' \
    '{"version": 1, "settings": {"keybindings": {"custom": {"custom20": {"command": "rm -rf ~"}}}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.settings-daemon.plugins.media-keys": {"custom-keybindings": ["/x/"]}}}}' \
    '{"version": 1, "settings": {"org.gnome.desktop.background": {"picture-uri": "x"}}}' \
    '{"version": 1, "flatpaks": {"system": [{"app": "--from=https://example.com/x.flatpakref", "origin": "flathub"}]}}' \
    '{"version": 1, "brew": {"taps": ["https://example.com/tap.git"]}}' \
    '{"version": 1, "brew": {"formulae": ["someone/tools/thing"]}}' \
    '{"version": 1, "brew": {"taps": ["someone/other"], "casks": ["someone/tools/thing"]}}' \
    '{"version": 1, "containers": [{"name": "x", "template": "debian", "packages": ["-o APT::Update::Pre-Invoke::=x"]}]}' \
    '{"version": 1, "settings": {"tiling": {"enabled": false}}}' \
    '{"version": 1, "settings": {"tiling": {"winprops": ["{\"wm_class\": \"x\"}"]}}}' \
    '{"version": 1, "settings": {"tiling": {"workspace-backgrounds": ["/tmp/x.png"]}}}' \
    '{"version": 1, "settings": {"tiling": {"no-such-setting": 1}}}' \
    '{"version": 1, "settings": {"tiling": [{"window-gap": 12}]}}' \
    '{"version": 1, "settings": {"tiling": {"window-gap": "12"}}}' \
    '{"version": 1, "settings": {"tiling": {"maximize-within-tiling": 1}}}' \
    '{"version": 1, "settings": {"tiling": {"window-gap": 12.5}}}' \
    '{"version": 1, "settings": {"tiling": {"window-gap": 201}}}' \
    '{"version": 1, "settings": {"tiling": {"maximize-width-percent": 0.4}}}' \
    '{"version": 1, "settings": {"tiling": {"cycle-width-steps": [0.5, 0]}}}' \
    '{"version": 1, "settings": {"tiling": {"swipe-friction": ["0.3"]}}}' \
    '{"version": 1, "settings": {"tiling": {"workspace-colors": ["red"]}}}' \
    '{"version": 1, "settings": {"pointer": {"shake": "spin"}}}' \
    '{"version": 1, "settings": {"motion": {"minimize": "bounce"}}}' \
    '{"version": 1, "settings": {"motion": {"reduce-motion": true}}}' \
    '{"version": 1, "settings": {"workspaces": {"auto-names": "yes"}}}' \
    '{"version": 1, "settings": {"workspaces": {"move-workspace-left": ["<Super>a"]}}}' \
    '{"version": 1, "settings": {"pointer": {"dense": "yes"}}}' \
    '{"version": 1, "settings": {"pointer": {"cursor-size": 48}}}' \
    '{"version": 1, "settings": {"pointer": ["grow"]}}'     '{"version": 1, "settings": {"glass": {"material": "liquid"}}}'     '{"version": 1, "settings": {"glass": {"solid": "on"}}}'     '{"version": 1, "settings": {"glass": {"corners": "round"}}}'     '{"version": 1, "settings": {"comfort": {"reduce-motion": true}}}' \
    '{"version": 1, "settings": {"windows": {"corner-top-left": "rm -rf ~"}}}' \
    '{"version": 1, "settings": {"windows": {"hold-super-delay": 100}}}' \
    '{"version": 1, "settings": {"windows": {"hold-super-delay": 1000.5}}}' \
    '{"version": 1, "settings": {"windows": {"dim": "on"}}}' \
    '{"version": 1, "settings": {"windows": {"snap-chooser": ["<Super>z"]}}}' \
    '{"version": 1, "settings": {"windows": {"places.json": {}}}}' \
    '{"version": 1, "settings": {"windows": {"drop-down-key": ["<Super>Return"]}}}' \
    '{"version": 1, "settings": {"windows": {"see-through-apps": {"kitty.desktop": 20}}}}' \
    '{"version": 1, "settings": {"windows": {"see-through-apps": {"rm -rf ~": 90}}}}' \
    '{"version": 1, "settings": {"windows": {"see-through-apps": ["kitty.desktop"]}}}' \
    '{"version": 1, "settings": {"wm": {"focus-mode": "everywhere"}}}' \
    '{"version": 1, "settings": {"wm": {"mouse-button-modifier": "<Control>"}}}' \
    '{"version": 1, "settings": {"wm": {"button-layout": "close:"}}}' \
    '{"version": 1, "settings": {"mutter": {"attach-modal-dialogs": true}}}' \
    '{"version": 1, "settings": {"mutter": {"center-new-windows": "yes"}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.shell.extensions.amethystora-ui.windows": {"frame": ["<Super>a"]}}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.shell.extensions.amethystora-ui.windows": {"snap": ["<Super>a"]}}}}' \
    '{"version": 1, "settings": {"dock": {"no-such-setting": true}}}' \
    '{"version": 1, "settings": {"dock": {"magnify": "wobble"}}}' \
    '{"version": 1, "settings": {"dock": {"magnify-amount": 3}}}' \
    '{"version": 1, "settings": {"dock": {"folders": {"Lonely": ["firefox.desktop"]}}}}' \
    '{"version": 1, "settings": {"dock": {"folders": {"Bad": ["rm -rf ~", "firefox.desktop"]}}}}' \
    '{"version": 1, "settings": {"dock": {"stacks": ["~/../x"]}}}' \
    '{"version": 1, "settings": {"dock": {"stacks": ["/etc"]}}}' \
    '{"version": 1, "settings": {"pill": {"no-such-setting": true}}}' \
    '{"version": 1, "settings": {"pill": {"quiet": "on"}}}' \
    '{"version": 1, "settings": {"search": {"answers": "on"}}}' \
    '{"version": 1, "settings": {"search": {"open": ["<Super>space"]}}}' \
    '{"version": 1, "settings": {"search": {"currency": true}}}' \
    '{"version": 1, "settings": {"emoji": {"skin-tone": "purple"}}}' \
    '{"version": 1, "settings": {"emoji": {"emoji-picker": ["<Super>period"]}}}' \
    '{"version": 1, "settings": {"quit": {"frozen-dialog": "on"}}}' \
    '{"version": 1, "settings": {"quit": {"force-quit": ["<Super>q"]}}}' \
    '{"version": 1, "settings": {"agent": {"pill": false}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.shell.extensions.amethystora-ui.agent": {"pill": ["<Super>a"]}}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.shell.extensions.amethystora-ui.quit": {"frozen-dialog": ["<Super>a"]}}}}' \
    '{"version": 1, "settings": {"bezel": {"round-corners": "on"}}}' \
    '{"version": 1, "settings": {"bezel": {"radius": 20}}}' \
    '{"version": 1, "settings": {"shelf": {"shelf": "dock"}}}' \
    '{"version": 1, "settings": {"shelf": {"items": []}}}' \
    '{"version": 1, "settings": {"desks": {"desks": "on"}}}' \
    '{"version": 1, "settings": {"desks": {"list": [{"serial": "ABC"}]}}}' \
    '{"version": 1, "settings": {"sound": {"theme-name": "__custom"}}}' \
    '{"version": 1, "settings": {"sound": {"event-sounds": 0}}}' \
    '{"version": 1, "settings": {"transitions": {"style": "wave"}}}' \
    '{"version": 1, "settings": {"hints": {"enabled": "on"}}}' \
    '{"version": 1, "settings": {"appgrid": {"folders": []}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.shell.extensions.amethystora-ui.search": {"answers": ["<Super>a"]}}}}' \
    '{"version": 1, "settings": {"pill": {"digest-times": []}}}' \
    '{"version": 1, "settings": {"pill": {"digest-times": ["25:00"]}}}' \
    '{"version": 1, "settings": {"pill": {"digest-times": ["12:00", "12:00"]}}}' \
    '{"version": 1, "settings": {"pill": {"digest-times": ["01:00", "02:00", "03:00", "04:00", "05:00", "06:00", "07:00"]}}}' \
    '{"version": 1, "settings": {"pill": {"show-banners": false}}}' \
    '{"version": 1, "settings": {"quick": {"no-such-setting": []}}}' \
    '{"version": 1, "settings": {"quick": {"tile-order": ["Wifi"]}}}' \
    '{"version": 1, "settings": {"input": {"sources": [["xkb", "us"]]}}}' \
    '{"version": 1, "settings": {"input": {"xkb-options": ["caps escape"]}}}' \
    '{"version": 1, "settings": {"input": {"xkb-options": ["caps:escape", "caps:escape"]}}}' \
    '{"version": 1, "settings": {"input": {"per-window": "on"}}}' \
    '{"version": 1, "settings": {"keyboard": {"delay": 50}}}' \
    '{"version": 1, "settings": {"keyboard": {"repeat-interval": 30}}}' \
    '{"version": 1, "settings": {"paste": {"font-name": "Sans 30"}}}' \
    '{"version": 1, "settings": {"keys": {"super-alone": "dock"}}}' \
    '{"version": 1, "settings": {"keys": {"plain-paste": ["<Super>v"]}}}' \
    '{"version": 1, "settings": {"typing": {"accents": true}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.shell.extensions.amethystora-ui.keys": {"super-alone": ["<Super>a"]}}}}' \
    '{"version": 1, "settings": {"quick": {"tile-hidden": ["../dnd"]}}}' \
    '{"version": 1, "settings": {"quick": {"tile-hidden": ["dnd", "dnd"]}}}' \
    '{"version": 1, "settings": {"quick": {"tile-order": "wifi"}}}' \
    '{"version": 1, "settings": {"bars": {"every-screen": "on"}}}' \
    '{"version": 1, "settings": {"bars": {"no-such-setting": true}}}' \
    '{"version": 1, "settings": {"today": {"weather": true}}}' \
    '{"version": 1, "settings": {"today": {"tasks": 1}}}' \
    '{"version": 1, "settings": {"breaks": {"selected-breaks": ["sleep"]}}}' \
    '{"version": 1, "settings": {"breaks": {"selected-breaks": ["eyesight", "eyesight"]}}}' \
    '{"version": 1, "settings": {"breaks": {"selected-breaks": "eyesight"}}}' \
    '{"version": 1, "settings": {"breaks": {"eyesight": {"interval-seconds": 60}}}}' \
    '{"version": 1, "settings": {"look": {"accent": "teal"}}}' \
    '{"version": 1, "settings": {"look": {"accent": "\"; rm -rf ~"}}}' \
    '{"version": 1, "settings": {"look": {"colours": "loud"}}}' \
    '{"version": 1, "settings": {"look": {"daylight": true}}}' \
    '{"version": 1, "settings": {"look": {"accent-per-workspace": "on"}}}' \
    '{"version": 1, "settings": {"look": {"workspace-accents": []}}}' \
    '{"version": 1, "settings": {"look": {"workspace-accents": ["sky", "coral", "jade", "amber", "orchid", "../x"]}}}' \
    '{"version": 1, "settings": {"look": {"no-such-setting": "on"}}}' \
    '{"version": 1, "settings": {"focus": {"profile": "work"}}}' \
    '{"version": 1, "settings": {"focus": {"ends": 0}}}' \
    '{"version": 1, "settings": {"focus": {"work-set": "Writing"}}}' \
    '{"version": 1, "settings": {"focus": {"games-power": "turbo"}}}' \
    '{"version": 1, "settings": {"focus": {"sleep-accent": "teal"}}}' \
    '{"version": 1, "settings": {"focus": {"work-apps": ["rm -rf ~"]}}}' \
    '{"version": 1, "settings": {"focus": {"work-apps": ["a.desktop", "a.desktop"]}}}' \
    '{"version": 1, "settings": {"focus": {"trigger-games": "on"}}}' \
    '{"version": 1, "settings": {"access": {"filter-on": false}}}' \
    '{"version": 1, "settings": {"access": {"ruler-shown": true}}}' \
    '{"version": 1, "settings": {"access": {"read-aloud-key": ["<Super>s"]}}}' \
    '{"version": 1, "settings": {"access": {"filter": "red"}}}' \
    '{"version": 1, "settings": {"access": {"filter-strength": 10}}}' \
    '{"version": 1, "settings": {"access": {"filter-strength": 50.5}}}' \
    '{"version": 1, "settings": {"access": {"read-aloud-speed": "3"}}}' \
    '{"version": 1, "settings": {"touch": {"touch-mode": "tablet"}}}' \
    '{"version": 1, "settings": {"touch": {"pen-ring-key": ["<Super>p"]}}}' \
    '{"version": 1, "settings": {"touch": {"pen-ring-tools": []}}}' \
    '{"version": 1, "settings": {"touch": {"pen-ring-tools": ["area", "area"]}}}' \
    '{"version": 1, "settings": {"touch": {"pen-ring-tools": ["rm -rf ~"]}}}' \
    '{"version": 1, "settings": {"present": {"scribble": ["<Super>d"]}}}' \
    '{"version": 1, "settings": {"present": {"pointer-mark": "laser"}}}' \
    '{"version": 1, "settings": {"present": {"enabled": "on"}}}' \
    '{"version": 1, "settings": {"screenshot": {"tool-settings": {}}}}' \
    '{"version": 1, "settings": {"screenshot": {"capture-pin": 1}}}' \
    '{"version": 1, "settings": {"deskwidgets": {"widgets": ["clock"]}}}' \
    '{"version": 1, "settings": {"privacy": {"shield": "loud"}}}' \
    '{"version": 1, "settings": {"privacy": {"share-guard-apps": ["rm -rf ~"]}}}' \
    '{"version": 1, "settings": {"privacy": {"walk-away": "on"}}}' \
    '{"version": 1, "settings": {"privacy": {"network-apps": "on"}}}' \
    '{"version": 1, "settings": {"bars": {"layout": {"left": ["clock"], "hidden": ["system"]}}}}' \
    '{"version": 1, "settings": {"bars": {"layout": {"left": ["clock"], "right": ["clock"]}}}}' \
    '{"version": 1, "settings": {"bars": {"layout": {"top": ["clock"]}}}}' \
    '{"version": 1, "settings": {"bars": {"layout": {"left": ["rm -rf ~"]}}}}' \
    '{"version": 1, "settings": {"bars": {"size": "huge"}}}' \
    '{"version": 1, "settings": {"pill": {"second-zone": "../../etc/passwd"}}}' \
    '{"version": 1, "settings": {"motion": {"speed": "warp"}}}' \
    '{"version": 1, "settings": {"interface": {"text-scaling-factor": 3}}}' \
    '{"version": 1, "settings": {"interface": {"font-name": "Comic Sans 11"}}}' \
    '{"version": 1, "settings": {"calendar": {"show-weekdate": "yes"}}}'; do
    printf '%s\n' "${bad}" >"${T}/bad.json"
    "${SETUP}" diff "${T}/bad.json" >/dev/null 2>&1 && { echo "accepted: ${bad}"; false; }
    "${SETUP}" apply --yes "${T}/bad.json" >/dev/null 2>&1 && { echo "applied: ${bad}"; false; }
done
[[ ! -s "${LOG}" ]]
# ...and a file with the windows' new settings, their keybindings and GNOME's window settings is one it reads
jq '.settings.windows += {"see-through-apps": {"kitty.desktop": 92}, "frame": true, "drop-down": false}
    | .settings.wm = {"focus-mode": "sloppy", "mouse-button-modifier": "<Alt>", "action-middle-click-titlebar": "lower"}
    | .settings.mutter = {"center-new-windows": false}
    | .settings.keybindings["org.gnome.shell.extensions.amethystora-ui.windows"] = {"reopen-key": ["<Super><Alt>t"]}' \
    "${FILE}" >"${T}/windows.json"
"${SETUP}" diff --json "${T}/windows.json" >/dev/null

# The look: what is in force is saved (the account's files, as the theme reads them, and the theme module's keys), and
# apply sets it through ame's own commands, the workspaces' accents (one past the sixth too) through the key
LOOK_DIR="${XDG_CONFIG_HOME}/amethystora"
mkdir -p "${LOOK_DIR}"
echo jade >"${LOOK_DIR}/accent"
echo off >"${LOOK_DIR}/daylight"
echo soft >"${LOOK_DIR}/colours"
[[ "$("${SETUP}" save - | jq -c .settings.look)" == '{"accent":"jade","daylight":"off","colours":"soft"}' ]]
"${SETUP}" save - | jq '.settings.look["workspace-accents"] = ["theme", "moss", "sky", "jade", "amber", "orchid", "coral"]' \
    >"${T}/look.json"
rm -f "${LOOK_DIR}/accent" "${LOOK_DIR}/daylight" "${LOOK_DIR}/colours"
"${SETUP}" apply --yes "${T}/look.json" >/dev/null
grep -qx "ame desktop accent jade" "${LOG}"
grep -qx "ame desktop daylight off" "${LOG}"
grep -qx "ame desktop colours soft" "${LOG}"
grep -qxF "gsettings set org.gnome.shell.extensions.amethystora-ui.theme workspace-accents ['theme', 'moss', 'sky', 'jade', 'amber', 'orchid', 'coral']" "${LOG}"
grep -qx "amethystora-theme accents" "${LOG}"
: >"${LOG}"
# ...and nothing is saved for a look that is the image's own
[[ "$("${SETUP}" save - | jq -c '.settings.look // {}')" == '{}' ]]
# The drawn wallpaper's pattern is saved, never its seed, and applied through its own command
printf 'dots\n42\n' >"${LOOK_DIR}/pattern"
[[ "$("${SETUP}" save - | jq -c .settings.look)" == '{"pattern":"dots"}' ]]
"${SETUP}" save - >"${T}/pattern.json"
rm -f "${LOOK_DIR}/pattern"
"${SETUP}" apply --yes "${T}/pattern.json" >/dev/null
grep -qx "ame desktop pattern dots" "${LOG}"
: >"${LOG}"

# bake: the same file as an image's defaults, into a stand-in for the image made of its own schemas, extensions'
# schemas and dconf files, with the real tools (no stand-ins on the PATH). What it writes is read back as GNOME
# reads it: the system's schemas, the tiling's own compiled copy, which GNOME Shell reads first, and dconf's database.
ROOT="${T}/root"
UI=usr/share/gnome-shell/extensions/amethystora-ui@iarsslen.github.io
bake_root() {
    rm -rf "${ROOT}"
    mkdir -p "${ROOT}/usr/share/glib-2.0" "${ROOT}/${UI}" "${ROOT}/etc/dconf/db" "${ROOT}/usr/share/amethystora/themes/nord" \
        "${ROOT}/usr/share/gnome-shell/extensions/amethystora-clipboard@iarsslen.github.io"
    cp -r /usr/share/glib-2.0/schemas "${ROOT}/usr/share/glib-2.0/"
    cp -r "/${UI}/schemas" "${ROOT}/${UI}/"
    cp -r /etc/dconf/db/distro.d "${ROOT}/etc/dconf/db/"
}
# ...and without HOME, which a build need not set
bake() { env -u HOME PATH=/usr/bin:/usr/sbin "${SETUP}" bake "$@" --root "${ROOT}"; }
image() { GSETTINGS_SCHEMA_DIR="${ROOT}/usr/share/glib-2.0/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "$@"; }
bake_root
jq '.image = {"name": "amethystora", "stream": "stable"}
    | .flatpaks.system += [{"app": "org.gnome.Maps", "origin": "flathub"}] | .extensions = ["amethystora-clipboard@iarsslen.github.io"]
    | .settings.layout = "classic" | .brew.taps = ["someone/tools"] | .brew.formulae = ["fd", "someone/tools/lint"]
    | .settings.motion = {"minimize": "shrink"} | .settings.workspaces = {"auto-names": false, "sets": false}
    | .settings.look = {"accent": "jade", "colours": "soft", "accent-per-workspace": true,
        "workspace-accents": ["theme", "moss", "sky", "jade", "amber", "orchid"]}' \
    "${FILE}" >"${T}/image.json"
out="$(bake "${T}/image.json")"
grep -qF "Left out: the containers" <<<"${out}"
grep -qF "Left out: image" <<<"${out}"
PRE="${ROOT}/usr/share/flatpak/preinstall.d/image-setup.preinstall"
[[ "$(grep -c '^\[Flatpak Preinstall ' "${PRE}")" == 3 && "$(grep -c '^Branch=stable$' "${PRE}")" == 3 ]]
grep -qxF "[Flatpak Preinstall org.gimp.GIMP]" "${PRE}"
[[ "$(image org.gnome.shell favorite-apps)" == "['org.gnome.Nautilus.desktop', 'firefox.desktop']" ]]
# ...the image's extensions kept, the file's added
extensions="$(image org.gnome.shell enabled-extensions)"
grep -qF "'amethystora-ui@iarsslen.github.io'" <<<"${extensions}"
grep -qF "'amethystora-clipboard@iarsslen.github.io'" <<<"${extensions}"
[[ "$(image org.gnome.desktop.wm.keybindings close)" == "['<Super>q']" ]]
[[ "$(image "${TILING}.keybindings" cycle-width)" == "['<Super>e']" ]]
[[ "$(image "${TILING}" enabled)" == false && "$(image "${TILING}" window-gap)" == 12 ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "${TILING}" window-gap)" == 12 ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "${TILING}" enabled)" == false ]]
[[ "$(image "${POINTER}" shake)" == "'beacon'" && "$(image "${POINTER}" dense)" == true ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "${POINTER}" shake)" == "'beacon'" ]]
[[ "$(image "${GLASS}" material)" == "'clear'" && "$(image "${GLASS}" solid)" == false ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "${GLASS}" material)" == "'clear'" ]]
[[ "$(image "${MOTION}" minimize)" == "'shrink'" && "$(image "${MOTION}" workspace-switch)" == "'depth'" ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "${WORKSPACES}" auto-names)" == false ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "${WORKSPACES}" sets)" == false ]]
[[ "$(image "${WINDOWS}" dim)" == true && "$(image "${WINDOWS}" snap)" == true ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "${WINDOWS}" dim)" == true ]]
printf 'file-db:%s\n' "${ROOT}/etc/dconf/db/distro" >"${T}/dconf-profile"
[[ "$(DCONF_PROFILE="${T}/dconf-profile" /usr/bin/dconf read /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom20/binding)" == "'<Super>t'" ]]
# ...the image's command for it left as it was
[[ -n "$(DCONF_PROFILE="${T}/dconf-profile" /usr/bin/dconf read /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom20/command)" ]]
[[ "$(cat "${ROOT}/usr/share/amethystora/image-setup/theme")" == nord ]]
# ...the look: the accent and colours where the theme reads them after the account's, the theme module's keys in zz2
[[ "$(cat "${ROOT}/usr/share/amethystora/image-setup/accent")" == jade ]]
[[ "$(cat "${ROOT}/usr/share/amethystora/image-setup/colours")" == soft ]]
[[ "$(image org.gnome.shell.extensions.amethystora-ui.theme accent-per-workspace)" == true ]]
[[ "$(GSETTINGS_SCHEMA_DIR="${ROOT}/${UI}/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get \
    org.gnome.shell.extensions.amethystora-ui.theme workspace-accents)" == "['theme', 'moss', 'sky', 'jade', 'amber', 'orchid']" ]]
grep -qF "Left out: the look's daylight" <<<"$(jq '.settings.look.daylight = "off"' "${T}/image.json" >"${T}/image-daylight.json" &&
    bake_root && bake "${T}/image-daylight.json")"
bake_root
bake "${T}/image.json" >/dev/null
diff <(printf '%s\n' 'tap "someone/tools"' 'brew "fd"' 'brew "someone/tools/lint"' 'cask "font-iosevka"') \
    "${ROOT}/usr/share/amethystora/image-setup/Brewfile"
cmp -s "${T}/image.json" "${ROOT}/usr/share/amethystora/image-setup/setup.json"
# An app from a remote the image declares (amethystora-image-policy --only flatpak-remotes, before bake) is baked in
bake_root
mkdir -p "${ROOT}/etc/flatpak/remotes.d"
printf '[Flatpak Repo]\nUrl=https://flatpak.acme.example/repo/\n' >"${ROOT}/etc/flatpak/remotes.d/acme.flatpakrepo"
echo '{"version": 1, "flatpaks": {"system": [{"app": "com.acme.Tool", "origin": "acme"}]}}' >"${T}/acme.json"
bake "${T}/acme.json" >/dev/null
grep -qxF "[Flatpak Preinstall com.acme.Tool]" "${ROOT}/usr/share/flatpak/preinstall.d/image-setup.preinstall"
bake_root

# The theme hook falls back on the baked theme, and the Brewfile hook installs once per version of the file
grep -qF /usr/share/amethystora/image-setup/theme /usr/share/amethystora/user-setup.hooks.d/11-theme.sh
grep -qF /usr/share/amethystora/image-setup/Brewfile /usr/share/amethystora/user-setup.hooks.d/18-image-brewfile.sh

# ...and refuses, writing nothing, what the image cannot have: an app from another remote, an extension or a theme
# it does not ship, a custom keybinding or a keybinding it does not have, PaperWM's, and all that the format refuses
for bad in '{"version": 1, "flatpaks": {"user": [{"app": "org.gimp.GIMP", "origin": "gnome-nightly"}]}}' \
    '{"version": 1, "extensions": ["someone@example.com"]}' \
    '{"version": 1, "theme": "no-such-theme"}' \
    '{"version": 1, "settings": {"keybindings": {"custom": {"custom99": ["<Super>t"]}}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.desktop.wm.keybindings": {"no-such-key": ["<Super>t"]}}}}' \
    '{"version": 1, "settings": {"keybindings": {"org.gnome.shell.extensions.paperwm.keybindings": {"switch-left": ["<Super>h"]}}}}' \
    '{"version": 1, "settings": {"tiling": {"winprops": ["{\"wm_class\": \"x\"}"]}}}' \
    '{"version": 1, "brew": {"taps": ["https://example.com/tap.git"]}}'; do
    bake_root
    printf '%s\n' "${bad}" >"${T}/bad.json"
    bake "${T}/bad.json" >/dev/null 2>&1 && { echo "baked: ${bad}"; false; }
    [[ ! -e "${ROOT}/usr/share/amethystora/image-setup" && ! -e "${ROOT}/usr/share/glib-2.0/schemas/zz2-image-setup.gschema.override" ]]
done
echo "ame setup: ok"
