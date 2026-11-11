#!/usr/bin/bash
# amethystora-control-list joins an entry's fields with the record separator and a choice's with the unit
# separator, so a field holding either would shift the fields after it: a title could make the next field the
# choice's `terminal` and its words the command the window runs. Both functions take the two out of every field.
# Runs the real choice() and item() with the list's own jq program, and needs no more than bash 5.3 and jq.
set -euo pipefail

LIST="${1:-/usr/libexec/amethystora-control-list}"
RS=$'\x1e'
US=$'\x1f'

# The functions and ITEM's jq program, cut out of the script by their own lines
{
    echo 'CHOICES=(); ITEMS=(); COMMAND=(); TERMINAL=true; BUTTON=""; EXTRA=""; REASON=""; DETAIL=""; GROUP=""; USAGE=""'
    sed -n '/^choice() {/,/^}/p;/^item() {/,/^}/p' "${LIST}"
    sed -n "/^ITEM='/,/fromjson);'\$/p" "${LIST}"
    echo 'items_json() { jq -nc "${ITEM}"'"'"' $ARGS.positional[] | item'"'"' --args "${ITEMS[@]}"; }'
} >"${TMPDIR:-/tmp}/control-list-functions.$$"
trap 'rm -f "${TMPDIR:-/tmp}/control-list-functions.$$"' EXIT
# shellcheck disable=SC1090
source "${TMPDIR:-/tmp}/control-list-functions.$$"

# A title and a choice's words with both separators in them, which would make "true" the terminal and the rest a command
choice "v${US}al" "Evil${US}title" "text${RS}" "${US}true${US}ame" desktop sneaky
EXTRA='{"a":1}'
COMMAND=(ame "desktop${RS}x" "y${US}z")
item page "id${RS}2" kind "Title${RS}${US}with" "text" state manual

json="$(items_json)"
[[ "$(jq -r '.title' <<<"${json}")" == "Titlewith" ]]
[[ "$(jq -r '.id' <<<"${json}")" == "id2" ]]
[[ "$(jq -c '.command' <<<"${json}")" == '["ame","desktopx","yz"]' ]]
[[ "$(jq -c '.choices | length' <<<"${json}")" == 1 ]]
# The choice kept its five fields: the separators went, so its value, title, text, terminal and command are its own
[[ "$(jq -c '.choices[0] | [.value, .title, .terminal, .command]' <<<"${json}")" == '["val","Eviltitle",false,["desktop","sneaky"]]' ]]
[[ "$(jq -r '.a' <<<"${json}")" == 1 ]]
echo "control list separators: ok"
