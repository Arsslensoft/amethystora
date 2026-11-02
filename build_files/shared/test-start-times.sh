#!/usr/bin/bash
# amethystora-start-times against stand-ins for systemd and the journal: what it records and what it leaves out, that the
# file holds a date and a number and nothing else and is the administrators' alone, that a full disk keeps the history, that
# status changes nothing and never waits on the deployments, and that it says what it measured, and since when, when the
# numbers look bad as well as when they look good. Run by 20-tests.sh; needs jq, awk and gettext.
set -euo pipefail

HELPER="${1:-/usr/libexec/amethystora-start-times}"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT
chmod 0755 "${WORK}"
mkdir "${WORK}/bin" "${WORK}/homes" "${WORK}/homes/someone"
export AMETHYSTORA_START_TIMES="${WORK}/state/seconds.tsv"
export AMETHYSTORA_START_STAMP="${WORK}/run/recorded"
export AMETHYSTORA_START_WANTS="${WORK}/wants"
export AMETHYSTORA_HOMES="${WORK}/homes"
export AMETHYSTORA_DEPLOYMENTS="${WORK}/bin/deployments"
export PATH="${WORK}/bin:${PATH}"
export LANGUAGE=C
FILE="${AMETHYSTORA_START_TIMES}"
STAMP="${AMETHYSTORA_START_STAMP}"

fail() { echo "test-start-times: $*" >&2; exit 1; }
same() { [[ "$2" == "$3" ]] || fail "$1: expected '$3', found '$2'"; }
# One start: a new boot has no stamp
record() { rm -f "${STAMP}"; "${HELPER}" record; }

# Text from others goes through the security cleaner: the image's own, or a stand-in that only cuts
if [[ -x "${HELPER%/*}/amethystora-security-text" ]]; then
    export AMETHYSTORA_SECURITY_TEXT="${HELPER%/*}/amethystora-security-text"
else
    printf '#!/usr/bin/bash\nshift 2\nprintf "%%s" "$*" | tr -d "\\033"\n' >"${WORK}/bin/security-text"
    export AMETHYSTORA_SECURITY_TEXT="${WORK}/bin/security-text"
fi

# The stand-ins: what each says is in a file of the test's own
cat >"${WORK}/bin/systemctl" <<EOF
#!/usr/bin/bash
case "\$*" in
    "is-system-running --wait") echo degraded; exit 1 ;;
    *SoftRebootsCount*) cat "${WORK}/soft" ;;
    *FinishTimestampMonotonic*) cat "${WORK}/finish" ;;
    *) exit 1 ;;
esac
EOF
cat >"${WORK}/bin/journalctl" <<EOF
#!/usr/bin/bash
cat "${WORK}/journal"
EOF
cat >"${WORK}/bin/systemd-analyze" <<EOF
#!/usr/bin/bash
printf '2min 3.000s systemd-cryptsetup@luks\\\\x2d1234.service\n1min 8.210s NetworkManager-wait-online.service\n  3.100s plymouth-quit-wait.service\n  1.500s dracut-initqueue.service\n  0.900s other.service\n  0.500s last.service\n'
EOF
cat >"${AMETHYSTORA_DEPLOYMENTS}" <<EOF
#!/usr/bin/bash
[[ ! -e "${WORK}/slow-deployments" ]] || sleep 5
cat "${WORK}/deployments"
EOF
# The machine's first home was made at the time in a file
cat >"${WORK}/bin/stat" <<EOF
#!/usr/bin/bash
if [[ "\$*" == *%W* && -e "${WORK}/born" ]]; then
    cat "${WORK}/born"
else
    exec /usr/bin/stat "\$@"
