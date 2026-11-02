#!/usr/bin/bash
# If it's lost (R7-71): what amethystora-if-found writes, against a stand-in root: one line in each dconf database, however
# it is quoted, nothing else, and off leaving nothing. Run by 20-tests.sh; HELPER names another copy.
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-if-found}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
export AMETHYSTORA_IF_FOUND_ROOT="${T}"
ACCOUNTS="${T}/etc/dconf/db/local.d/70-amethystora-if-found"
LOGIN="${T}/etc/dconf/db/gdm.d/70-amethystora-if-found"

fail() {
    echo "test-if-found: $*"
    exit 1
}

[[ -z "$("${HELPER}" status)" ]] || fail "a line with nothing written"

line="If found, please call Sam's mum: +00 000 000 000 \\ thanks"
printf '%s\n' "${line}" | "${HELPER}" set
[[ "$("${HELPER}" status)" == "${line}" ]] || fail "the line did not come back: $("${HELPER}" status)"
[[ "$(wc -l <"${ACCOUNTS}")" == 2 && "$(wc -l <"${LOGIN}")" == 3 ]] || fail "more than the keys were written"
grep -qx '\[org/gnome/shell/extensions/amethystora-ui/found\]' "${ACCOUNTS}" || fail "the accounts' file has the wrong section"
grep -qx '\[org/gnome/login-screen\]' "${LOGIN}" || fail "GDM's file has the wrong section"
grep -qx 'banner-message-enable=true' "${LOGIN}" || fail "GDM's banner is not on"
grep -qxF "banner-message-text='If found, please call Sam\\'s mum: +00 000 000 000 \\\\ thanks'" "${LOGIN}" ||
    fail "GDM's line is not quoted: $(cat "${LOGIN}")"
# Each value parses as one GVariant string
python3 - "${ACCOUNTS}" "${LOGIN}" <<'EOF' || fail "a value does not parse"
import sys
import gi
from gi.repository import GLib
for path in sys.argv[1:]:
    for row in open(path, encoding='utf-8'):
        if row.startswith(('line=', 'banner-message-text=')):
            assert GLib.Variant.parse(None, row.split('=', 1)[1].strip()).get_type_string() == 's'
EOF

# What would break out, or that is no text, is refused and changes nothing
before="$(cat "${ACCOUNTS}")"
for bad in "$(printf 'one\ttwo')" "$(printf 'esc\033[2J')" "$(printf 'a\xe2\x80\xaeb')" "   " "$(printf '%0121d' 0)" "$(printf 'caf\xe9')"; do
    printf '%s\n' "${bad}" | "${HELPER}" set 2>/dev/null && fail "took a bad line"
done
[[ "$(cat "${ACCOUNTS}")" == "${before}" ]] || fail "a refused line changed the file"
# A second line on standard input is not read: no way to add a key
printf "x\n[org/gnome/desktop/lockdown]\ndisable-lock-screen=true\n" | "${HELPER}" set
grep -q lockdown "${ACCOUNTS}" "${LOGIN}" && fail "a second line got in"

"${HELPER}" clear
[[ ! -e "${ACCOUNTS}" && ! -e "${LOGIN}" ]] || fail "clear left a file"
[[ -z "$("${HELPER}" status)" ]] || fail "a line after clear"
echo "test-if-found: ok"
