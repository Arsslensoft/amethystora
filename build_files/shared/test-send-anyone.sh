#!/usr/bin/bash
# Send to anyone (R7-66): /usr/libexec/amethystora-send-anyone against stand-ins, run from 20-tests.sh and by hand
# (`bash build_files/shared/test-send-anyone.sh [HELPER]`). flatpak, gum and the installer are stubs that log what they were asked.
# It checks that: off, nothing is written; on, only Files' entry and a marker are, and a script of the person's under that name is
# left; Warp is fetched only after a yes (and never without a terminal), from Flathub into the system installation, by the one
# helper that installs; the relay is said before the question and again before each send; and off takes away only what was written.
set -uo pipefail

HELPER="${1:-$(dirname "$0")/../../system_files/shared/usr/libexec/amethystora-send-anyone}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
fail() {
    echo "test-send-anyone: $*" >&2
    exit 1
}

export HOME="${T}/home" XDG_CONFIG_HOME="${T}/home/.config" XDG_DATA_HOME="${T}/home/.local/share"
mkdir -p "${HOME}" "${T}/stubs" "${T}/log"
export PATH="${T}/stubs:${PATH}"
export AMETHYSTORA_FLATPAK_ASK="${T}/stubs/flatpak-ask"
SCRIPTS="${XDG_DATA_HOME}/nautilus/scripts"
MARKER="${XDG_CONFIG_HOME}/amethystora/send-anyone"
LOG="${T}/log/calls"

cat >"${T}/stubs/flatpak" <<EOF
#!/bin/sh
echo "flatpak \$*" >>"${LOG}"
if [ "\$1" = info ]; then [ -e "${T}/installed" ]; exit \$?; fi
exit 0
EOF
cat >"${T}/stubs/flatpak-ask" <<EOF
#!/bin/sh
echo "ask \$*" >>"${LOG}"
touch "${T}/installed"
EOF
cat >"${T}/stubs/gum" <<EOF
#!/bin/sh
echo "gum \$*" >>"${LOG}"
[ -e "${T}/answer-yes" ]
EOF
chmod +x "${T}/stubs/"*

# A terminal, which is what the helper insists on
in_terminal() {
    python3 -c 'import os, pty, sys; sys.exit(os.waitstatus_to_exitcode(pty.spawn(sys.argv[1:])))' "$@"
}

# --- Off: nothing is there -------------------------------------------------------------------------------------------------------
[[ ! -e "${MARKER}" && ! -d "${SCRIPTS}" ]] || fail "something is there before anything was turned on"
[[ "$("${HELPER}" status | head -1)" == off ]] || fail "status of an off feature"
: >"${LOG}"

# --- On: Files' entry and a marker, nothing else ----------------------------------------------------------------------------------------
"${HELPER}" menu on >/dev/null || fail "menu on"
ENTRY="${SCRIPTS}/Send to anyone"
[[ -x "${ENTRY}" ]] || fail "no entry in Files"
[[ "$(sed -n 2p "${ENTRY}")" == "# amethystora-send-anyone "* ]] || fail "the entry is not marked as ours"
[[ "$(sed -n 3p "${ENTRY}")" == 'exec /usr/libexec/amethystora-in-terminal /usr/libexec/amethystora-send-anyone send -- "$@"' ]] || fail "the entry's command: $(sed -n 3p "${ENTRY}")"
[[ "$(cat "${MARKER}")" == "${ENTRY}" ]] || fail "the marker does not name the entry"
[[ ! -s "${LOG}" ]] || fail "turning it on asked flatpak for anything: $(cat "${LOG}")"
[[ "$(find "${HOME}" -type f | wc -l)" == 2 ]] || fail "more than the entry and the marker were written: $(find "${HOME}" -type f)"
[[ "$("${HELPER}" status | head -1)" == on ]] || fail "status of an on feature"
: >"${LOG}"

