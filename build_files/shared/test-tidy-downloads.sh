#!/usr/bin/bash
# Tidy downloads (amethystora-tidy-downloads) against a stand-in home, for 20-tests.sh: files and installers are moved and put back,
# and a month folder, or the Trash, that a program with Downloads access swapped for a link is never followed, by the run or by Undo.
#   bash test-tidy-downloads.sh [HELPER]

set -uo pipefail

HELPER="${1:-/usr/libexec/amethystora-tidy-downloads}"
T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
export HOME="${T}/home" XDG_DATA_HOME="${T}/home/.local/share" XDG_STATE_HOME="${T}/home/.local/state"
export AMETHYSTORA_TIDY_DOWNLOADS="${HOME}/Downloads" AMETHYSTORA_TIDY_NOTIFY=none
DL="${AMETHYSTORA_TIDY_DOWNLOADS}"
TRASH="${XDG_DATA_HOME}/Trash"
SECRET="${HOME}/.ssh"
MONTH="$(date -d '40 days ago' +%Y-%m)"
mkdir -p "${DL}" "${TRASH}/files" "${TRASH}/info" "${SECRET}"
failed=0
fail() {
    echo "test-tidy-downloads: $*" >&2
    failed=1
}
old() { # NAME DAYS
    printf 'data of %s\n' "$1" >"${DL}/$1"
    touch -d "$2 days ago" "${DL}/$1"
}

# --- The ordinary run and Undo ---------------------------------------------------------------------------------------------
old photo.jpg 40
old setup.exe 20
old fresh.txt 2
"${HELPER}" run || fail "run failed"
[[ -f "${DL}/${MONTH}/photo.jpg" && ! -e "${DL}/photo.jpg" ]] || fail "an old file did not go into its month"
[[ -f "${TRASH}/files/setup.exe" && -f "${TRASH}/info/setup.exe.trashinfo" && ! -e "${DL}/setup.exe" ]] || fail "an old installer did not go to the trash"
[[ -f "${DL}/fresh.txt" ]] || fail "a new file moved"
"${HELPER}" undo >/dev/null || fail "undo failed"
[[ -f "${DL}/photo.jpg" && -f "${DL}/setup.exe" && ! -e "${DL}/${MONTH}" && ! -e "${TRASH}/info/setup.exe.trashinfo" ]] || fail "undo did not put everything back"

# --- A month folder that is a link: not filed into -----------------------------------------------------------------------------
OUT="${T}/elsewhere"
mkdir -p "${OUT}"
ln -s "${OUT}" "${DL}/${MONTH}"
"${HELPER}" run
[[ -f "${DL}/photo.jpg" && -z "$(ls -A "${OUT}")" ]] || fail "a file was filed through a month folder that is a link"
rm "${DL}/${MONTH}"
"${HELPER}" undo >/dev/null

# --- Undo with the month folder swapped for a link to a folder of the account's own: the key stays where it is ---------------------
"${HELPER}" run
[[ -f "${DL}/${MONTH}/photo.jpg" ]] || fail "setup of the swap: the file was not filed"
printf 'private key\n' >"${SECRET}/photo.jpg"
mv "${DL}/${MONTH}" "${DL}/real-month"
ln -s "${SECRET}" "${DL}/${MONTH}"
"${HELPER}" undo >/dev/null
[[ "$(cat "${SECRET}/photo.jpg" 2>/dev/null)" == "private key" ]] || fail "Undo moved a file out of a folder a month folder's link led to"
[[ ! -e "${DL}/photo.jpg" ]] || fail "Undo put a file into Downloads through a link"
rm "${DL}/${MONTH}"
mv "${DL}/real-month" "${DL}/${MONTH}"
rm -f "${SECRET}/photo.jpg"

# --- Undo with the Trash's files folder swapped for a link -----------------------------------------------------------------------
old setup.exe 20
"${HELPER}" run
[[ -f "${TRASH}/files/setup.exe" ]] || fail "setup of the swap: the installer did not go to the trash"
mv "${TRASH}/files" "${TRASH}/real-files"
mkdir "${T}/decoy"
printf 'private key\n' >"${T}/decoy/setup.exe"
ln -s "${T}/decoy" "${TRASH}/files"
"${HELPER}" undo >/dev/null
[[ "$(cat "${T}/decoy/setup.exe" 2>/dev/null)" == "private key" && ! -e "${DL}/setup.exe" ]] || fail "Undo followed a Trash folder that is a link"
rm "${TRASH}/files"
mv "${TRASH}/real-files" "${TRASH}/files"

# --- The state file names another folder: refused -------------------------------------------------------------------------------------
printf 'x\n' >"${SECRET}/id_ed25519"
mkdir -p "${XDG_STATE_HOME}/amethystora"
printf '{"version":1,"at":0,"made":[],"moves":[{"from":"%s/id_ed25519.back","to":"%s/id_ed25519"}]}' "${DL}" "${SECRET}" >"${XDG_STATE_HOME}/amethystora/tidy-downloads.json"
rm -f "${XDG_STATE_HOME}/amethystora/tidy-downloads.json.bak"
"${HELPER}" undo >/dev/null
[[ -f "${SECRET}/id_ed25519" && ! -e "${DL}/id_ed25519.back" ]] || fail "Undo took a file from a folder that is neither a month folder nor the trash"

if [[ "${failed}" != 0 ]]; then
    exit 1
fi
echo "test-tidy-downloads: files are moved and put back, and no link is followed"
