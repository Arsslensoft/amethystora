#!/usr/bin/bash
# Your year (R7-30): what /usr/libexec/amethystora-your-year works out from stand-ins, that it leaves out what it cannot
# read, that it writes nothing, and that no number of it is kept. Run by 20-tests.sh; HELPER names another copy.
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-your-year}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
export AMETHYSTORA_YEAR_ROOT="${T}"
mkdir -p "${T}/usr/bin" "${T}/usr/libexec" "${T}/etc" "${T}/var/lib/amethystora/security"
export HOME="${T}/home"
mkdir -p "${HOME}"

fail() {
    echo "test-your-year: $*"
    exit 1
}

YEAR="$(date +%Y)"
[[ "$(date +%-m)" == 1 ]] && YEAR=$((YEAR - 1))
ago="$(($(date +%s) - 40 * 86400))"
# Born when /var was made, never /etc/machine-id, which each new deployment copies: stat answers for /var alone
printf '#!/usr/bin/bash\n[[ "${3}" == "${AMETHYSTORA_YEAR_ROOT}/var" ]] || exit 1\necho %s\n' "${ago}" >"${T}/usr/bin/stat"
# The journal says a finished run for every run of the updater; nothing of it is read, so the page says nothing of updates
printf '#!/usr/bin/bash\nprintf "Updates Completed Successfully\\nUpdates Completed Successfully\\n"\n' >"${T}/usr/bin/journalctl"
printf '#!/usr/bin/bash\necho %s\n' "'{\"snapshots\": {\"count\": 12}, \"backup\": {\"last_success\": ${ago}}}'" >"${T}/usr/libexec/amethystora-restore"
chmod +x "${T}/usr/bin/stat" "${T}/usr/bin/journalctl" "${T}/usr/libexec/amethystora-restore"
now="$(date -d "${YEAR}-06-15" +%s)"
cat >"${T}/var/lib/amethystora/security/events.json" <<EOF
[{"time": ${now}, "level": "critical", "text": "canary-word"}, {"time": ${now}, "level": "quiet", "text": "x"},
 {"time": ${now}, "level": "normal", "text": "z"}, {"time": $((now - 2 * 366 * 86400)), "level": "critical", "text": "y"}]
EOF

before="$(find "${T}" -type f | sort | xargs sha256sum)"
answer="$("${HELPER}" --json)"
python3 - "${answer}" "${YEAR}" <<'EOF' || fail "the numbers are not what the stand-ins say: ${answer}"
import json, sys
a = json.loads(sys.argv[1])
assert a['year'] == int(sys.argv[2])
assert 39 <= a['days'] <= 41
assert 'updates' not in a
assert a['snapshots'] == 12
assert a['last_backup'] > 0
assert a['findings'] == 2
EOF
[[ "${answer}" != *canary-word* ]] || fail "words of the history reached the answer"
page="$("${HELPER}")"
grep -q "Together for" <<<"${page}" || fail "the page has no days: ${page}"
grep -q "Nothing was sent" <<<"${page}" || fail "the page does not say nothing was sent"
grep -qi "update" <<<"${page}" && fail "the page counts updates: ${page}"
grep -q "lately" <<<"${page}" || fail "the page does not say the security count is of lately: ${page}"
[[ "$(find "${T}" -type f | sort | xargs sha256sum)" == "${before}" ]] || fail "it wrote or changed a file"
[[ -z "$(find "${T}" -newer "${T}/usr/libexec/amethystora-restore" -type f ! -path '*/usr/*' ! -path '*/var/*')" ]] || fail "it left a file"

# A birth that cannot be (in the future) gives no days, rather than a false number
printf '#!/usr/bin/bash\necho %s\n' "$(($(date +%s) + 30 * 86400))" >"${T}/usr/bin/stat"
python3 -c 'import json,sys; sys.exit(json.loads(sys.argv[1])["days"] is not None)' "$("${HELPER}" --json)" ||
    fail "days from a birth in the future"

# What it cannot read is left out, and the page still comes
rm -f "${T}/var/lib/amethystora/security/events.json" "${T}/usr/bin/journalctl" "${T}/usr/libexec/amethystora-restore" "${T}/usr/bin/stat"
answer="$("${HELPER}" --json)"
python3 - "${answer}" <<'EOF' || fail "what it cannot read is not null: ${answer}"
import json, sys
a = json.loads(sys.argv[1])
assert all(a[key] is None for key in ('days', 'snapshots', 'last_backup', 'findings'))
EOF
page="$("${HELPER}")"
[[ "$(wc -l <<<"${page}")" == 2 ]] || fail "lines it cannot know were printed: ${page}"
grep -qE '\bopen\(.*["'"'"'][wa]|mkstemp|NamedTemporaryFile|urllib|socket' "${HELPER}" && fail "the helper writes or connects"
echo "test-your-year: ok"