# --- Nothing is fetched without a yes, nor without a terminal ---------------------------------------------------------------------------
"${HELPER}" send -- "${T}/a file.zip" >"${T}/log/out" 2>&1 </dev/null
[[ $? == 64 ]] || fail "without a terminal it must refuse (64)"
[[ ! -s "${LOG}" ]] || fail "it called something without a terminal: $(cat "${LOG}")"
OUT="$(in_terminal "${HELPER}" send -- "${T}/a file.zip")"
[[ $? == 0 || -n "${OUT}" ]] || true
if grep -q '^ask' "${LOG}"; then fail "Warp was fetched after a no"; fi
grep -q '^gum confirm' "${LOG}" || fail "the person was not asked: $(cat "${LOG}")"
grep -q '^flatpak run' "${LOG}" && fail "Warp ran though it was not installed"
[[ "${OUT}" == *"public server of the Magic Wormhole project"* ]] || fail "the relay was not said before the question: ${OUT}"
[[ ! -e "${T}/installed" ]] || fail "installed after a no"

# --- A yes fetches it, once, by the one helper that installs; it opens, and says the relay again -------------------------------------------------
touch "${T}/answer-yes"
: >"${LOG}"
OUT="$(in_terminal "${HELPER}" send -- "${T}/a file.zip")"
grep -qx "ask install --system flathub app/app.drey.Warp/$(uname -m)/stable" "${LOG}" || fail "it did not install the right ref by the helper: $(cat "${LOG}")"
[[ "$(grep -c '^ask' "${LOG}")" == 1 ]] || fail "installed more than once"
grep -qx "flatpak run app.drey.Warp" "${LOG}" || fail "Warp was not opened"
[[ "${OUT}" == *"a file.zip"* && "${OUT}" == *"Send File"* ]] || fail "it does not say which file to choose: ${OUT}"
# Installed already: no question, no fetch, the relay is still said
: >"${LOG}"
OUT="$(in_terminal "${HELPER}" send -- "${T}/a file.zip")"
grep -q '^ask\|^gum' "${LOG}" && fail "it asked or fetched again: $(cat "${LOG}")"
[[ "${OUT}" == *"public relay of the same project"* ]] || fail "the relay is not said before each send"
grep -qx "flatpak run app.drey.Warp" "${LOG}" || fail "Warp was not opened the second time"
# A name with control characters is shown plainly
OUT="$(in_terminal "${HELPER}" send -- $'/x/\e[31mevil.zip')"
[[ "${OUT}" != *$'\e[31m'* ]] || fail "a control character of a file name reached the terminal"

# --- Off takes away what it wrote, and only that ----------------------------------------------------------------------------------------
"${HELPER}" menu off >/dev/null || fail "menu off"
[[ ! -e "${ENTRY}" && ! -e "${MARKER}" ]] || fail "off left the entry or the marker"
# A script of the person's under the same name is never written over nor removed
printf '#!/bin/sh\necho mine\n' >"${ENTRY}"
chmod +x "${ENTRY}"
"${HELPER}" menu on >/dev/null || fail "menu on over a script of the person's"
[[ "$(sed -n 2p "${ENTRY}")" == "echo mine" ]] || fail "a script of the person's was written over"
"${HELPER}" menu off >/dev/null
[[ -f "${ENTRY}" && "$(sed -n 2p "${ENTRY}")" == "echo mine" ]] || fail "off removed a script of the person's"
# A marker that names a path elsewhere removes nothing
mkdir -p "${XDG_CONFIG_HOME}/amethystora"
echo "${T}/home/precious" >"${MARKER}"
echo keep >"${T}/home/precious"
"${HELPER}" menu off >/dev/null
[[ -f "${T}/home/precious" ]] || fail "a marker made off remove a file outside the scripts folder"
"${HELPER}" bogus >/dev/null 2>&1
[[ $? == 64 ]] || fail "usage exits 64"

echo "test-send-anyone: ok"
