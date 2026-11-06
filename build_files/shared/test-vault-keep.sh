#!/usr/bin/bash
# What scripts use to keep data under the Vault's key (/usr/libexec/amethystora-vault-keep), against the stand-in Vault (test-vault-stand-in.py, which really
# encrypts: never the live one): a round trip, a swapped record or purpose refused, each answer of the Vault as its own status, and that what is sealed
# shows nothing of what went in.
set -euo pipefail

SRC="${SRC:-$(cd "$(dirname "$0")/../.." && pwd)}"
KEEP="${KEEP:-${SRC}/system_files/shared/usr/libexec/amethystora-vault-keep}"
[[ -x "${KEEP}" ]] || KEEP=/usr/libexec/amethystora-vault-keep
STAND_IN="${SRC}/build_files/shared/test-vault-stand-in.py"
[[ -f "${STAND_IN}" ]] || STAND_IN=/ctx/build_files/shared/test-vault-stand-in.py
T="$(mktemp -d)"
PID=""
trap 'if [[ -n "${PID}" ]]; then kill "${PID}" 2>/dev/null || true; fi; rm -rf "${T}"' EXIT
fail() { echo "FAIL $*"; exit 1; }
echo open >"${T}/mode"
python3 -I "${STAND_IN}" "${T}/s" "${T}/mode" &
PID=$!
for _ in $(seq 50); do [[ -S "${T}/s" ]] && break; sleep 0.1; done
keep() { python3 -I "${KEEP}" --socket "${T}/s" "$@"; }
status() { local code=0; "$@" >/dev/null 2>&1 || code=$?; echo "${code}"; }

CANARY="canary-vault-keep-$$"
printf '%s' "${CANARY}" | keep seal put-away list >"${T}/sealed"
[[ "$(keep open put-away list <"${T}/sealed")" == "${CANARY}" ]] || fail "the round trip"
! grep -qF "${CANARY}" "${T}/sealed" || fail "the canary is in what was sealed"
[[ "$(head -c 4 "${T}/sealed")" == AVs1 ]] || fail "the sealed form is not the Vault's"
[[ "$(status keep open put-away other <"${T}/sealed")" == 5 ]] || fail "a swapped record was opened"
[[ "$(status keep open charge-pattern list <"${T}/sealed")" == 5 ]] || fail "a swapped purpose was opened"
printf 'x' >"${T}/bad"
[[ "$(status keep open put-away list <"${T}/bad")" == 5 ]] || fail "something that was never sealed was opened"
# The answers of the Vault, each its own status: closed 3, not set up 4, refused 5
for pair in closed:3 not-set-up:4 refused:5; do
    echo "${pair%%:*}" >"${T}/mode"
    [[ "$(printf x | status keep seal put-away list)" == "${pair##*:}" ]] || fail "${pair%%:*} is not status ${pair##*:}"
done
# The background key works with the vault closed, and nothing else does
echo background >"${T}/mode"
[[ "$(printf x | status keep seal background r)" == 0 ]] || fail "the background key was refused while closed"
[[ "$(printf x | status keep seal put-away list)" == 3 ]] || fail "another purpose worked while closed"
echo open >"${T}/mode"
# What a script may ask: a purpose of letters, digits and dashes, a record without control characters, never data as an argument
[[ "$(printf x | status keep seal 'Bad Purpose' r)" == 7 ]] || fail "a bad purpose was sent"
[[ "$(printf x | status keep seal put-away "$(printf 'a\nb')")" == 7 ]] || fail "a record with a line break was sent"
[[ "$(status python3 -I "${KEEP}" --socket "${T}/missing" open put-away list <"${T}/sealed")" == 7 ]] || fail "no service is a failure"
[[ "$(status python3 -I "${KEEP}" seal put-away)" == 2 ]] || fail "usage"
head -c 2000000 /dev/zero >"${T}/big"
[[ "$(status keep seal put-away list <"${T}/big")" == 6 ]] || fail "more than a megabyte was sealed"
# Two accounts' keys differ in the real service; here, a new purpose is a new key: the same text seals to another result
printf '%s' "${CANARY}" | keep seal other-purpose list >"${T}/sealed2"
! cmp -s "${T}/sealed" "${T}/sealed2" || fail "the same text sealed twice is the same bytes"
echo "test-vault-keep: ok"
