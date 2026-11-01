#!/usr/bin/bash
# YARA rules in ClamAV (amethystora-yara), for 20-tests.sh, from stand-in archives in each source's layout:
# what ClamAV cannot load is left out without taking the rules after it along, a rule under a licence that asks
# more than its author's name is left out, Elastic's files stay whole, every set put in ClamAV's folder loads,
# a match names its rule's author, and a source turned off takes its rules away. Run as clamupdate, as the
# service runs it, in a temporary folder.

set -euo pipefail

T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
chmod 0755 "${T}"
fail() { echo "test-yara: $*" >&2; exit 1; }
YARA=/usr/libexec/amethystora-yara
export AMETHYSTORA_YARA_ROOT="${T}/root" AMETHYSTORA_YARA_ARCHIVE="${T}/archive" AMETHYSTORA_SECURITY_CONF="${T}/security.conf"
mkdir -p "${T}/root/clamav" "${T}/root/yara"

# Elastic: one file of rules ClamAV loads, and one that needs a module, which goes whole
mkdir -p "${T}/e/protections-artifacts-main/yara/rules"
cat >"${T}/e/protections-artifacts-main/yara/rules/Linux_Trojan_Test.yar" <<'EOF'
rule Linux_Trojan_Test_1 {
    meta:
        author = "Elastic Security"
        license = "Elastic License v2"
    strings:
        $a = "amethystora-yara-test-elastic"
    condition:
        $a
}
EOF
cat >"${T}/e/protections-artifacts-main/yara/rules/Linux_Trojan_Module.yar" <<'EOF'
import "elf"
rule Linux_Trojan_Module_1 {
    strings:
        $a = "never"
    condition:
        $a and elf.number_of_sections > 1
}
EOF
tar -C "${T}/e" -czf "${AMETHYSTORA_YARA_ARCHIVE}.elastic" .

