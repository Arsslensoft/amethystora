#!/usr/bin/env bash
#
# Widgets are plugins now: the extension that ran the account's own widgets (programs with everything the account has) is gone, and
# nothing is read from ~/.config/amethystora/widgets any more. An account that had widgets there would find them gone without a word,
# so it is told, once, a minute into the first login after. Its folders are left alone; `ame plugin new` starts a plugin from the
# same files, and the image's own widgets are plugins to add (ame plugin shipped).

STATE="${XDG_STATE_HOME:-${HOME}/.local/state}/amethystora/widgets-moved-notice"
[[ -e "${STATE}" ]] && exit 0

FOLDER="${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/widgets"
[[ -d "${FOLDER}" && ! -L "${FOLDER}" ]] || exit 0
compgen -G "${FOLDER}/*/widget.json" >/dev/null || exit 0

mkdir -p "${STATE%/*}"
: >"${STATE}"
systemd-run --user --collect --quiet --on-active=60 -- /usr/libexec/amethystora-widgets-moved-notice
exit 0
