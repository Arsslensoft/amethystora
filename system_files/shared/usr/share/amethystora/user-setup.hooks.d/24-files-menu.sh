#!/usr/bin/bash
#
# Files' scripts menu: Folder colour (the six colours and No colour), Back in time, Earlier versions, Copy without hidden
# data, What is using this? and Send to nearby, in the account's language, as tiny scripts in ~/.local/share/nautilus/scripts that hand Files' arguments to
# /usr/libexec/amethystora-folder-colour, /usr/libexec/amethystora-back-in-time, /usr/libexec/amethystora-clean-copy and
# /usr/libexec/amethystora-what-uses show. Files shows them under Scripts in the menu of a selection. Convert, which is
# offered and off, is written by /usr/libexec/amethystora-convert menu instead (ame desktop files-convert).
#
# Each file carries a marker in its second line, and only such a file is ever written over: a script of the person's
# own under the same name stays, and that entry is not written. What was written is listed in
# ~/.local/state/amethystora/files-menu.json, so that a later version can replace or take away exactly those, whatever
# the language since. Once: an entry the person deletes does not come back until the version is raised.

source /usr/lib/amethystora/setup-services/libsetup.sh

command -v jq >/dev/null || exit 0

version-script files-menu user 5 || exit 0

set -uo pipefail

# The account's language: the session's, or, where this runs without one, the one the account chose in Settings
if [[ -z "${LANG:-}" || "${LANG}" == C* || "${LANG}" == POSIX ]]; then
    chosen="$(gdbus call --system --dest org.freedesktop.Accounts --object-path "/org/freedesktop/Accounts/User$(id -u)" \
        --method org.freedesktop.DBus.Properties.Get org.freedesktop.Accounts.User Language 2>/dev/null |
        sed -n "s/^(<'\([a-zA-Z_]*\)\(\.[A-Za-z0-9-]*\)\{0,1\}'>,)$/\1/p")"
    [[ -n "${chosen}" ]] && export LANG="${chosen}.UTF-8" LANGUAGE="${chosen}:${chosen%%_*}"
fi
export TEXTDOMAIN=amethystora
# shellcheck source=/dev/null
source gettext.sh

SCRIPTS="${XDG_DATA_HOME:-${HOME}/.local/share}/nautilus/scripts"
STATE="${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/files-menu.json"
MARK='# amethystora-files-menu 1. Written by the image (user-setup.hooks.d/24-files-menu.sh); delete it to remove the entry.'

# A name as the menu shows it, translated; a catalog can never make it a path, a hidden file or nothing
entry_name() { # TRANSLATED ENGLISH
    local name
    name="$(printf '%s' "$1" | tr -d '/\n')"
    [[ -n "${name}" && "${name}" != .* ]] || name="$2"
    printf '%s' "${name}"
}

written=()

# What an earlier run wrote: an entry the person has deleted since stays gone when a later version runs this again (and
# stays in the list, so that it stays known)
mapfile -t before < <(jq -r '.paths[]? // empty' "${STATE}" 2>/dev/null)
gone() { # PATH
    local known
    [[ -e "$1" || -L "$1" ]] && return 1
    for known in "${before[@]}"; do
        [[ "${known}" == "$1" ]] && return 0
    done
    return 1
}

# A file this hook wrote: a plain file whose second line is the marker
marked() { # PATH
    [[ -f "$1" && ! -L "$1" && "$(sed -n 2p "$1")" == "# amethystora-files-menu "* ]]
}

