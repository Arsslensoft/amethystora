#!/usr/bin/env bash
#
# The account's own kitty options that `ame desktop` commands write (`ame desktop command-done`), which the image's
# /etc/xdg/kitty/kitty.conf includes last: made once, a comment only, so that kitty never warns that the file it
# includes is missing. An account that has one already keeps it as it is.

source /usr/lib/amethystora/setup-services/libsetup.sh

version-script kitty-options user 2 || exit 0

FILE="${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/kitty.conf"
COMMENT='# Written by `ame desktop command-done`. Your own settings go in ~/.config/kitty/kitty.conf.'
# Version 1 wrote the comment in double quotes, so bash ran the command it names and wrote "# Written by on.": only
# that line is put right, in a plain file
if [[ -f "${FILE}" && ! -L "${FILE}" ]]; then
    sed -i -E "s@^# Written by (on|off)\. Your own settings go in ~/\.config/kitty/kitty\.conf\.\$@${COMMENT}@" "${FILE}"
fi
[[ -e "${FILE}" || -L "${FILE}" ]] && exit 0
mkdir -p "$(dirname "${FILE}")"
printf '%s\n' "${COMMENT}" >"${FILE}"

exit 0
