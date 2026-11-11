#!/usr/bin/bash
# amethystora-quarantine against an account that swaps its own folder, or the found file, for a link between
# the scan and the act, for 20-tests.sh. Nothing outside the account's own files may change: not by the
# quarantine, not by a delete, not by a restore, not by trusting a file. Runs as root, as the helper does,
# with a stand-in clamdscan that finds every file named *.bad and then does what the account would.

set -euo pipefail

HELPER="${1:-/usr/libexec/amethystora-quarantine}"
T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
chmod 0755 "${T}"
export AMETHYSTORA_QUARANTINE_ROOT="${T}/root"
export AMETHYSTORA_SECURITY_CONF="${T}/security.conf"
mkdir -p "${T}/root/var/lib/clamav" "${T}/bin" "${T}/home/mallory/dir" "${T}/system"
chown 4400:4400 "${T}/home/mallory" "${T}/home/mallory/dir"
fail() { echo "test-quarantine: $*" >&2; exit 1; }

# What the account must never reach: a file of the system's, with its mode
echo secret >"${T}/system/passwd"
chmod 0644 "${T}/system/passwd"
cp "${T}/system/passwd" "${T}/system/evil.bad"
system_untouched() {
    [[ -f "${T}/system/passwd" && "$(stat -c '%a %u' "${T}/system/passwd")" == "644 0" ]] || fail "$1: the system's file changed"
    [[ -f "${T}/system/evil.bad" ]] || fail "$1: the system's file was moved or deleted"
}

cat >"${T}/bin/clamdscan" <<'EOF'
#!/usr/bin/bash
# Finds every *.bad file it is asked about, then runs what the test put in SWAP, as the account would
[[ "$1" == --reload ]] && exit 0
for arg in "$@"; do
    [[ "${arg}" == *.bad ]] && echo "${arg}: Test.Signature FOUND"
    [[ "${arg}" == *.yarabad ]] && echo "${arg}: YARA.Poisoned_Rule.UNOFFICIAL FOUND"
done
[[ -n "${SWAP:-}" ]] && bash -c "${SWAP}"
exit 1
EOF
cat >"${T}/bin/sigtool" <<'EOF'
#!/usr/bin/bash
echo "0000:1:$2"
EOF
chmod 0755 "${T}/bin/clamdscan" "${T}/bin/sigtool"
export PATH="${T}/bin:${PATH}"
M="${T}/home/mallory"
plant() { echo malware >"${M}/dir/evil.bad"; chown 4400:4400 "${M}/dir/evil.bad"; chmod 0640 "${M}/dir/evil.bad"; }

for mode in quarantine delete; do
    echo "ON_DETECTION=${mode}" >"${AMETHYSTORA_SECURITY_CONF}"

    # The folder becomes a link to the system's, between the scan and the act
    plant
    out="$(SWAP="mv ${M}/dir ${M}/dir.real; ln -s ${T}/system ${M}/dir" "${HELPER}" act "${M}/dir/evil.bad")"
    [[ "${out}" == failed* ]] || fail "${mode}: acted through a swapped folder: ${out}"
    system_untouched "${mode} through a folder"
    rm -f "${M}/dir"
    mv "${M}/dir.real" "${M}/dir"

    # The file becomes a link to the system's file
    plant
    out="$(SWAP="rm ${M}/dir/evil.bad; ln -s ${T}/system/passwd ${M}/dir/evil.bad" "${HELPER}" act "${M}/dir/evil.bad")"
    [[ "${out}" == failed* ]] || fail "${mode}: acted on a link swapped in for the file: ${out}"
    system_untouched "${mode} through the file"
    rm -f "${M}/dir/evil.bad"

    # A path through a link is refused before anything is scanned
    ln -sfn "${T}/system" "${M}/link"
    out="$("${HELPER}" act "${M}/link/evil.bad")"
    [[ "${out}" == failed* ]] || fail "${mode}: acted through a link in the path"
    system_untouched "${mode} through a path"
    rm -f "${M}/link"

    # And what it is for still works
    plant
    out="$("${HELPER}" act "${M}/dir/evil.bad")"
    [[ "${out}" == "${mode/%e/ed}"* || "${out}" == quarantined* ]] || fail "${mode}: a found file was not ${mode}d: ${out}"
    [[ ! -e "${M}/dir/evil.bad" ]] || fail "${mode}: the found file is still there"
done

# A match that comes only from a downloaded YARA rule is reported and nothing more, whatever ON_DETECTION says: a poisoned
# rule that matches every file must not be able to quarantine or delete them
for mode in quarantine delete; do
    echo "ON_DETECTION=${mode}" >"${AMETHYSTORA_SECURITY_CONF}"
    echo document >"${M}/dir/mine.yarabad"
    chown 4400:4400 "${M}/dir/mine.yarabad"
    out="$("${HELPER}" act "${M}/dir/mine.yarabad")"
    [[ "${out}" == reported$'\t'YARA.Poisoned_Rule.UNOFFICIAL$'\t'* ]] || fail "${mode}: a YARA-only match was not just reported: ${out}"
    [[ -f "${M}/dir/mine.yarabad" ]] || fail "${mode}: a file matched only by a YARA rule was moved or deleted"
    rm -f "${M}/dir/mine.yarabad"
done

# A restore into a folder that became a link is refused, and the file stays in quarantine
echo "ON_DETECTION=quarantine" >"${AMETHYSTORA_SECURITY_CONF}"
plant
"${HELPER}" act "${M}/dir/evil.bad" >/dev/null
id="$("${HELPER}" list | cut -f1 | tail -n1)"
mv "${M}/dir" "${M}/dir.real"
ln -s "${T}/system" "${M}/dir"
rm -f "${T}/system/evil.bad"
"${HELPER}" restore "${id}" 2>/dev/null && fail "restored through a folder that became a link"
[[ ! -e "${T}/system/evil.bad" ]] || fail "a restore wrote into the system's folder"
[[ "$("${HELPER}" list | cut -f1)" == *"${id}"* ]] || fail "a refused restore lost the file"
rm -f "${M}/dir"
mv "${M}/dir.real" "${M}/dir"
cp "${T}/system/passwd" "${T}/system/evil.bad"

# A restore into a folder that is no longer its owner's is refused too
chown 0:0 "${M}/dir"
"${HELPER}" restore "${id}" 2>/dev/null && fail "restored into a folder its owner no longer has"
chown 4400:4400 "${M}/dir"

# Trusting a file reads no link planted as the allow list, and writes a plain one
ln -s "${T}/system/passwd" "${T}/root/var/lib/clamav/amethystora-restored.sfp"
"${HELPER}" restore "${id}" trust >/dev/null || fail "a plain restore with trust failed"
ALLOWED="${T}/root/var/lib/clamav/amethystora-restored.sfp"
[[ -f "${ALLOWED}" && ! -L "${ALLOWED}" ]] || fail "the allow list is not a plain file"
grep -q secret "${ALLOWED}" && fail "the allow list copied what a planted link pointed to"
system_untouched "trust"
[[ "$(stat -c '%u:%g %a' "${M}/dir/evil.bad")" == "4400:4400 640" ]] || fail "the restored file lost its owner or mode"

echo "test-quarantine: no swapped link takes root anywhere"
