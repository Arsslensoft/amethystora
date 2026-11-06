#!/usr/bin/bash
# Start from your backup (R7-55) and the recovery key (`ame backup recovery-key`): a real restic repository on a folder, a second key added to it,
# then a "new machine" (the files gone, nothing set up) that brings everything back with the recovery key alone. Checks that the key is never
# left on disk, that a wrong key restores nothing, that nothing is set up or deleted, and that amethystora-restore never makes a key.
set -euo pipefail

SRC="${SRC:-$(cd "$(dirname "$0")/../.." && pwd)/system_files/shared/usr/libexec}"
[[ -x "${SRC}/amethystora-start-from-backup" ]] || SRC=/usr/libexec
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
fail() { echo "FAIL $*"; exit 1; }

# The helpers as they would be installed: each one calls the others at /usr/libexec, here the stand-in folder
mkdir -p "${T}/libexec"
for helper in amethystora-start-from-backup amethystora-restore amethystora-restore-setup amethystora-backup-recovery; do
    sed "s#/usr/libexec/#${T}/libexec/#g" "${SRC}/${helper}" >"${T}/libexec/${helper}"
    chmod +x "${T}/libexec/${helper}"
done

export HOME="${T}/home/user" XDG_RUNTIME_DIR="${T}/run" TMPDIR="${T}/tmp"
unset XDG_CONFIG_HOME XDG_STATE_HOME XDG_CACHE_HOME RESTIC_PASSWORD_FILE RESTIC_PASSWORD RESTIC_REPOSITORY
mkdir -p "${HOME}/.config/amethystora" "${XDG_RUNTIME_DIR}" "${TMPDIR}" "${T}/media/Backup"
chmod 700 "${XDG_RUNTIME_DIR}"

# The old machine: a home, a repository with its own password, a backup of the home
CANARY="canary-file-4e7c-$$"
mkdir -p "${HOME}/Documents"
echo "${CANARY}" >"${HOME}/Documents/tax.txt"
echo "second" >"${HOME}/notes.txt"
echo "old-password-$$" >"${T}/old-password"
REPO="${T}/media/Backup/restic"
cat >"${HOME}/.config/amethystora/backup.env" <<ENV
RESTIC_REPOSITORY=${REPO}
RESTIC_PASSWORD_FILE=${T}/old-password
AMETHYSTORA_BACKUP_PATHS=${HOME}
ENV
( set -a; source "${HOME}/.config/amethystora/backup.env"; set +a
  restic init >/dev/null
  restic backup --verbose=0 --tag amethystora "${HOME}/Documents" "${HOME}/notes.txt" >/dev/null
  echo '{"version":1}' | restic backup --verbose=0 --stdin --stdin-filename amethystora-setup.json --tag amethystora-setup >/dev/null )

# The recovery key: a second key, from the helper only
KEY="$("${T}/libexec/amethystora-backup-recovery" --print)"
[[ "${KEY}" =~ ^([A-HJ-NP-Z2-9]{5}-){4}[A-HJ-NP-Z2-9]{5}$ ]] || fail "the key's shape: ${KEY}"
( set -a; source "${HOME}/.config/amethystora/backup.env"; set +a
  [[ "$(restic key list --json | jq length)" -eq 2 ]] ) || fail "the repository has two keys"
[[ -z "$(ls -A "${XDG_RUNTIME_DIR}")" ]] || fail "the recovery helper left something in the runtime folder"
# ...the recovery key opens it with nothing else
printf '%s' "${KEY//-/}" >"${T}/typed"
RESTIC_REPOSITORY="${REPO}" restic --no-lock --password-file "${T}/typed" snapshots >/dev/null || fail "the key (without its dashes) does not open the repository"
RESTIC_REPOSITORY="${REPO}" restic --no-lock --password-file <(printf '%s' "${KEY}") snapshots >/dev/null 2>&1 && fail "the dashes are part of the key"

# The new machine: the files gone, no backup set up, nothing but the key
BEFORE_CONFIG="$(find "${HOME}/.config" | sort)"
rm -rf "${HOME}/Documents" "${HOME}/notes.txt" "${HOME}/.config/amethystora/backup.env"
BEFORE_CONFIG="$(find "${HOME}/.config" | sort)"

# A wrong key opens nothing and restores nothing
out="$(printf 'WRONG-KEY-0000\n' | "${T}/libexec/amethystora-start-from-backup" --repository "${REPO}" --steps files --key-stdin 2>&1)" && fail "a wrong key was accepted"
grep -q "did not open the backup" <<<"${out}" || fail "a wrong key says what: ${out}"
[[ ! -e "${HOME}/Documents/tax.txt" ]] || fail "a wrong key restored a file"

