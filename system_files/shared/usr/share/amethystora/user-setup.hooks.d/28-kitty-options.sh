#!/usr/bin/env bash
#
# The account's own kitty options that `ame desktop` commands write (`ame desktop command-done`), which the image's
# /etc/xdg/kitty/kitty.conf includes last: made once, a comment only, so that kitty never warns that the file it
# includes is missing. An account that has one already keeps it as it is.

source /usr/lib/amethystora/setup-services/libsetup.sh

version-script kitty-options user 1 || exit 0

FILE="${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/kitty.conf"
[[ -e "${FILE}" || -L "${FILE}" ]] && exit 0
mkdir -p "$(dirname "${FILE}")"
printf '%s\n' "# Written by `ame desktop command-done`. Your own settings go in ~/.config/kitty/kitty.conf." >"${FILE}"

exit 0