fi
EOF
chmod +x "${WORK}"/bin/*
echo 0 >"${WORK}/soft"
: >"${WORK}/journal"
echo '{"deployments": []}' >"${WORK}/deployments"

begun=7d4958e842da4a758f6c1cdc7b36dcc5
done_=39f53479d3a045ac8e11786248231fbf
line() { printf '{"UNIT": "%s", "MESSAGE_ID": "%s", "__MONOTONIC_TIMESTAMP": "%s"}\n' "$1" "$2" "$3"; }

# --- recording ----------------------------------------------------------------------------------------------------
echo 15000000 >"${WORK}/finish"
record
same "a start with no disk to unlock" "$(cut -f2 "${FILE}")" 15.0
same "the date, and nothing of the time" "$(cut -f1 "${FILE}")" "$(date +%F)"
[[ "$(awk -F'\t' '{ print NF }' "${FILE}")" == 2 ]] || fail "the file holds more than a date and a number"
grep -qE ':[0-9]{2}|T[0-9]' "${FILE}" && fail "a time of day in the file"
# A calendar of the machine's use is for its administrators (wheel) alone
same "the file is root's and wheel's" "$(stat -c %a "${FILE}")" 640
same "...in a folder only they can enter" "$(stat -c %a "${FILE%/*}")" 750
if getent group wheel >/dev/null && [[ "$(id -u)" == 0 ]]; then
    same "the file's group" "$(stat -c %G "${FILE}")" wheel
fi

# A restart of the unit in the same boot is the same start
"${HELPER}" record
same "a start is measured once a boot" "$(wc -l <"${FILE}")" 1

# Two disks' unlocks overlapping (2 s to 5 s, 4 s to 6 s) count once; the same unit starting again later does not count
{
    line 'systemd-cryptsetup@a.service' "${begun}" 2000000
    line 'systemd-cryptsetup@b.service' "${begun}" 4000000
    line 'systemd-cryptsetup@a.service' "${done_}" 5000000
    line 'systemd-cryptsetup@b.service' "${done_}" 6000000
    line 'systemd-cryptsetup@a.service' "${begun}" 9000000
    line 'systemd-cryptsetup@a.service' "${done_}" 9100000
} >"${WORK}/journal"
record
same "the time spent unlocking is left out" "$(tail -n 1 "${FILE}" | cut -f2)" 11.0
: >"${WORK}/journal"

# A soft reboot starts no kernel, and a start that has not finished has nothing to say
echo 1 >"${WORK}/soft"
record
same "a soft reboot is not a start" "$(wc -l <"${FILE}")" 2
echo 0 >"${WORK}/soft"
echo 0 >"${WORK}/finish"
record
same "an unfinished start is not recorded" "$(wc -l <"${FILE}")" 2

# The first line stays for good, and at most 999 follow it
{
    printf '2020-01-01\t9.9\n'
    for ((i = 0; i < 1500; i++)); do printf '2024-01-02\t20.0\n'; done
} >"${FILE}"
echo 12000000 >"${WORK}/finish"
record
same "the file's length" "$(wc -l <"${FILE}")" 1000
same "the first start stays" "$(head -n 1 "${FILE}")" "$(printf '2020-01-01\t9.9')"
same "the last start is there" "$(tail -n 1 "${FILE}" | cut -f2)" 12.0

# The baseline stays: the rest of the first week is kept when the file is trimmed, so that it does not become the
# first boot alone (the slowest, usually) and a machine that got slower does not read as "still as fast"
{
    for d in 01 02 03 04 05 06 07; do printf '2020-01-%s\t10.0\n2020-01-%s\t11.0\n' "${d}" "${d}"; done
    for ((i = 0; i < 1300; i++)); do printf '2024-01-02\t30.0\n'; done
} >"${FILE}"
record
same "the file's length, with a week to keep" "$(wc -l <"${FILE}")" 1000
same "the first week is kept" "$(grep -c '^2020-01-0[1-7]' "${FILE}")" 14
same "...and the baseline with it" "$("${HELPER}" status --json | jq -r ".first_week | floor")" 10
same "...so that it still reads as slower" "$("${HELPER}" status --json | jq -r .slower)" true

# A full disk keeps the history as it was, and is not a failed unit: the file size limit stands in for it
{
    printf '2020-01-01\t9.9\n'
    for ((i = 0; i < 999; i++)); do printf '2024-01-02\t20.0\n'; done
} >"${FILE}"
before="$(sha256sum "${FILE}")"
rm -f "${STAMP}"
( ulimit -f 4; trap '' XFSZ; "${HELPER}" record ) || fail "a full disk should not fail the unit"
same "a full disk loses nothing" "$(sha256sum "${FILE}")" "${before}"
same "...and leaves nothing behind" "$(find "${FILE%/*}" -name 'seconds.tsv.*' | wc -l)" 0
[[ ! -e "${STAMP}" ]] || fail "a start that was not written is not marked as measured"

# --- reading ------------------------------------------------------------------------------------------------------
rm -f "${FILE}"
[[ "$("${HELPER}" status --json | jq -r .count)" == 0 ]] || fail "nothing measured should count nothing"
"${HELPER}" status | grep -q "Nothing has been measured yet" || fail "nothing measured should say so"
[[ ! -e "${FILE}" ]] || fail "status wrote something"

mkdir -p "${WORK}/state"
days() { # FIRST COUNT SECONDS: one start a day
    local i
    for ((i = 0; i < $2; i++)); do printf '%s\t%s\n' "$(date -d "$1 +${i} days" +%F)" "$3"; done
}
touch "${AMETHYSTORA_START_WANTS}"
days 2026-01-01 2 12.3 >"${FILE}"
out="$("${HELPER}" status --json)"
same "early on it says what it measured" "$(jq -r .title <<<"${out}")" "It starts in 12 seconds"
same "...and promises nothing yet" "$(jq -r .slower <<<"${out}")" false
days 2026-01-01 2 1.2 >"${FILE}"
same "one second is not seconds" "$("${HELPER}" status --json | jq -r .title)" "It starts in 1 second"
rm -f "${AMETHYSTORA_START_WANTS}"
days 2026-01-01 2 12.3 >"${FILE}"
out="$("${HELPER}" status --json)"
same "not measuring any more, it says the last one" "$(jq -r .title <<<"${out}")" "The last start measured took 12 seconds"
same "...and so in its flag" "$(jq -r .measuring <<<"${out}")" false
touch "${AMETHYSTORA_START_WANTS}"

# A line that is not a date and a number is left out, not a reason to fail. The machine's first home is a day older than
# its first start, so that "day one" is true
{ days 2026-01-01 7 12.0; echo 'not a line'; printf 'x\t1\n'; days 2026-01-20 5 12.5; } >"${FILE}"
date -d 2025-12-31 +%s >"${WORK}/born"
before="$(sha256sum "${FILE}")"
out="$("${HELPER}" status --json)"
same "steady, on a machine measured from its first day" "$(jq -r .title <<<"${out}")" "Still as fast as day one"
same "bad lines are left out" "$(jq -r .count <<<"${out}")" 12
same "steady is not slower" "$(jq -r .slower <<<"${out}")" false
same "status changes nothing" "$(sha256sum "${FILE}")" "${before}"
fields="$("${HELPER}" status --fields)"
same "four lines for the list: title, text, points, slower" "$(wc -l <<<"${fields}")" 4
jq -e 'length == 12 and .[0][0] == 0 and .[11][0] == 23' <<<"$(sed -n 3p <<<"${fields}")" >/dev/null || fail "the graph's points by date: ${fields}"
same "...and not slower" "$(sed -n 4p <<<"${fields}")" false

# A machine updated into this, or one whose homes say nothing of its first day, says since when it measured: never "day one"
date -d 2025-01-01 +%s >"${WORK}/born"
out="$("${HELPER}" status --json)"
same "measured later than the first week of the machine" "$(jq -r .title <<<"${out}")" "Still as fast as when it began to be measured"
jq -r .text <<<"${out}" | grep -q "Measured since 2026-01-01" || fail "it should say since when: ${out}"
jq -r .text <<<"${out}" | grep -qi "day one" && fail "day one is not true here: ${out}"
echo 0 >"${WORK}/born"
same "no home to say" "$("${HELPER}" status --json | jq -r .title)" "Still as fast as when it began to be measured"
date -d 2025-12-31 +%s >"${WORK}/born"
days 2026-01-01 7 1.0 >"${FILE}"; days 2026-01-20 4 1.0 >>"${FILE}"
jq -r .text <<<"$("${HELPER}" status --json)" | grep -q "This start took 1 second\. In its first week it took 1 second\." || fail "one second is one second"

# A start a quarter longer is called slower; a little longer is a little slower, and never "still as fast"
{ days 2026-01-01 7 12.0; days 2026-02-01 6 14.9; } >"${FILE}"
out="$("${HELPER}" status --json)"
same "12 to 14.9 seconds is not called slower" "$(jq -r .slower <<<"${out}")" false
same "...but not as fast as it was either" "$(jq -r .title <<<"${out}")" "It starts a little more slowly than it did"
jq -r .text <<<"${out}" | grep -q "cannot wear out" && fail "no promise over a slower start: ${out}"
{ days 2026-01-01 7 12.0; days 2026-02-01 6 13.0; } >"${FILE}"
same "12 to 13 seconds is the same" "$("${HELPER}" status --json | jq -r .title)" "Still as fast as day one"
{ days 2026-01-01 7 12.0; days 2026-02-01 6 14.0; } >"${FILE}"
same "12 to 14 seconds is not called slower" "$("${HELPER}" status --json | jq -r .slower)" false
same "...and is a little slower" "$("${HELPER}" status --json | jq -r .title)" "It starts a little more slowly than it did"
{ days 2026-01-01 7 4.0; days 2026-02-01 6 6.5; } >"${FILE}"
same "4 to 6.5 seconds is not called slower" "$("${HELPER}" status --json | jq -r .slower)" false

# Slower is said, with the numbers; what changed is asked for, never read by the list, the doctor's first read or status
{ days 2026-01-01 7 12.0; days 2026-02-01 6 14.0; days 2026-03-01 6 30.0; } >"${FILE}"
printf '{"deployments": [{"booted": true, "timestamp": %s, "version": "45.20260301"}]}\n' "$(date -d 2026-03-01 +%s)" >"${WORK}/deployments"
touch "${WORK}/slow-deployments"
begin="$(date +%s)"
out="$("${HELPER}" status --json)"
fields="$("${HELPER}" status --fields)"
(($(date +%s) - begin < 3)) || fail "status waited for what changed"
same "slower" "$(jq -r .slower <<<"${out}")" true
same "slower is the title" "$(jq -r .title <<<"${out}")" "It starts more slowly than it did"
jq -r .text <<<"${out}" | grep -q "takes 30 seconds.*it took 12" || fail "the bad number should be said: ${out}"
same "...and in the list's fourth line" "$(sed -n 4p <<<"${fields}")" true
[[ "$(jq -c '.points | length' <<<"${out}")" == 19 ]] || fail "every date is in the graph: ${out}"
rm -f "${WORK}/slow-deployments"
detail="$("${HELPER}" why)"
grep -q "updated to 45.20260301 on 2026-03-01" <<<"${detail}" || fail "the update should be named: ${detail}"
# The disk's unlock is what the number leaves out, and the splash only waits for the rest: neither is blamed
grep -q "NetworkManager-wait-online.service (1min 8.210s), dracut-initqueue.service (1.500s), other.service (0.900s)" <<<"${detail}" ||
    fail "what took longest should be named, without the password or the splash: ${detail}"
grep -qE 'cryptsetup|luks|plymouth|last\.service' <<<"${detail}" && fail "only the three longest, and not the disk's unlock: ${detail}"

# The update's version is text from the image: cleaned before a terminal
printf '{"deployments": [{"booted": true, "timestamp": %s, "version": "45.\\u001b[31mred"}]}\n' "$(date -d 2026-03-01 +%s)" >"${WORK}/deployments"
detail="$("${HELPER}" why)"
grep -q "updated to 45\." <<<"${detail}" || fail "the update should be named: ${detail}"
grep -q $'\033' <<<"${detail}" && fail "a control character reached the terminal"

# Slower with no update behind it names no update, and when it is not slower there is nothing to ask
printf '{"deployments": [{"booted": true, "timestamp": %s, "version": "45.20251201"}]}\n' "$(date -d 2025-12-01 +%s)" >"${WORK}/deployments"
"${HELPER}" why | grep -q "updated to" && fail "an update that changed nothing should not be blamed"
[[ "$("${HELPER}" status --json | jq -r .slower)" == true ]] || fail "slower still"
days 2026-01-01 14 12.0 >"${FILE}"
same "nothing got slower, nothing to say" "$("${HELPER}" why)" ""

# An account that is not an administrator is not shown the history, and is not told that nothing was measured
if [[ "$(id -u)" == 0 ]] && command -v setpriv >/dev/null; then
    days 2026-01-01 14 12.0 >"${FILE}"
    chmod 0640 "${FILE}"
    chmod 0750 "${FILE%/*}"
    chgrp 0 "${FILE%/*}" "${FILE}"
    as_nobody() { setpriv --reuid=65534 --regid=65534 --clear-groups "${HELPER}" "$@"; }
    same "no card for it" "$(as_nobody status --fields)" ""
    same "no numbers for it" "$(as_nobody status --json)" '{"unreadable":true}'
    as_nobody status | grep -q "Nothing has been measured" && fail "an unreadable history is not an empty one"
    as_nobody status | grep -q "administrators" || fail "it should say who may read it"
fi

echo "test-start-times: all passed"