# The recovery key brings the files back
out="$(printf '%s\n' "${KEY}" | "${T}/libexec/amethystora-start-from-backup" --repository "${REPO}" --steps files --key-stdin 2>&1)" || fail "restore failed: ${out}"
[[ "$(cat "${HOME}/Documents/tax.txt")" == "${CANARY}" ]] || fail "the file did not come back"
[[ "$(cat "${HOME}/notes.txt")" == second ]] || fail "the second file did not come back"
grep -q "It holds the setup too" <<<"${out}" || fail "it did not say the setup is in the backup: ${out}"
grep -q "not backing up yet" <<<"${out}" || fail "it did not say no backup is set up: ${out}"
[[ "$(find "${HOME}/.config" | sort)" == "${BEFORE_CONFIG}" ]] || fail "it changed ~/.config"
[[ -z "$(ls -A "${XDG_RUNTIME_DIR}")" ]] || fail "it left something in the runtime folder: $(ls -A "${XDG_RUNTIME_DIR}")"

# The key is nowhere: not in the home, the temporary folders, the runtime folder, or what the command printed
if grep -rIl --exclude-dir=media "${KEY}" "${T}" >/dev/null 2>&1 && [[ -n "$(grep -rIl --exclude-dir=media "${KEY}" "${T}" 2>/dev/null)" ]]; then
    fail "the recovery key is on disk in $(grep -rIl --exclude-dir=media "${KEY}" "${T}" | head -n 1)"
fi
grep -q "${KEY}" <<<"${out}" && fail "the key was printed"

# A file that differs is kept beside the new one, never replaced
echo "changed since" >"${HOME}/notes.txt"
printf '%s\n' "${KEY}" | "${T}/libexec/amethystora-start-from-backup" --repository "${REPO}" --steps files --key-stdin >/dev/null 2>&1 || fail "second restore"
[[ "$(cat "${HOME}/notes.txt")" == "changed since" ]] || fail "a file that differs was replaced"
compgen -G "${HOME}/notes (*).txt" >/dev/null || fail "the old version was not kept beside it"

# A backup disk is found one folder down, and only a restic repository is
mkdir -p "${T}/media2/Disk/restic" "${T}/media2/Disk/photos"
mv "${REPO}"/* "${T}/media2/Disk/restic/" 2>/dev/null
# A disk whose label has a space and a dollar sign in it ("My Passport"): the settings the helper writes are quoted for the shell that sources them
mkdir -p "${T}/media3/My Pass\$port"
cp -a "${T}/media2/Disk/restic" "${T}/media3/My Pass\$port/restic"
rm -rf "${HOME}/notes (*).txt" "${HOME}/Documents"
out="$(printf '%s\n' "${KEY}" | "${T}/libexec/amethystora-start-from-backup" --repository "${T}/media3/My Pass\$port/restic" --steps files --key-stdin 2>&1)" ||
    fail "a repository with a space and a dollar sign in its name was not opened: ${out}"
[[ "$(cat "${HOME}/Documents/tax.txt")" == "${CANARY}" ]] || fail "nothing came back from a disk with a space in its name"

cat >"${T}/finder.sh" <<'SH'
source "$1"
SH
list="$(sed -n '/^is_repository()/,/^if \[\[ -z "${REPOSITORY}" \]\]; then/p' "${T}/libexec/amethystora-start-from-backup" | sed '$d')"
MEDIA="${T}/media2" bash -c "${list}; find_disks" | grep -qx "${T}/media2/Disk/restic" || fail "the backup disk was not found"
[[ "$(MEDIA="${T}/media2" bash -c "${list}; find_disks" | wc -l)" -eq 1 ]] || fail "something that is no repository was offered"

# The recovery key is made by the helper alone, and the read-only helper never makes one or deletes from a backup
for file in "${SRC}"/*; do
    [[ -f "${file}" ]] || continue
    case "${file##*/}" in amethystora-backup-recovery | amethystora-start-from-backup | amethystora-backup | amethystora-restore) continue ;; esac
    ! grep -qE 'restic[^#]* key (add|remove|passwd)' "${file}" || fail "${file##*/} changes the repository's keys"
done
! grep -E '^[^#]*restic[^#]* (key|forget|prune|rewrite)( |$)' "${SRC}/amethystora-restore" "${SRC}/amethystora-start-from-backup" "${SRC}/amethystora-restore-setup" || fail "a helper that must only read changes a backup"
grep -q 'restic key add' "${SRC}/amethystora-backup-recovery" || fail "the recovery helper does not add a key"
echo "test-start-from-backup: ok"
