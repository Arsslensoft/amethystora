#!/usr/bin/bash
# Round 7's files group against stand-ins, for 20-tests.sh (plans/ui-ideas/r7/spec-files.md): what amethystora-open-anything finds
# in a catalogue and what its switch writes, what amethystora-free-space counts and clears and what it never touches, what
# amethystora-uninstall moves and brings back, and Earlier versions' entry and arguments. Each in a home of its own, with
# stand-ins for flatpak and the others: nothing here reaches a network or the machine's own apps.
#
#   test-files-group.sh [LIBEXEC]   LIBEXEC: where the helpers are (/usr/libexec)

set -euo pipefail

LIBEXEC="${1:-/usr/libexec}"
T="$(realpath "$(mktemp -d)")"
trap 'chmod -R u+w "${T}" 2>/dev/null; rm -rf "${T}"' EXIT
fail() { echo "test-files-group: $*" >&2; exit 1; }
same() { [[ "$2" == "$3" ]] || fail "$1: expected '$3', found '$2'"; }

export HOME="${T}/home"
export XDG_DATA_HOME="${HOME}/.local/share"
unset XDG_CONFIG_HOME XDG_STATE_HOME
mkdir -p "${HOME}/.config/glib-2.0/settings" "${T}/bin" "${XDG_DATA_HOME}"
export PATH="${T}/bin:${PATH}"
# The account's own settings, in a file a second process reads too
export GSETTINGS_BACKEND=keyfile
housekeeping() { # ON DAYS
    printf '[org/gnome/desktop/privacy]\nremove-old-trash-files=%s\nold-files-age=uint32 %s\n' "$1" "$2" >"${HOME}/.config/glib-2.0/settings/keyfile"
}
housekeeping true 30

# flatpak as a log and a few answers
export FLATPAK_LOG="${T}/flatpak.log"
: >"${FLATPAK_LOG}"
cat >"${T}/bin/flatpak" <<'EOF'
#!/usr/bin/bash
echo "flatpak $*" >>"${FLATPAK_LOG}"
case "$1 ${2:-}" in
    "remote-ls --system") printf 'org.example.Drawing\t60.0 MB\norg.example.Everything\t1.1 GB\norg.example.Second\t2 MB\norg.example.Third\t3 MB\norg.example.Fourth\t4 MB\n' ;;
    "ps "*) echo org.example.Running ;;
    "info --user") exit 1 ;;
    "info --system") [[ "${!#}" == org.example.Installed ]] || exit 1; echo "app/${!#}/x86_64/stable" ;;
    "history "*) cat "${FLATPAK_HISTORY:-/dev/null}" ;;
    "list --app") printf 'org.example.Old\tOld\tsystem\norg.example.Fresh\tFresh\tsystem\norg.example.NoLauncher\tNo launcher\tsystem\n' ;;