# signature-base: a rule that loads, one ClamAV cannot parse and one after it, which has to survive, one under
# a non-commercial licence, one with a module, and a file that needs LOKI's variables
mkdir -p "${T}/s/signature-base-master/yara"
cat >"${T}/s/signature-base-master/yara/apt_test.yar" <<'EOF'
rule Good_Rule {
    meta:
        author = "Florian Roth"
    strings:
        $a = "amethystora-yara-test-sigbase"
    condition:
        $a
}
rule Broken_Rule {
    strings:
        $a = "broken-marker"
    condition:
        $a and nonsense(
}
rule After_Broken {
    meta:
        author = "Someone \"Quoted\""
    strings:
        $a = "after-broken-marker"
    condition:
        $a
}
rule Not_For_Sale {
    meta:
        author = "X"
        license = "CC BY-NC 4.0"
    strings:
        $a = "nc-marker-string"
    condition:
        $a
}
rule Needs_Pe {
    strings:
        $a = "pe-marker-string"
    condition:
        $a and pe.is_dll()
}
EOF
cp "${T}/s/signature-base-master/yara/apt_test.yar" "${T}/s/signature-base-master/yara/loki_only.yar"
echo "loki_only.yar" >"${T}/s/signature-base-master/yara/external-variable-rules.txt"
tar -C "${T}/s" -czf "${AMETHYSTORA_YARA_ARCHIVE}.signature-base" .

# YARA Forge: a repository under MIT and one with no licence
python3 -I - "${AMETHYSTORA_YARA_ARCHIVE}.forge" <<'EOF'
import sys, zipfile
text = '''import "pe"
/*
 * YARA Rule Set
 * Repository Name: Free
 * Repository: https://example.org/free
 * License: MIT License. Permission is hereby granted, free of charge, to any person
 */
rule FREE_Rule : test {
    meta:
        author = "Free Author"
    strings:
        $a = "amethystora-yara-test-forge"
    condition:
        $a
}
/*
 * YARA Rule Set
 * Repository Name: Closed
 * Repository: https://example.org/closed
 * NO LICENSE SET
 */
rule CLOSED_Rule {
    strings:
        $a = "closed-marker-string"
    condition:
        $a
}
'''
with zipfile.ZipFile(sys.argv[1], "w") as archive:
    archive.writestr("packages/core/yara-rules-core.yar", text)
EOF
chown -R clamupdate "${T}"
as_clamupdate() {
    setpriv --reuid=clamupdate --regid=clamupdate --clear-groups env AMETHYSTORA_YARA_ROOT="${AMETHYSTORA_YARA_ROOT}" \
        AMETHYSTORA_YARA_ARCHIVE="${AMETHYSTORA_YARA_ARCHIVE}" AMETHYSTORA_SECURITY_CONF="${AMETHYSTORA_SECURITY_CONF}" "${YARA}" "$@"
}

: >"${AMETHYSTORA_SECURITY_CONF}"
"${YARA}" due && fail "something to do with every source off"
printf 'YARA_ELASTIC=on\nYARA_SIGNATURE_BASE=on\nYARA_FORGE=on\n' >"${AMETHYSTORA_SECURITY_CONF}"
"${YARA}" due || fail "nothing to do with the sources on"
as_clamupdate update >/dev/null || fail "the update failed"

DB="${T}/root/clamav"
names() { sed -nE 's/^[[:space:]]*rule[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*/\1/p' "$1" | sort | tr '\n' ' '; }
[[ "$(names "${DB}/amethystora-yara-elastic.yar")" == "Linux_Trojan_Test_1 " ]] || fail "elastic: $(names "${DB}/amethystora-yara-elastic.yar")"
cmp -s <(head -n 9 "${DB}/amethystora-yara-elastic.yar") "${T}/e/protections-artifacts-main/yara/rules/Linux_Trojan_Test.yar" ||
    fail "Elastic's file was changed"
[[ "$(names "${DB}/amethystora-yara-signature-base.yar")" == "SigBase_After_Broken SigBase_Good_Rule " ]] ||
    fail "signature-base: $(names "${DB}/amethystora-yara-signature-base.yar")"
[[ "$(names "${DB}/amethystora-yara-forge.yar")" == "Forge_FREE_Rule " ]] || fail "forge: $(names "${DB}/amethystora-yara-forge.yar")"
for file in "${DB}"/*.yar; do
    errors="$(clamscan --quiet --no-summary -d "${file}" "${T}/root/yara/empty" 2>&1 >/dev/null)" && [[ -z "${errors}" ]] ||
        fail "$(basename "${file}") does not load: ${errors}"
done
grep -qxP 'SigBase_Good_Rule\tsignature-base\tFlorian Roth\tDetection Rule License 1.1' "${T}/root/yara/credits.tsv" ||
    fail "the credits: $(cat "${T}/root/yara/credits.tsv")"
grep -qxP 'Forge_FREE_Rule\tYARA Forge, from Free\tFree Author\tMIT' "${T}/root/yara/credits.tsv" || fail "Forge's credits"

# A match names its rule's author, in a scan's lines and on its own
echo amethystora-yara-test-sigbase >"${T}/sample"
LINE="$(clamscan --no-summary -d "${DB}" "${T}/sample" | AMETHYSTORA_YARA_ROOT="${T}/root" "${YARA}" credit)" || true
[[ "${LINE}" == "${T}/sample: YARA.SigBase_Good_Rule.UNOFFICIAL FOUND (YARA rule by Florian Roth, Detection Rule License 1.1, signature-base)" ]] ||
    fail "a match's credit: ${LINE}"
[[ "$("${YARA}" credit YARA.Forge_FREE_Rule.UNOFFICIAL)" == "YARA rule by Free Author, MIT, YARA Forge, from Free" ]] ||
    fail "a signature's credit"
[[ -z "$("${YARA}" credit Eicar-Signature)" ]] || fail "a signature that is no YARA rule's got a credit"

# Off takes the rules away, and leaves nothing to do
printf 'YARA_ELASTIC=on\n' >"${AMETHYSTORA_SECURITY_CONF}"
as_clamupdate update >/dev/null || fail "the update after turning two off failed"
[[ ! -e "${DB}/amethystora-yara-forge.yar" && ! -e "${DB}/amethystora-yara-signature-base.yar" ]] || fail "a source turned off kept its rules"
: >"${AMETHYSTORA_SECURITY_CONF}"
"${YARA}" due || fail "nothing to do with a source's rules left behind"
as_clamupdate update >/dev/null
"${YARA}" due && fail "something to do with every source off and taken away"

# A run that was stopped leaves its pieces. They count as something to do with every source off, the next run takes
# them away, and what a link of that name leads to is left alone
mkdir -p "${T}/root/yara/forge.pieces.ABC123/one" "${T}/kept"
touch "${T}/kept/file"
ln -s "${T}/kept" "${T}/root/yara/forge.pieces.LINK12"
: >"${T}/root/yara/credits.tsv.new"
chown -R clamupdate "${T}/root/yara"
"${YARA}" due || fail "nothing to do with a stopped run's pieces left behind"
as_clamupdate update >/dev/null
[[ ! -e "${T}/root/yara/forge.pieces.ABC123" ]] || fail "a stopped run's pieces stayed"
[[ -e "${T}/kept/file" ]] || fail "what a link leads to was taken away"
[[ ! -e "${T}/root/yara/credits.tsv.new" ]] || fail "a stopped run's credits stayed"

echo "test-yara: what ClamAV loads, and only that, with its authors named"
