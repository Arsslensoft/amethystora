#!/usr/bin/bash
# What changed (R7-32) with the helper it hands an app's Undo to, as a pair (R7-41): /usr/libexec/amethystora-changes against the
# real /usr/libexec/amethystora-uninstall, with the program that asks Flatpak (AMETHYSTORA_FLATPAK_ASK) and flatpak itself stood in.
# test-changes.sh stands in for the whole of the second helper, so only this has ever run the two together: it holds that what the
# one lists is what the other brings back, with the folder out of the trash, once, and that a folder made since stays in the trash.
#   bash test-changes-uninstall.sh [CHANGES-HELPER [UNINSTALL-HELPER]]

set -uo pipefail

HELPER="${1:-/usr/libexec/amethystora-changes}"
UNINSTALL="${2:-${AMETHYSTORA_UNINSTALL:-/usr/libexec/amethystora-uninstall}}"
ROOT="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${ROOT}"' EXIT
export AMETHYSTORA_CHANGES_ROOT="${ROOT}"
export HOME="${ROOT}/var/home/me"
export XDG_CONFIG_HOME="${HOME}/.config" XDG_DATA_HOME="${HOME}/.local/share" XDG_STATE_HOME="${HOME}/.local/state"
export LOG="${ROOT}/log" INSTALLED="${ROOT}/installed"
TRASH="${XDG_DATA_HOME}/Trash"
NOW="$(date +%s)"
mkdir -p "${HOME}" "${TRASH}/info" "${TRASH}/files" "${ROOT}/bin" "${ROOT}/usr/libexec" "${XDG_CONFIG_HOME}" "${XDG_STATE_HOME}"
: >"${LOG}"
: >"${INSTALLED}"
failed=0
fail() {
    echo "test-changes-uninstall: $*" >&2
    failed=1
}
[[ -s "${UNINSTALL}" ]] || { echo "test-changes-uninstall: no ${UNINSTALL} to run the pair with" >&2; exit 1; }
python3 -c 'import gi' 2>/dev/null || { echo "test-changes-uninstall: python3 cannot import gi, which the uninstall helper needs" >&2; exit 1; }
cp "${UNINSTALL}" "${ROOT}/usr/libexec/amethystora-uninstall"

# flatpak: nothing is installed until the stand-in for the program that asks Flatpak has installed it
cat >"${ROOT}/bin/flatpak" <<'EOF'
#!/usr/bin/bash
case "$1" in
    info) grep -q "^$4\$" "${INSTALLED}" 2>/dev/null && echo "app/$4/x86_64/stable" || exit 1 ;;
    list) cat "${INSTALLED}" ;;
    history) printf 'deploy install\tapp/org.example.Pair/x86_64/stable\tsystem\tflathub\n' ;;
esac
EOF
cat >"${ROOT}/bin/flatpak-ask" <<'EOF'
#!/usr/bin/bash
echo "ask $*" >>"${LOG}"
[[ "$1" == install ]] && echo "$4" | sed 's#^app/##; s#/.*##' >>"${INSTALLED}"
exit 0
EOF
chmod +x "${ROOT}"/bin/*
export AMETHYSTORA_FLATPAK_ASK="${ROOT}/bin/flatpak-ask"
export PATH="${ROOT}/bin:${PATH}"

# An app's folder, in the trash as the uninstall leaves it
trash_app() {
    mkdir -p "${TRASH}/files/$1"
    printf 'saved\n' >"${TRASH}/files/$1/settings"
    printf '[Trash Info]\nPath=%s\nDeletionDate=%s\n' "${HOME}/.var/app/$1" "$(date '+%Y-%m-%dT%H:%M:%S')" >"${TRASH}/info/$1.trashinfo"
}
trash_app org.example.Pair
trash_app org.example.Newer

list() { "${HELPER}" list --json; }
undo() { "${HELPER}" undo "$1" --json; }

LIST="$(list)"
jq -e '.items | any(.id == "app:org.example.Pair" and (.what | startswith("You removed")) and .undo.terminal == true)' <<<"${LIST}" >/dev/null || fail "an app removed with its folder is not listed: ${LIST}"
jq -e '.items | all(.id | startswith("trash:org.example") | not)' <<<"${LIST}" >/dev/null || fail "an app's folder in the trash is listed as a file: ${LIST}"

undo "app:org.example.Pair" | jq -e '.ok == true and (.words | contains("is back"))' >/dev/null || fail "the app did not come back"
grep -qxF "ask install --system flathub app/org.example.Pair/x86_64/stable" "${LOG}" || fail "it was not installed again where it came from: $(cat "${LOG}")"
[[ "$(cat "${HOME}/.var/app/org.example.Pair/settings" 2>/dev/null)" == saved ]] || fail "what the app had saved is not back"
[[ ! -e "${TRASH}/files/org.example.Pair" && ! -e "${TRASH}/info/org.example.Pair.trashinfo" ]] || fail "the trash still holds the folder"
list | jq -e '.items | all(.id != "app:org.example.Pair")' >/dev/null || fail "an app that is back is still listed as removed"
[[ "$(undo "app:org.example.Pair" | jq -r .ok)" == false ]] || fail "Undo of an app that is back was accepted again"

# A folder the app made since: what was removed stays in the trash, and is said so
mkdir -p "${HOME}/.var/app/org.example.Newer"
undo "app:org.example.Newer" | jq -e '.ok == true and (.words | contains("stays in the trash"))' >/dev/null || fail "a folder made since was not said to leave the old one in the trash"
[[ -e "${TRASH}/files/org.example.Newer/settings" && ! -e "${HOME}/.var/app/org.example.Newer/settings" ]] || fail "the older folder was moved over a newer one"

if [[ "${failed}" != 0 ]]; then
    exit 1
fi
echo "test-changes-uninstall: the pair brings an app back with its folder, once"
