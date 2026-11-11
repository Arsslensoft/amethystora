#!/usr/bin/bash
# amethystora-audit-home-rules (/usr/libexec/amethystora-audit-home-rules, or the copy named first) against
# a stand-in /etc/passwd and /var/home, for 20-tests.sh. It runs as root at every boot, so: it gives an
# account back what an older version left to root inside its home, and it never follows a link the account
# made out of the home, neither to change an owner nor to write a watch, nor writes a rule auditctl cannot
# read. Runs as root, as the generator does.

set -euo pipefail

GENERATOR="${1:-/usr/libexec/amethystora-audit-home-rules}"
T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
chmod 0755 "${T}"
export AMETHYSTORA_AUDIT_ROOT="${T}/root"
R="${AMETHYSTORA_AUDIT_ROOT}"
RULES="${R}/etc/audit/rules.d/61-amethystora-home.rules"
mkdir -p "${R}/etc/audit/rules.d" "${R}/var/home" "${T}/system/systemd/user" "${T}/system/containers/systemd"
: >"${R}/etc/passwd"

account() {
    mkdir -m 0700 "${R}/var/home/$1"
    chown "$2:$2" "${R}/var/home/$1"
    echo "$1:x:$2:$2::${R}/var/home/$1:/bin/bash" >>"${R}/etc/passwd"
}
# A link or folder made as the account, as it would be
as() {
    local uid="$1"
    shift
    setpriv --reuid="${uid}" --regid="${uid}" --clear-groups "$@"
}
account mallory 4201
account alice 4202
M="${R}/var/home/mallory"
A="${R}/var/home/alice"

# Mallory points ~/.config at folders of the system's, ~/.local/bin at / and ~/.bashrc at a system file
as 4201 ln -s "${T}/system" "${M}/.config"
as 4201 mkdir -m 0700 "${M}/.local"
as 4201 ln -s / "${M}/.local/bin"
touch "${T}/system/bashrc"
as 4201 ln -s "${T}/system/bashrc" "${M}/.bashrc"

# Alice has what an older version left: the folders on the way to one it made owned by root. Her autostart
# folder and .bashrc are links into her own dotfiles, and her launchers live in a folder with a space
mkdir -p "${A}/.local/share/flatpak/overrides"
chown 4202:4202 "${A}/.local/share/flatpak/overrides"
as 4202 mkdir -p "${A}/dotfiles/autostart" "${A}/.config" "${A}/My Stuff/applications"
as 4202 touch "${A}/dotfiles/bashrc"
as 4202 ln -s ../dotfiles/autostart "${A}/.config/autostart"
as 4202 ln -s dotfiles/bashrc "${A}/.bashrc"
ln -s "${A}/My Stuff/applications" "${A}/.local/share/applications"
as 4202 touch "${A}/.config/mimeapps.list"

# Its list of places, which the security watcher reads, changes nothing
bash "${GENERATOR}" --paths | grep -qx .config/systemd/user
bash "${GENERATOR}" --paths | grep -qx bin
[[ ! -e "${RULES}" ]]

bash "${GENERATOR}"

# Nothing outside a home changed owner, and nothing outside a home is watched
for folder in system system/systemd system/systemd/user system/containers system/containers/systemd; do
    [[ "$(stat -c %u "${T}/${folder}")" == 0 ]] || { echo "given away: ${folder}"; false; }
done
grep -E '^-' "${RULES}" | grep -vE "^-w ${R}/var/home/(mallory|alice)/[^ ]+ -p wa -k (persistence|user-programs)$" &&
    { echo "a rule outside the homes, or one auditctl cannot read"; false; }
grep -qF -- "${T}/system" "${RULES}" && { echo "watches a folder of the system's"; false; }
grep -qF "My Stuff" "${RULES}" && { echo "wrote a path with a space"; false; }

# Both accounts were handled, mallory's links did not stop the run before alice's
grep -qx "## mallory" "${RULES}"
grep -qx "## alice" "${RULES}"

# Alice got back what root held inside her home, and her links inside it are watched where they lead
for folder in .local .local/share .local/share/flatpak; do
    [[ "$(stat -c %u "${A}/${folder}")" == 4202 ]] || { echo "not given back: ${folder}"; false; }
done
grep -qxF -- "-w ${A}/.local/share/flatpak/overrides/ -p wa -k persistence" "${RULES}"
# Widgets are plugins, run by the host from the root's store: nothing in the home runs by itself any more
grep -qF -- ".config/amethystora/widgets" "${RULES}" && { echo "the old widgets folder is still watched"; false; }
grep -qxF -- "-w ${A}/dotfiles/autostart/ -p wa -k persistence" "${RULES}"
grep -qxF -- "-w ${A}/dotfiles/bashrc -p wa -k persistence" "${RULES}"
grep -qxF -- "-w ${A}/.local/bin/ -p wa -k user-programs" "${RULES}"
# Which app opens what is rewritten at every login (an app registers for a type at a login): recorded only
grep -qxF -- "-w ${A}/.config/mimeapps.list -p wa -k user-programs" "${RULES}"
grep -qF -- "mimeapps.list -p wa -k persistence" "${RULES}" && { echo "mimeapps.list is said as persistence"; false; }
[[ "$(stat -c %a "${RULES}")" == 600 ]]

# With CREDENTIALS on and the sensor in use, one home holding thousands of browser profiles cannot cost the run: its
# secrets stop at a bound and its UID is said in `skipped`, and the homes after it are read. The plan and the watch
# list are the sensor's alone to read.
account bulk 4203
account omega 4204
as 4203 bash -c 'cd "$1" && for i in $(seq 350); do mkdir -p ".config/chromium/p$i"; : >".config/chromium/p$i/Cookies"; done' _ "${R}/var/home/bulk"
as 4204 touch "${R}/var/home/omega/.npmrc"
printf 'CREDENTIALS=on\n' >"${T}/security.conf"
AMETHYSTORA_SECURITY_CONF="${T}/security.conf" AMETHYSTORA_SENSOR_FORCE=sensor bash "${GENERATOR}"
RUN="${R}/run/amethystora/sensor"
grep -qx 4203 "${RUN}/skipped" || { echo "a home that cost too much was not said"; false; }
grep -qx 4204 "${RUN}/skipped" && { echo "a home that cost little was said"; false; }
[[ "$(grep -c "^read-file credentials ${R}/var/home/bulk/" "${RUN}/watch.list")" -le 300 ]] || { echo "one home was not bounded"; false; }
grep -qx "read-file credentials ${R}/var/home/omega/.npmrc" "${RUN}/watch.list" || { echo "the home after it was not read"; false; }
grep -qx 'identity sensor' "${RUN}/plan"
[[ "$(stat -c %a "${RUN}/watch.list")" == 600 && "$(stat -c %a "${RUN}/plan")" == 600 ]] ||
    { echo "the plan or the watch list can be read by anyone"; false; }

echo "test-audit-home-rules: every check passed"