# Writes one entry, unless something of the person's own is already there under that name, or this run wrote that
# path already (two words a catalog translated alike)
entry() { # PATH COMMAND...
    local path="$1" temporary done
    shift
    for done in "${written[@]}"; do
        [[ "${done}" == "${path}" ]] && return 0
    done
    if gone "${path}"; then
        written+=("${path}")
        return 0
    fi
    if [[ -e "${path}" || -L "${path}" ]]; then
        marked "${path}" || return 0
    fi
    temporary="$(mktemp "${path%/*}/.files-menu.XXXXXX")" || return 0
    printf '#!/bin/sh\n%s\nexec %s -- "$@"\n' "${MARK}" "$*" >"${temporary}"
    if ! chmod 0755 "${temporary}" || ! mv -f -- "${temporary}" "${path}"; then
        rm -f -- "${temporary}"
        return 0
    fi
    written+=("${path}")
}

# Files' folder and its scripts folder 0700 when made here, as Files makes them; the data folder above as it comes
mkdir -p -- "${SCRIPTS%/*/*}" || exit 0
for folder in "${SCRIPTS%/*}" "${SCRIPTS}"; do
    [[ -d "${folder}" ]] || mkdir -m 0700 -- "${folder}" || exit 0
done

# Translators: a submenu of Files' Scripts menu, holding the six colours a folder can be given and No colour
FOLDER="${SCRIPTS}/$(entry_name "$(gettext "Folder colour")" "Folder colour")"
ours=false
if gone "${FOLDER}"; then
    written+=("${FOLDER}")
elif [[ ! -e "${FOLDER}" && ! -L "${FOLDER}" ]]; then
    mkdir -m 0700 -- "${FOLDER}" && written+=("${FOLDER}") && ours=true
elif [[ -d "${FOLDER}" && ! -L "${FOLDER}" ]]; then
    # A folder of that name is this hook's only when it holds an entry the hook wrote; one the person made is left
    for file in "${FOLDER}"/*; do
        marked "${file}" && ours=true && written+=("${FOLDER}") && break
    done
fi
if ${ours}; then
    # Translators: the colours of folders in Files' Scripts menu, which sorts them by name
    entry "${FOLDER}/$(entry_name "$(gettext "Amethyst")" Amethyst)" /usr/libexec/amethystora-folder-colour amethyst
    entry "${FOLDER}/$(entry_name "$(gettext "Sky")" Sky)" /usr/libexec/amethystora-folder-colour sky
    entry "${FOLDER}/$(entry_name "$(gettext "Jade")" Jade)" /usr/libexec/amethystora-folder-colour jade
    entry "${FOLDER}/$(entry_name "$(gettext "Amber")" Amber)" /usr/libexec/amethystora-folder-colour amber
    entry "${FOLDER}/$(entry_name "$(gettext "Coral")" Coral)" /usr/libexec/amethystora-folder-colour coral
    entry "${FOLDER}/$(entry_name "$(gettext "Slate")" Slate)" /usr/libexec/amethystora-folder-colour slate
    entry "${FOLDER}/$(entry_name "$(gettext "No colour")" "No colour")" /usr/libexec/amethystora-folder-colour none
fi
# Translators: an entry of Files' Scripts menu, which opens Backups on the folder picked as it was at other times
entry "${SCRIPTS}/$(entry_name "$(gettext "Back in time")" "Back in time")" /usr/libexec/amethystora-back-in-time
# Translators: an entry of Files' Scripts menu, which opens Backups on the file picked: each version kept of it, to look at
# or keep beside the file. Unless it was turned off (ame desktop earlier-versions off), or that command wrote it already, in the
# language of its day: not a second one under another name
versions_there=false
for file in "${SCRIPTS}"/*; do
    marked "${file}" && [[ "$(sed -n 3p "${file}")" == 'exec /usr/libexec/amethystora-back-in-time --versions -- '* &&
        "$(sed -n 2p "${file}")" == *"(amethystora-back-in-time menu)"* ]] && versions_there=true
done
if ! ${versions_there} && [[ "$(gsettings get org.gnome.shell.extensions.amethystora-ui.tools earlier-versions 2>/dev/null)" != false ]]; then
    entry "${SCRIPTS}/$(entry_name "$(gettext "Earlier versions")" "Earlier versions")" /usr/libexec/amethystora-back-in-time --versions
fi
# Translators: an entry of Files' Scripts menu, which makes a copy of each file picked without its location, camera,
# author and edit history
entry "${SCRIPTS}/$(entry_name "$(gettext "Copy without hidden data")" "Copy without hidden data")" /usr/libexec/amethystora-clean-copy
# Translators: an entry of Files' Scripts menu, which lists the programs of the person's own that hold the file, folder
# or drive picked (one that will not eject), each with Quit
entry "${SCRIPTS}/$(entry_name "$(gettext "What is using this?")" "What is using this?")" /usr/libexec/amethystora-what-uses show
# Translators: an entry of Files' Scripts menu, which opens the phone link's send window on the files picked, with the paired phones
# and computers that can take them shown as round avatars
entry "${SCRIPTS}/$(entry_name "$(gettext "Send to nearby")" "Send to nearby")" amethystora-phone send

# The list, written whole beside a copy, so that a file cut short never loses it
mkdir -p -- "${STATE%/*}"
list="$(jq -n '{version: 1, paths: $ARGS.positional}' --args "${written[@]}")" || exit 0
for target in "${STATE}" "${STATE}.bak"; do
    printf '%s\n' "${list}" >"${target}.new" && mv -f -- "${target}.new" "${target}"
done
exit 0
