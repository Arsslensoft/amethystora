#!/usr/bin/bash
# Hide what's private (R7-67): what /usr/libexec/amethystora-cover-private finds in Tesseract's own TSV, that its answer
# holds places and kinds and never words, and that the shell's part covers solidly and writes nothing. Run by 20-tests.sh
# against the image; HELPER and COVER name other copies to try it from a checkout.
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-cover-private}"
COVER="${COVER:-/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/screenshot/cover.js}"

fail() {
    echo "test-cover-private: $*"
    exit 1
}

# One TSV from lines of text, each word 40 pixels wide on a line of its own number
tsv() {
    printf 'level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n'
    local line=0 word
    while IFS= read -r text; do
        line=$((line + 1))
        local n=0
        for word in ${text}; do
            n=$((n + 1))
            printf '5\t1\t1\t1\t%d\t%d\t%d\t%d\t40\t10\t90\t%s\n' "${line}" "${n}" "$((n * 50))" "$((line * 20))" "${word}"
        done
    done
}

kinds() {
    tsv | "${HELPER}" tsv | python3 -c 'import json,sys; print(" ".join(b["kind"] for b in json.load(sys.stdin)["boxes"]))'
}

expect() {
    local want="$1" text="$2" got
    got="$(kinds <<<"${text}")"
    [[ "${got}" == "${want}" ]] || fail "${text}: found '${got}', wanted '${want}'"
}

expect "address" "To: sam.jones@example.org"
expect "address address" "a@example.org and b+c@mail.example.co.uk"
expect "card" "Card 4111 1111 1111 1111"
expect "card" "pay 4111-1111-1111-1111 now"
expect "card" "number 4111111111111111"
expect "card" "Card ending 4821 7730"
expect "card" "ending in 7730"
expect "phone" "Call +33 6 12 34 56 78 today"
expect "phone" "Call (555) 010-4477"
expect "phone" "0044 20 7946 0958"
expect "account" "Account GB82 WEST 1234 5698 7654 32"
# A card with its expiry or code on the row, an account with other words after it, a phone with more numbers after it
expect "card" "4111 1111 1111 1111 12/25 John Smith"
expect "card" "5500 0000 0000 0004 01/28 John"
expect "card" "4111 1111 1111 1111 123"
expect "card" "Exp 12/25 4111-1111-1111-1111 123"
expect "account" "DE89 3704 0044 0532 0130 00 BIC COBADEFF"
expect "account" "FR76 3000 6000 0112 3456 7890 189 USD"
expect "phone" "+33 6 12 34 56 78 12 34"
# What only looks like one
expect "" "Version 1.2.3 released 2026-11-02 at 10:42"
expect "" "Total: 42.00, order number 20261102"
expect "" "1234567890123456"
expect "" "table 10 20 30 40 50 60 70 80 90 11 22 33"
expect "phone" "GB82 WEST 1234 5698 7654 33"
expect "" "192.168.1.20 and 10:42:07"
expect "" "Page 3 of 12, 2 November 2026"

# The box is the words', a little over, and the answer never carries a word: only the four numbers and a kind
canary="canary.$RANDOM@example.org"
answer="$(tsv <<<"Plain ${canary}" | "${HELPER}" tsv)"
[[ "${answer}" != *canary* && "${answer}" != *example* && "${answer}" != *Plain* ]] || fail "the answer carries words: ${answer}"
python3 - "${answer}" <<'EOF' || fail "the box is not the word's: ${answer}"
import json, sys
box = json.loads(sys.argv[1])['boxes'][0]
assert sorted(box) == ['h', 'kind', 'w', 'x', 'y'] and box['kind'] == 'address'
assert box['x'] <= 100 and box['x'] + box['w'] >= 140 and box['y'] <= 20 and box['y'] + box['h'] >= 30
EOF

# What cannot be read says so, with no reason that could name anything
out="$(head -c 512 /dev/urandom | "${HELPER}" 2>/dev/null)" && fail "garbage read as a picture"
[[ "${out}" == '{"error": "unreadable"}' ]] || fail "unexpected answer to garbage: ${out}"

# Solid, not blurred; through a pipe, with no file; only after a click
grep -qiE 'blur[^:!,]*\(|GaussianBlur|BlurEffect' "${COVER}" && fail "cover.js blurs"
grep -q "newMark('fill', BLACK" "${COVER}" || fail "cover.js does not cover with a filled mark"
grep -qE 'replace_contents|create_tmp|new_tmp|writeToPNG|GLib\.get_tmp_dir|[a-z0-9]_sync\(|file_get_contents' "${COVER}" &&
    fail "cover.js writes a file or blocks"
grep -qE '(NamedTemporaryFile|mkstemp|open\(.*["'"'"']w)' "${HELPER}" && fail "the helper writes a file"
grep -q "Check before you send" "${COVER}" || fail "cover.js does not say to check"
# In a scope with a memory limit, ended with the reading, nothing left running and no core dump of what it read
grep -q 'MemoryMax=' "${COVER}" && grep -q 'systemd-run' "${COVER}" || fail "cover.js does not run the helper in a scope"
grep -q 'send_signal(15)' "${COVER}" || fail "cover.js does not end the helper when a reading is cancelled"
grep -q 'prctl(1, 9)' "${HELPER}" || fail "Tesseract is not tied to the helper's life"
grep -q 'RLIMIT_CORE, (0, 0)' "${HELPER}" || fail "the helper can dump its core"

echo "test-cover-private: ok"
