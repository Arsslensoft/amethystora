#!/usr/bin/bash
#
# Files' New Document menu, empty on a fresh account: four templates in the account's templates folder, in its language,
# once. An empty text, a Markdown note and two scripts with their first line written. They are the account's to change,
# add to or delete, and are written only into a templates folder with no file in it yet: one the person already filled
# is theirs, and is left alone. A script template is never made executable: a new script from it is a text file until
# the person makes it one.

source /usr/lib/amethystora/setup-services/libsetup.sh

set -uo pipefail

# Where Files looks for templates is xdg-user-dirs' TEMPLATES, which it names in the account's language at its first login
# (user-dirs.dirs). Until it has, this waits for a later login rather than make a folder of another name
[[ -f "${XDG_CONFIG_HOME:-${HOME}/.config}/user-dirs.dirs" ]] || exit 0

version-script templates user 1 || exit 0

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

# TEMPLATES is the home folder itself when the account has turned templates off, and then nothing is written
TEMPLATES="$(xdg-user-dir TEMPLATES 2>/dev/null)"
[[ -n "${TEMPLATES}" && "${TEMPLATES}" == /* ]] || exit 0
[[ "$(realpath -m -- "${TEMPLATES}")" != "$(realpath -m -- "${HOME}")" ]] || exit 0
[[ ! -L "${TEMPLATES}" ]] || exit 0
mkdir -p -- "${TEMPLATES}" || exit 0
# Only into a folder with no file of the person's own in it
[[ -z "$(find "${TEMPLATES}" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" ]] || exit 0

# A file's name as the menu shows it, translated; a catalog can never make it a path, a hidden file or nothing
template_name() { # TRANSLATED ENGLISH
    local name
    name="$(printf '%s' "$1" | tr -d '/\n')"
    [[ -n "${name}" && "${name}" != .* ]] || name="$2"
    printf '%s' "${name}"
}

# One template: written new, never over a file, 0644
template() { # NAME CONTENT
    local path="${TEMPLATES}/$1"
    [[ -e "${path}" || -L "${path}" ]] && return 0
    (umask 022 && set -o noclobber && printf '%s' "$2" >"${path}") 2>/dev/null || return 0
    chmod 0644 -- "${path}" 2>/dev/null || true
}

# Translators: a file name in Files' New Document menu (keep the .txt)
template "$(template_name "$(gettext "Empty text.txt")" "Empty text.txt")" ""
# Translators: a file name in Files' New Document menu (keep the .md)
template "$(template_name "$(gettext "Markdown note.md")" "Markdown note.md")" $'# \n\n'
# Translators: a file name in Files' New Document menu (keep the .sh)
template "$(template_name "$(gettext "Shell script.sh")" "Shell script.sh")" $'#!/usr/bin/env bash\nset -euo pipefail\n\n'
# Translators: a file name in Files' New Document menu (keep the .py)
template "$(template_name "$(gettext "Python script.py")" "Python script.py")" $'#!/usr/bin/env python3\n\n'

exit 0