esac
exit 0
EOF
chmod +x "${T}/bin/flatpak"
# What installs and removes, which lets polkit ask, as a log too. Its shapes are its own check, below
export AMETHYSTORA_FLATPAK_ASK="${T}/bin/flatpak-ask"
cat >"${T}/bin/flatpak-ask" <<'EOF'
#!/usr/bin/bash
echo "ask $*" >>"${FLATPAK_LOG}"
[[ -z "${FLATPAK_FAILS:-}" ]]
EOF
chmod +x "${T}/bin/flatpak-ask"
ASK_REAL="${LIBEXEC}/amethystora-flatpak-ask"
[[ "$("${ASK_REAL}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "flatpak-ask with no arguments is a usage error"
for bad in "install --system --x app/org.example.A/x86_64/stable" "install --system flathub --x" "install --force flathub app/org.example.A/x86_64/stable" \
    "uninstall --system org.example.A" "uninstall --system app/../x/y" "uninstall-unused --system more" "remove --system app/org.example.A/x86_64/stable"; do
    # shellcheck disable=SC2086
    [[ "$("${ASK_REAL}" ${bad} >/dev/null 2>&1; echo $?)" == 64 ]] || fail "flatpak-ask took: ${bad}"
done

# --- Anything opens --------------------------------------------------------------------------------------------------
OPEN="${LIBEXEC}/amethystora-open-anything"
export AMETHYSTORA_CATALOGUE="${T}/appstream.xml"
cat >"${AMETHYSTORA_CATALOGUE}" <<'EOF'
<components>
  <component type="desktop-application"><id>org.example.Everything</id><name>Everything</name><summary>Opens it all</summary>
    <bundle type="flatpak">app/org.example.Everything/x86_64/stable</bundle>
    <provides><mediatype>application/x-sqlite3</mediatype><mediatype>a/b</mediatype><mediatype>a/c</mediatype></provides></component>
  <component type="desktop-application"><id>org.example.Drawing</id><name xml:lang="ar">رسم</name><name>Drawing</name>
    <summary xml:lang="ar">وصف</summary><summary>Draws</summary><bundle type="flatpak">app/org.example.Drawing/x86_64/stable</bundle>
    <custom><value key="flathub::verification::verified">true</value></custom>
    <provides><mediatype>application/x-sqlite3</mediatype></provides></component>
  <component type="desktop-application"><id>org.example.Stale</id><name>Stale</name><summary>The remote has dropped it</summary>
    <bundle type="flatpak">app/org.example.Stale/x86_64/stable</bundle><provides><mediatype>application/x-sqlite3</mediatype></provides></component>
  <component type="desktop-application"><id>evil</id><name>No bundle at all</name>
    <provides><mediatype>application/x-sqlite3</mediatype></provides></component>
  <component type="runtime"><id>org.example.Platform</id><name>A runtime</name>
    <bundle type="flatpak">runtime/org.example.Platform/x86_64/1</bundle><provides><mediatype>application/x-sqlite3</mediatype></provides></component>
</components>
EOF
FILE="${T}/secret canary name.sqlite"
printf 'SQLite format 3\000' >"${FILE}"
head -c 2000 /dev/zero >>"${FILE}"

[[ "$("${OPEN}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "no arguments is a usage error"
answer="$(LANGUAGE=en "${OPEN}" lookup --json -- "${FILE}")"
# By the name shared-mime-info gives it, whichever: the list names it by its other one, which has to find it all the same
[[ "$(jq -r .type <<<"${answer}")" == application/*sqlite3 ]] || fail "the type: $(jq -r .type <<<"${answer}")"
same "its words" "$(jq -r '.description | length > 0' <<<"${answer}")" true
same "the file's name is not in the answer" "$(grep -c canary <<<"${answer}" || true)" 0
same "the checked and narrow app first, no stale or bundle-less one, no runtime" "$(jq -r '[.apps[].id] | join(" ")' <<<"${answer}")" \
    "org.example.Drawing org.example.Everything"
same "the size from Flatpak's local list" "$(jq -r '.apps[0].size' <<<"${answer}")" "60.0 MB"
same "untagged words without that language" "$(jq -r '.apps[0].name' <<<"${answer}")" Drawing
same "the session's language" "$(LANGUAGE=ar "${OPEN}" lookup --json -- "${FILE}" | jq -r '.apps[0].name')" "رسم"
grep -q -- '--cached' "${FLATPAK_LOG}" || fail "the sizes asked the network"
grep -q 'flatpak install' "${FLATPAK_LOG}" && fail "a lookup installed something"

# What it says with no list, and of a file that says nothing
same "no catalogue yet" "$(AMETHYSTORA_CATALOGUE="${T}/none" "${OPEN}" lookup --json -- "${FILE}" | jq -c '[.catalogue, .apps]')" '[false,[]]'
# The same bytes every time: of random ones, about 1 in 170 starts with a magic number (a '%' is MATLAB's)
python3 -I -c 'import random, sys; r = random.Random(1); sys.stdout.buffer.write(bytes(r.randrange(256) for _ in range(3000)))' >"${T}/noise.qqqq"
same "a file that says nothing" "$("${OPEN}" lookup --json -- "${T}/noise.qqqq" | jq -r .unknown)" true

# Only a ref of its own shape is ever passed on; the file goes as one argument, between Flatpak's own markers
for ref in "--user" "app/../../x" "runtime/org.example.Platform/x86_64/1" "app/org.example.Drawing/x86_64/stable --foo"; do
    [[ "$("${OPEN}" install-open "${ref}" -- "${FILE}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "took the ref ${ref}"
done
: >"${FLATPAK_LOG}"
"${OPEN}" install-open app/org.example.Drawing/x86_64/stable -- "${FILE}"
grep -qx 'ask install --system flathub app/org.example.Drawing/x86_64/stable' "${FLATPAK_LOG}" || fail "install-open did not install"
for _ in 1 2 3 4 5 6 7 8 9 10; do grep -q '^flatpak run' "${FLATPAK_LOG}" && break; sleep 0.2; done
grep -qxF "flatpak run --file-forwarding org.example.Drawing @@ ${FILE} @@" "${FLATPAK_LOG}" || fail "the app was not given the file"
: >"${FLATPAK_LOG}"
[[ "$(FLATPAK_FAILS=1 "${OPEN}" install-open app/org.example.Drawing/x86_64/stable -- "${FILE}" >/dev/null 2>&1; echo $?)" == 1 ]] || fail "a failed install said it worked"
sleep 0.3
grep -q '^flatpak run' "${FLATPAK_LOG}" && fail "a failed install opened the file"

# Its switch: exactly the entry the watcher takes as starting nothing, and nobody else's launcher
SHADOW="${XDG_DATA_HOME}/applications/zz-amethystora-open-anything.desktop"
"${OPEN}" switch off
same "off writes exactly the hidden entry" "$(cat "${SHADOW}")" "$(printf '[Desktop Entry]\nHidden=true')"
"${OPEN}" switch off
"${OPEN}" switch on
[[ ! -e "${SHADOW}" ]] || fail "on left the hidden entry"
"${OPEN}" switch on
# The desktop and `ame desktop open-anything` at once: neither fails, whichever comes first, and no temporary file stays
for _ in 1 2 3 4 5 6 7 8; do
    for state in off off on on off on; do "${OPEN}" switch "${state}" 2>>"${T}/races" & done
    for job in $(jobs -p); do wait "${job}" || fail "a switch at the same moment as another failed: $(cat "${T}/races")"; done
done
[[ ! -s "${T}/races" ]] || fail "a switch said something: $(cat "${T}/races")"
same "no temporary file stays" "$(find "${XDG_DATA_HOME}/applications" -type f ! -name 'zz-amethystora-open-anything.desktop' | wc -l)" 0
"${OPEN}" switch on
printf '[Desktop Entry]\nType=Application\nName=Mine\nExec=mine\n' >"${SHADOW}"
[[ "$("${OPEN}" switch off >/dev/null 2>&1; echo $?)" == 1 ]] || fail "a launcher of the person's own was taken over"
grep -q 'Name=Mine' "${SHADOW}" || fail "a launcher of the person's own changed"
[[ "$("${OPEN}" switch on >/dev/null 2>&1; echo $?)" == 1 ]] || fail "a launcher of the person's own was removed"
rm -f "${SHADOW}"

# An app added after the build, in the launcher's own folder, takes its kinds from it: update-desktop-database sorts each kind's list
# by name, and GIO takes the first. Only the launcher's real name holds this, so it is read from the helper.
LAUNCHER="$(sed -n 's/^LAUNCHER = "\(.*\)"$/\1/p' "${OPEN}")"
APPS="${T}/data/applications"
mkdir -p "${APPS}"
for launcher in "${LAUNCHER}" org.example.Later.desktop z.example.Later.desktop; do
    printf '[Desktop Entry]\nType=Application\nName=%s\nExec=/usr/bin/true %%f\nMimeType=application/x-amethystora-test;\n' "${launcher}" >"${APPS}/${launcher}"
done
update-desktop-database "${APPS}"
default="$(XDG_DATA_DIRS="${T}/data" XDG_DATA_HOME="${T}/none" XDG_CONFIG_HOME="${T}/none" XDG_CONFIG_DIRS="${T}/none" XDG_CURRENT_DESKTOP=GNOME \
    gio mime application/x-amethystora-test | sed -n '1s/.*: //p')"
same "an app added later comes before the launcher" "${default}" org.example.Later.desktop
[[ "$(tr ';' '\n' <<<"$(sed -n 's/^application.x-amethystora-test=//p' "${APPS}/mimeinfo.cache")" | tail -n2 | head -n1)" == "${LAUNCHER}" ]] ||
    fail "the launcher's name does not sort last: $(cat "${APPS}/mimeinfo.cache")"

# --- Never out of room ------------------------------------------------------------------------------------------------
ROOM="${LIBEXEC}/amethystora-free-space"
TRASH="${XDG_DATA_HOME}/Trash"
mkdir -p "${TRASH}/files" "${TRASH}/info" "${HOME}/.var/app/org.example.Idle/cache/x" "${HOME}/.var/app/org.example.Idle/data" \
    "${HOME}/.var/app/org.example.Running/cache" "${HOME}/.cache/thumbnails/large" "${HOME}/Documents"
write() { head -c "$2" /dev/zero >"$1"; }
write "${HOME}/.var/app/org.example.Idle/cache/x/blob" 1048576
write "${HOME}/.var/app/org.example.Idle/data/work" 1048576
write "${HOME}/.var/app/org.example.Running/cache/in-use" 1048576
write "${HOME}/.cache/thumbnails/large/t.png" 1048576
write "${HOME}/Documents/report.odt" 1048576
ln -s "${HOME}/Documents" "${HOME}/.cache/thumbnails/a-link"
trashed() { # NAME DAYS-AGO
    write "${TRASH}/files/$1" 1048576
    printf '[Trash Info]\nPath=/somewhere/%s\nDeletionDate=%s\n' "$1" "$(date -d "$2 days ago" +%Y-%m-%dT%H:%M:%S)" >"${TRASH}/info/$1.trashinfo"
}
trashed long-gone 40
trashed yesterday 1
write "${TRASH}/files/no-date" 1048576
printf '[Trash Info]\nPath=/somewhere/no-date\n' >"${TRASH}/info/no-date.trashinfo"
export AMETHYSTORA_HOMES="${T}/homes"
mkdir -p "${AMETHYSTORA_HOMES}/home"
export AMETHYSTORA_UNUSED_RUNTIMES="${T}/unused.json"
echo '[{"installation": "system", "size": 5000000000}, {"installation": "user", "size": 1000000}]' >"${AMETHYSTORA_UNUSED_RUNTIMES}"
# A file named exactly .trashinfo, which any program of the account can leave there: dated long ago, it must not name all of Trash/files
printf '[Trash Info]\nPath=/somewhere/x\nDeletionDate=2020-01-01T00:00:00\n' >"${TRASH}/info/.trashinfo"
# ...and "..trashinfo" and "...trashinfo", whose stems are "." and ".." (Trash/files itself, and the Trash folder)
printf '[Trash Info]\nPath=/somewhere/x\nDeletionDate=2020-01-01T00:00:00\n' >"${TRASH}/info/..trashinfo"
printf '[Trash Info]\nPath=/somewhere/x\nDeletionDate=2020-01-01T00:00:00\n' >"${TRASH}/info/...trashinfo"
# Homebrew and podman, as the helper asks them
export AMETHYSTORA_BREW="${T}/bin/brew"
cat >"${T}/bin/brew" <<'EOF'
#!/usr/bin/bash
echo "brew $*" >>"${FLATPAK_LOG}"
EOF
mkdir -p "${T}/Cellar/footool/1.0" "${T}/Cellar/footool/2.0" "${T}/opt"
write "${T}/Cellar/footool/1.0/old" 4194304
write "${T}/Cellar/footool/2.0/kept" 104857600
ln -s ../Cellar/footool/2.0 "${T}/opt/footool"
cat >"${T}/bin/podman" <<'EOF'
#!/usr/bin/bash
echo "podman $*" >>"${FLATPAK_LOG}"
[[ "$1 $2" != "image ls" ]] || echo '[{"Id": "abc", "Size": 7000000}]'
EOF
chmod +x "${T}/bin/brew" "${T}/bin/podman"
mkdir -p "${XDG_DATA_HOME}/containers/storage"
: >"${FLATPAK_LOG}"

[[ "$("${ROOM}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "free-space with no arguments is a usage error"
# Nothing is low: only what is on a low disk counts, and here that is nothing
found="$(AMETHYSTORA_LOW_SPACE="0 0" "${ROOM}" check --json)"
same "not low" "$(jq -c '[.low, .total]' <<<"${found}")" '[false,0]'
# As GNOME's thresholds would have it on a full disk
found="$(AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" check --json)"
same "low" "$(jq -r .low <<<"${found}")" true
same "the runtimes, Homebrew's old version and unnamed layers are counted, not the version in use" "$(jq -r '.apps >= 5011000000 and .apps < 5100000000' <<<"${found}")" true
# A check is nobody's click: Homebrew's own dry run fetches its list of formulae
grep -q '^brew' "${FLATPAK_LOG}" && fail "a check ran Homebrew"
same "only the old trash counts" "$(jq -r '.trash >= 1048576 and .trash < 2097152' <<<"${found}")" true
same "the trash's last days and an undated entry are said, not counted" "$(jq -r '.trash_recent >= 2097152' <<<"${found}")" true
same "the answer holds totals and no name" "$(grep -cE 'long-gone|yesterday|report|example' <<<"${found}" || true)" 0
same "--all counts on any disk" "$(AMETHYSTORA_LOW_SPACE="0 0" "${ROOM}" check --all --json | jq -r '.total >= 5001000000')" true

# The trash obeys the account's own housekeeping: a longer time, and off
housekeeping true 90
found="$(AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" check --json)"
same "with 90 days chosen, what is 40 days old does not count" "$(jq -c '[.trash, .trash_days]' <<<"${found}")" '[0,90]'
AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" clear --json >/dev/null
[[ -e "${TRASH}/files/long-gone" ]] || fail "a clear took what the account's 90 days keep"
housekeeping false 30
found="$(AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" check --json)"
same "with housekeeping off, no trash counts" "$(jq -c '[.trash, .trash_days]' <<<"${found}")" '[0,null]'
AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" clear --json >/dev/null
[[ -e "${TRASH}/files/long-gone" ]] || fail "a clear took the trash with housekeeping off"
housekeeping true 7
same "never under 30 days, whatever is chosen" "$(AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" check --json | jq -r .trash_days)" 30
housekeeping true 30
mkdir -p "${HOME}/.var/app/org.example.Idle/cache/x"
write "${HOME}/.var/app/org.example.Idle/cache/x/blob" 1048576
: >"${FLATPAK_LOG}"

# Two disks: only what lies on the low one counts, and only that is cleared. "same" stands in for one disk of two subvolumes,
# whose free space is said once however many places are on it.
disks() { printf '{"home": {"id": "%s", "free": %s, "size": 1000000000000}, "system": {"id": "%s", "free": %s, "size": 1000000000000}}' "$@"; }
HOME_LOW="$(disks a 1000 b 900000000000)"
SYSTEM_LOW="$(disks a 900000000000 b 1000)"
found="$(AMETHYSTORA_LOW_SPACE="0.05 2" AMETHYSTORA_DISKS="${HOME_LOW}" "${ROOM}" check --json)"
same "only the home is low: the machine's runtimes do not count" "$(jq -r '.apps < 5000000000 and .trash >= 1048576' <<<"${found}")" true
found="$(AMETHYSTORA_LOW_SPACE="0.05 2" AMETHYSTORA_DISKS="${SYSTEM_LOW}" "${ROOM}" check --json)"
same "only the system is low: the home's part does not count" "$(jq -c '[.apps, .trash, .home_part]' <<<"${found}")" '[5000000000,0,0]'
AMETHYSTORA_LOW_SPACE="0.05 2" AMETHYSTORA_DISKS="${SYSTEM_LOW}" "${ROOM}" clear --json >/dev/null
grep -qx 'ask uninstall-unused --system' "${FLATPAK_LOG}" || fail "the low disk's runtimes stayed"
grep -qE 'uninstall-unused --user|^podman image prune|^brew cleanup$' "${FLATPAK_LOG}" && fail "a clear took from the disk that is not low"
[[ -e "${HOME}/.var/app/org.example.Idle/cache/x/blob" && -e "${TRASH}/files/long-gone" ]] || fail "a clear took the home's part with only the system low"
: >"${FLATPAK_LOG}"
AMETHYSTORA_LOW_SPACE="0.05 2" AMETHYSTORA_DISKS="${HOME_LOW}" "${ROOM}" clear --json >/dev/null
grep -q 'uninstall-unused --system' "${FLATPAK_LOG}" && fail "a clear took the machine's runtimes with only the home low"
grep -qx 'ask uninstall-unused --user' "${FLATPAK_LOG}" || fail "the account's runtimes stayed"
trashed long-gone 40
mkdir -p "${HOME}/.var/app/org.example.Idle/cache/x"
write "${HOME}/.var/app/org.example.Idle/cache/x/blob" 1048576
# A second account's apps are not on this account's list of what is unused: the machine's runtimes are not one click's then
mkdir "${AMETHYSTORA_HOMES}/somebody"
same "with a second account, the machine's runtimes do not count" "$(AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" check --json | jq -r '.apps < 5000000000')" true
: >"${FLATPAK_LOG}"
AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" clear --json >/dev/null
grep -q 'uninstall-unused --system' "${FLATPAK_LOG}" && fail "a clear took the machine's runtimes from under another account"
rmdir "${AMETHYSTORA_HOMES}/somebody"
trashed long-gone 40
mkdir -p "${HOME}/.var/app/org.example.Idle/cache/x"
write "${HOME}/.var/app/org.example.Idle/cache/x/blob" 1048576
same "one disk under two names says its free space once" \
    "$(AMETHYSTORA_LOW_SPACE="1 99999999" AMETHYSTORA_DISKS="$(disks same 1000 same 1000)" "${ROOM}" clear --json | jq -r '.free < 2000 + 10000000000')" true
trashed long-gone 40
mkdir -p "${HOME}/.var/app/org.example.Idle/cache/x"
write "${HOME}/.var/app/org.example.Idle/cache/x/blob" 1048576
: >"${FLATPAK_LOG}"

AMETHYSTORA_LOW_SPACE="1 99999999" "${ROOM}" clear --json >"${T}/cleared.json"
jq -e '.version == 1 and (.cleared | type) == "number"' "${T}/cleared.json" >/dev/null || fail "clear said nothing"
grep -qx 'ask uninstall-unused --system' "${FLATPAK_LOG}" || fail "the unused runtimes stayed"
grep -qx 'podman image prune -f' "${FLATPAK_LOG}" || fail "the unnamed layers stayed"
grep -qx 'brew cleanup' "${FLATPAK_LOG}" || fail "Homebrew's old versions stayed"
grep -qE '^podman .*(-a|volume)' "${FLATPAK_LOG}" && fail "a clear went beyond unnamed layers"
grep -E 'flatpak uninstall' "${FLATPAK_LOG}" | grep -q -- '--delete-data' && fail "clear deleted an app's data"
[[ -d "${TRASH}/files" && -e "${TRASH}/info/.trashinfo" ]] || fail "a file named exactly .trashinfo took the whole trash, or went itself"
[[ -e "${TRASH}/info/..trashinfo" && -e "${TRASH}/info/...trashinfo" && -d "${TRASH}/info" ]] || fail "an info file named for . or .. took the trash folder"
[[ ! -e "${TRASH}/files/long-gone" && ! -e "${TRASH}/info/long-gone.trashinfo" ]] || fail "the old trash stayed"
[[ -e "${TRASH}/files/yesterday" && -e "${TRASH}/info/yesterday.trashinfo" ]] || fail "the trash's last days went"
[[ -e "${TRASH}/files/no-date" ]] || fail "a trash entry with no date went"
[[ ! -e "${HOME}/.var/app/org.example.Idle/cache/x" ]] || fail "an idle app's cache stayed"
[[ -d "${HOME}/.var/app/org.example.Idle/cache" ]] || fail "the cache folder itself went"
[[ -e "${HOME}/.var/app/org.example.Idle/data/work" ]] || fail "an app's data went"
[[ -e "${HOME}/.var/app/org.example.Running/cache/in-use" ]] || fail "a running app's cache went"
[[ -e "${HOME}/Documents/report.odt" ]] || fail "a document went, through a link or otherwise"
# Nothing is low: clearing without --all touches nothing that does not count
trashed long-gone 40
AMETHYSTORA_LOW_SPACE="0 0" "${ROOM}" clear --json >/dev/null
[[ -e "${TRASH}/files/long-gone" ]] || fail "a clear with nothing low took the trash"
rm -f "${TRASH}/files/long-gone" "${TRASH}/info/long-gone.trashinfo"

# --- Uninstall means gone ---------------------------------------------------------------------------------------------
GONE="${LIBEXEC}/amethystora-uninstall"
ID=org.example.Gone
[[ "$("${GONE}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "uninstall with no arguments is a usage error"
for bad in ../.. "../../Documents" "--user" "org" "a/b.c"; do
    [[ "$("${GONE}" trash-data "${bad}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "took the id ${bad}"
done
mkdir -p "${HOME}/.var/app/${ID}/data"
write "${HOME}/.var/app/${ID}/data/keep" 2097152
touch -d '200 days ago' "${HOME}/.var/app/${ID}/data/keep" "${HOME}/.var/app/${ID}/data"
mkdir -p "${XDG_DATA_HOME}/gnome-shell"
printf '<application-state><context id=""><application id="%s.desktop" open-window-count="0" score="1" last-seen="1700000000"/></context></application-state>\n' "${ID}" \
    >"${XDG_DATA_HOME}/gnome-shell/application_state"
info="$("${GONE}" info "${ID}" --json)"
same "its size" "$(jq -r '.data and .data_bytes >= 2097152' <<<"${info}")" true
same "when it was last opened: the later of GNOME's word and its own folder" "$(jq -r '.last_opened >= 1700000000' <<<"${info}")" true
"${GONE}" trash-data "${ID}"
[[ ! -e "${HOME}/.var/app/${ID}" ]] || fail "the folder stayed"
[[ -e "${TRASH}/files/${ID}/data/keep" ]] || fail "the folder did not reach the trash"
grep -q "^Path=${HOME}/.var/app/${ID}$" "${TRASH}/info/${ID}.trashinfo" || fail "the trash does not say where it came from"
grep -qx "flatpak permission-reset ${ID}" "${FLATPAK_LOG}" || fail "what the app was granted stayed"
grep -qx "flatpak override --user --reset ${ID}" "${FLATPAK_LOG}" || fail "the account's overrides for the app stayed"
same "the trash says what was removed" "$("${GONE}" removed --json | jq -r '.[0].id')" "${ID}"
same "nothing else is taken for an app" "$("${GONE}" removed --json | jq length)" 1
# A file named exactly .trashinfo that says it was this app's folder: never the whole of Trash/files
printf '[Trash Info]\nPath=%s/.var/app/org.example.Planted\nDeletionDate=2020-01-01T00:00:00\n' "${HOME}" >"${TRASH}/info/.trashinfo"
same "an info file with no name names nothing" "$("${GONE}" removed --json | jq -r '[.[].id] | index("org.example.Planted")')" null
: >"${FLATPAK_LOG}"
"${GONE}" undo "${ID}"
grep -qx "ask install --system flathub app/${ID}/$(uname -m)/stable" "${FLATPAK_LOG}" || fail "undo did not install again"
[[ -e "${HOME}/.var/app/${ID}/data/keep" && ! -e "${TRASH}/files/${ID}" && ! -e "${TRASH}/info/${ID}.trashinfo" ]] || fail "undo did not put it back"
# ...the way it was installed, as the desktop tells it, whatever the history says
"${GONE}" trash-data "${ID}"
: >"${FLATPAK_LOG}"
"${GONE}" undo "${ID}" --user
grep -qx "ask install --user flathub app/${ID}/$(uname -m)/stable" "${FLATPAK_LOG}" || fail "undo did not bring it back for the account"
for bad in --force "--installation=../x" "--foo=bar"; do
    [[ "$("${GONE}" undo "${ID}" "${bad}" >/dev/null 2>&1; echo $?)" == 64 ]] || fail "undo took ${bad}"
done
# The history's words, in its shape: where it was installed and from where. The journal can be written to, so only a name is believed
export FLATPAK_HISTORY="${T}/history"
"${GONE}" trash-data "${ID}"
printf 'deploy install\tapp/%s/aarch64/beta\tuser\tmy-remote\n' "${ID}" >"${FLATPAK_HISTORY}"
: >"${FLATPAK_LOG}"
"${GONE}" undo "${ID}"
grep -qx "ask install --user my-remote app/${ID}/aarch64/beta" "${FLATPAK_LOG}" || fail "undo did not go by the history: $(cat "${FLATPAK_LOG}")"
"${GONE}" trash-data "${ID}"
printf 'deploy install\tapp/%s/x86_64/beta\tsystem\t--subpath=/x\ndeploy install\tapp/%s/x86_64/beta\t--danger\tflathub\n' "${ID}" "${ID}" >"${FLATPAK_HISTORY}"
: >"${FLATPAK_LOG}"
"${GONE}" undo "${ID}"
grep -qx "ask install --system flathub app/${ID}/$(uname -m)/stable" "${FLATPAK_LOG}" || fail "undo believed a value that is no name: $(cat "${FLATPAK_LOG}")"
unset FLATPAK_HISTORY
# A folder the app has made since is left, what was removed stays in the trash, and undo says so with the app back
"${GONE}" trash-data "${ID}"
mkdir -p "${HOME}/.var/app/${ID}"
same "undo says the data stays" "$("${GONE}" undo "${ID}")" data-stays-in-trash
[[ -e "${TRASH}/files/${ID}/data/keep" ]] || fail "what was removed left the trash"
rmdir "${HOME}/.var/app/${ID}"
# An entry whose file has gone fails, with no traceback
mv "${TRASH}/files/${ID}" "${T}/kept"
[[ "$("${GONE}" undo "${ID}" 2>"${T}/said" >/dev/null; echo $?)" == 1 && ! -s "${T}/said" ]] || fail "undo with the file gone: $(cat "${T}/said")"
mv "${T}/kept" "${TRASH}/files/${ID}"
rm -rf "${TRASH}/files/${ID}" "${TRASH}/info/${ID}.trashinfo" "${TRASH}/info/.trashinfo"
# A link is moved as a link: what it leads to stays
ln -s "${HOME}/Documents" "${HOME}/.var/app/org.example.Linked"
"${GONE}" trash-data org.example.Linked
[[ -e "${HOME}/Documents/report.odt" ]] || fail "a link led the trash out of the app's folder"
# remove: the app, then its folder; --keep-data leaves the folder
mkdir -p "${HOME}/.var/app/org.example.Installed"
: >"${FLATPAK_LOG}"
"${GONE}" remove org.example.Installed --keep-data
grep -qx 'ask uninstall --system app/org.example.Installed/x86_64/stable' "${FLATPAK_LOG}" || fail "remove did not uninstall"
[[ -d "${HOME}/.var/app/org.example.Installed" ]] || fail "--keep-data took the data"
"${GONE}" remove org.example.Installed
[[ ! -e "${HOME}/.var/app/org.example.Installed" ]] || fail "remove left the data"
grep -q -- '--delete-data' "${FLATPAK_LOG}" && fail "remove went round the trash"
[[ "$("${GONE}" remove org.example.NotThere >/dev/null 2>&1; echo $?)" == 1 ]] || fail "removed what was not installed"

# --- Earlier versions ------------------------------------------------------------------------------------------------
TIME="${LIBEXEC}/amethystora-back-in-time"
cat >"${T}/bin/notify-send" <<'EOF'
#!/usr/bin/bash
echo "notify $*" >>"${FLATPAK_LOG}"
EOF
chmod +x "${T}/bin/notify-send"
SCRIPTS="${XDG_DATA_HOME}/nautilus/scripts"
LC_ALL=C LANGUAGE=C "${TIME}" menu on
ENTRY="${SCRIPTS}/Earlier versions"
[[ -x "${ENTRY}" ]] || fail "menu on wrote no entry"
same "the entry's marker" "$(sed -n 2p "${ENTRY}" | cut -c1-25)" "# amethystora-files-menu "
same "the entry's command" "$(sed -n 3p "${ENTRY}")" 'exec /usr/libexec/amethystora-back-in-time --versions -- "$@"'
LC_ALL=C LANGUAGE=C "${TIME}" menu on
same "on twice writes one entry" "$(find "${SCRIPTS}" -type f | wc -l)" 1
printf '#!/bin/sh\necho mine\n' >"${SCRIPTS}/A script of mine"
chmod +x "${SCRIPTS}/A script of mine"
"${TIME}" menu off
[[ ! -e "${ENTRY}" ]] || fail "menu off left the entry"
[[ -e "${SCRIPTS}/A script of mine" ]] || fail "menu off took a script of the person's own"
printf '#!/bin/sh\necho mine\n' >"${ENTRY}"
LC_ALL=C LANGUAGE=C "${TIME}" menu on
same "a person's own under the name stays" "$(sed -n 2p "${ENTRY}")" 'echo mine'
[[ "$("${TIME}" menu sideways >/dev/null 2>&1; echo $?)" == 64 ]] || fail "menu took a state it does not know"
rm -f "${ENTRY}"
# What the desktop looks at to tell whether the menu matches the key
OFF_MARK="${HOME}/.local/state/amethystora/files-earlier-versions-off"
"${TIME}" menu off
[[ -e "${OFF_MARK}" ]] || fail "menu off left no mark"
"${TIME}" menu on
[[ ! -e "${OFF_MARK}" ]] || fail "menu on left the mark"
"${TIME}" menu off

# Its arguments: the first file's versions, inside the home only; a folder's are the timeline
export AMETHYSTORA_BACKUPS="${T}/bin/backups"
cat >"${T}/bin/backups" <<'EOF'
#!/usr/bin/bash
printf 'backups %s\n' "$*" >>"${FLATPAK_LOG}"
EOF
chmod +x "${T}/bin/backups"
mkdir -p "${HOME}/Documents/a folder" "${T}/outside"
: >"${HOME}/Documents/one note.txt"
: >"${HOME}/Documents/-dash.txt"
: >"${T}/outside/file"
: >"${FLATPAK_LOG}"
"${TIME}" --versions -- "${HOME}/Documents/one note.txt" "${HOME}/Documents/report.odt"
same "a file: its versions, whole" "$(cat "${FLATPAK_LOG}")" "backups --versions=${HOME}/Documents/one note.txt"
: >"${FLATPAK_LOG}"
(cd "${HOME}/Documents" && "${TIME}" --versions -- -dash.txt)
same "a name that starts with a dash stays a file" "$(cat "${FLATPAK_LOG}")" "backups --versions=${HOME}/Documents/-dash.txt"
: >"${FLATPAK_LOG}"
"${TIME}" --versions -- "${HOME}/Documents/a folder"
same "a folder: its timeline" "$(cat "${FLATPAK_LOG}")" "backups ${HOME}/Documents/a folder"
: >"${FLATPAK_LOG}"
"${TIME}" --versions -- "${T}/outside/file"
same "outside the home: a notice, and Backups is not opened" "$(cat "${FLATPAK_LOG}")" "notify --app-name=Backups --icon=amethystora-backups Earlier versions covers your home folder."
: >"${FLATPAK_LOG}"
ln -s "${T}/outside/file" "${HOME}/Documents/a link"
"${TIME}" --versions -- "${HOME}/Documents/a link"
grep -q '^backups' "${FLATPAK_LOG}" && fail "a link out of the home opened Backups"
unset AMETHYSTORA_BACKUPS

# --- Look's copy from a backup goes by itself ---------------------------------------------------------------------------
# What the account's tmpfiles timer does to the shape amethystora-restore leaves: files read-only, in folders it can still write to
LOOK="${HOME}/.cache/amethystora-restore/look/point.abc"
mkdir -p "${LOOK}/inner"
: >"${LOOK}/inner/canary"
chmod -R a-w "${LOOK}"
find "${LOOK}" -type d -exec chmod u+w -- {} +
grep -q 'find "${look}" -type d -exec chmod u+w' "${LIBEXEC}/amethystora-restore" || fail "the helper leaves the copy's folders closed to the timer"
mkdir -p "${HOME}/.config/user-tmpfiles.d"
printf 'e %%C/amethystora-restore/look - - - 1s\n' >"${HOME}/.config/user-tmpfiles.d/test-look.conf"
sleep 3
XDG_RUNTIME_DIR="${T}" systemd-tmpfiles --user --clean "${HOME}/.config/user-tmpfiles.d/test-look.conf" 2>/dev/null || true
[[ ! -e "${LOOK}/inner/canary" ]] || fail "the tmpfiles timer cannot take the copy away"
rm -f "${HOME}/.config/user-tmpfiles.d/test-look.conf"

echo "test-files-group: ok"
