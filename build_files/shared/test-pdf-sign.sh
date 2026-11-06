#!/usr/bin/bash
# Sign a PDF (R7-42, amethystora-pdf-sign) against stand-ins for the Vault's command line, zenity, notify-send and the
# terminal: what it refuses, that the card (which says a reader calls the signer unknown) comes before anything is signed,
# that a declined card signs nothing, which certificate it asks for, and what a closed vault or a missing one offers.
#
#   test-pdf-sign.sh [LIBEXEC]   LIBEXEC: where the helper is (/usr/libexec)

set -euo pipefail

LIBEXEC="${1:-/usr/libexec}"
T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
fail() { echo "test-pdf-sign: $*" >&2; exit 1; }
same() { [[ "$2" == "$3" ]] || fail "$1: expected '$3', found '$2'"; }

export HOME="${T}/home" PATH="${T}/bin:${PATH}" LOG="${T}/log"
mkdir -p "${HOME}" "${T}/bin" "${T}/docs"
: >"${LOG}"
HELPER="${LIBEXEC}/amethystora-pdf-sign"

# The Vault: cert list from a file, pdf sign as a log that writes the copy as the real one does
cat >"${T}/vault" <<'EOS'
#!/usr/bin/bash
case "$1 $2" in
    "cert list") [[ -z "${VAULT_CODE:-}" ]] || exit "${VAULT_CODE}"; cat "${CERTS}" ;;
    "pdf sign")
        echo "sign $3 $4" >>"${LOG}"
        [[ -z "${SIGN_REFUSED:-}" ]] || exit 1
        out="${3%.pdf} (signed).pdf"; cp "$3" "${out}"; echo "${out}" ;;
esac
EOS
printf '#!/usr/bin/bash\necho "terminal $*" >>"${LOG}"\n' >"${T}/terminal"
# zenity answers by ZENITY_ANSWER (0 yes) and ZENITY_PICK, and keeps the card it was shown
cat >"${T}/bin/zenity" <<'EOS'
#!/usr/bin/bash
echo "zenity $1" >>"${LOG}"
for arg in "$@"; do [[ "${arg}" == --text=* ]] && printf '%s\n' "${arg#--text=}" >"${CARD}.$((++N_CARD))"; done
[[ "$1" == --list ]] && echo "${ZENITY_PICK:-}"
exit "${ZENITY_ANSWER:-0}"
EOS
printf '#!/usr/bin/bash\necho "notify $*" >>"${LOG}"\n' >"${T}/bin/notify-send"
chmod +x "${T}/vault" "${T}/terminal" "${T}/bin/zenity" "${T}/bin/notify-send"
export AMETHYSTORA_VAULT_CLI="${T}/vault" AMETHYSTORA_IN_TERMINAL="${T}/terminal" CERTS="${T}/certs" CARD="${T}/card"
printf 'abc123\tMy signature\tpdf\tCN=Ann\t2030-01-01\n' >"${CERTS}"
printf 'zzz999\tCode\tcode\tCN=Ann\t2030-01-01\n' >>"${CERTS}"

printf '%%PDF-1.7\nbody\n' >"${T}/docs/Lease.pdf"
printf 'not a pdf\n' >"${T}/docs/note.txt"
printf '%%PDF-1.7\n' >"${T}/docs/Two.pdf"
ln -s "${T}/docs/Lease.pdf" "${T}/docs/link.pdf"

[[ "$("${HELPER}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "no arguments is a usage error"
[[ "$("${HELPER}" "${T}/docs/Lease.pdf" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "paths come after --"

# Not a PDF, or a link: nothing is asked and nothing is signed
: >"${LOG}"
"${HELPER}" -- "${T}/docs/note.txt" "${T}/docs/link.pdf" && fail "a text file and a link were signed"
grep -q '^sign' "${LOG}" && fail "something was signed that is no plain PDF"
grep -q '^zenity' "${LOG}" && fail "a card was shown for no PDF"

# One certificate for documents (the code one is never offered): the card first, then the signature, then the notice
: >"${LOG}"
rm -f "${CARD}".*
(cd "${T}/docs" && "${HELPER}" -- Lease.pdf) || fail "a PDF was not signed"
same "the order" "$(cut -d' ' -f1 "${LOG}" | tr '\n' ' ')" "zenity sign notify "
grep -q "^sign ${T}/docs/Lease.pdf abc123$" "${LOG}" || fail "the document certificate was not the one used: $(cat "${LOG}")"
[[ -f "${T}/docs/Lease (signed).pdf" ]] || fail "no signed copy"
grep -qi 'unknown' "${CARD}.1" || fail "the card does not say a reader calls the signer unknown"
grep -q 'ame vault root' "${CARD}.1" || fail "the card does not name the root"
grep -q 'never touched' "${CARD}.1" || fail "the card does not say the original is left alone"
grep -q 'Lease.pdf' "${CARD}.1" || fail "the card does not name the file"

# A declined card signs nothing
: >"${LOG}"
ZENITY_ANSWER=1 "${HELPER}" -- "${T}/docs/Two.pdf" && fail "a declined card still signed"
grep -q '^sign' "${LOG}" && fail "signed after a declined card"

# Two certificates for documents: the person picks, and that id is the one signed with
printf 'def456\tWork\tpdf\tCN=Ann\t2030-01-01\n' >>"${CERTS}"
: >"${LOG}"
ZENITY_PICK=def456 "${HELPER}" -- "${T}/docs/Two.pdf" || fail "a picked certificate did not sign"
grep -q "^sign ${T}/docs/Two.pdf def456$" "${LOG}" || fail "the picked certificate was not used"
: >"${LOG}"
ZENITY_PICK='x; rm -rf ~' "${HELPER}" -- "${T}/docs/Two.pdf" && fail "an odd id was accepted"
grep -q '^sign' "${LOG}" && fail "signed with an id that is no id"

# A refusal is said, and no copy is left
printf 'abc123\tMy signature\tpdf\tCN=Ann\t2030-01-01\n' >"${CERTS}"
: >"${LOG}"
SIGN_REFUSED=1 "${HELPER}" -- "${T}/docs/Two.pdf" && fail "a refused signature counted as signed"
grep -q 'Nothing was signed' "${LOG}" || fail "a refusal was not said"

# A closed vault and none at all offer the command, in a terminal, and sign nothing
: >"${LOG}"
VAULT_CODE=2 "${HELPER}" -- "${T}/docs/Two.pdf" && fail "a closed vault signed"
grep -q '^terminal ame vault open$' "${LOG}" || fail "a closed vault did not offer to open it"
: >"${LOG}"
VAULT_CODE=3 "${HELPER}" -- "${T}/docs/Two.pdf" && fail "no vault signed"
grep -q '^terminal ame vault setup$' "${LOG}" || fail "no vault did not offer setting it up"
# Declined, the terminal is never started
: >"${LOG}"
ZENITY_ANSWER=1 VAULT_CODE=2 "${HELPER}" -- "${T}/docs/Two.pdf" || true
grep -q '^terminal' "${LOG}" && fail "a terminal was started after a no"

# No certificate for documents: it says how to make one, and signs nothing
printf 'zzz999\tCode\tcode\tCN=Ann\t2030-01-01\n' >"${CERTS}"
: >"${LOG}"
"${HELPER}" -- "${T}/docs/Two.pdf" && fail "signed with no document certificate"
grep -q '^sign' "${LOG}" && fail "signed with a code certificate"
echo "test-pdf-sign: ok"
