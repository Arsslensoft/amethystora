#!/usr/bin/bash
# Your setup as a file (ame setup) on a machine as a person has it: GNOME's own settings, Flathub and Homebrew.
# What the account changes in its session is what `save` writes, and nothing left at the image's value; `diff`
# agrees with the file; `apply` lists what it adds, asks, does it, and finds nothing to do a second time;
# `--remove` asks again and, told no, takes nothing away; `--yes` needs no terminal; a file with an address or a
# command is refused before anything runs. A backup still stores the setup, and `ame restore-setup` reads it.
# An image of your own, the machine's side: `ame system trust-image` makes the machine take a repository's images
# only with its key's signature (bootc refuses one without), never for Amethystora's own, and
# amethystora-image-check passes here. The suites after this one get the machine back as it was.
# vmlab-paths: @sf/usr/libexec/amethystora-setup* @sf/usr/libexec/amethystora-restore-setup @sf/usr/libexec/amethystora-trust-image @sf/usr/libexec/amethystora-image-check @sf/usr/libexec/amethystora-backup @sf/*just/setup.just @sf/*just/backup.just
source "$(dirname "$0")/lib.sh"

HOME_DIR="$(realpath "$(getent passwd "${TEST_USER}" | cut -d: -f6)")"
FILE="${HOME_DIR}/.config/amethystora/setup.json"
STATE=/var/lib/vmlab-tests/40-setup
CUSTOM=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings
CUSTOM20="org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:${CUSTOM}/custom20/"
WM=org.gnome.desktop.wm.keybindings
TILING=org.gnome.shell.extensions.amethystora-ui.tiling.keybindings
TILING_KEY='.settings.keybindings["org.gnome.shell.extensions.amethystora-ui.tiling.keybindings"]["cycle-width"]'
TILING_SETTINGS=org.gnome.shell.extensions.amethystora-ui.tiling
BREW=/home/linuxbrew/.linuxbrew/bin/brew
mkdir -p "${STATE}"

# A terminal in the session, as Ptyxis opens one: in the account's user manager, which polkit takes for the
# seat's session (so Flatpak installs for the machine as for the person at it), on a pseudo-terminal of its own
session_terminal() {
    local uid steps=()
    uid="$(id -u "${TEST_USER}")"
    while [[ $# -gt 0 && "$1" != -- ]]; do steps+=("$1"); shift; done
    shift
    in_terminal "${steps[@]}" -- runuser -u "${TEST_USER}" -- env XDG_RUNTIME_DIR="/run/user/${uid}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${uid}/bus" \
        systemd-run --user --pty --wait --quiet --collect -- /usr/bin/bash -lc "$*"
}
# The LINEs that TEXT lacks, each whole
lacking() { local text="$1" line; shift; for line; do grep -qxF -- "${line}" <<<"${text}" || echo "no line: ${line}"; done; }
# verdict WHAT STATUS PROBLEMS OUTPUT: ok when STATUS is 0 and there are no PROBLEMS
verdict() { if [[ "$2" == 0 && -z "$3" ]]; then ok "$1"; else not_ok "$1" "exit status $2" "$3" "$(tail -n 15 <<<"$4")"; fi; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
# holds WHAT FILTER [JQ ARGUMENTS]: ok when the setup file is as FILTER says
holds() { if jq -e "${@:3}" "$2" "${FILE}" >/dev/null 2>&1; then ok "$1"; else not_ok "$1" "$(jq -c . "${FILE}" 2>&1 | cut -c1-3000)"; fi; }
# derive NAME FILTER [JQ ARGUMENTS]: a file of the account's, the setup file as FILTER changes it; edit changes it
derive() { jq "${@:3}" "$2" "${FILE}" >"$1" && chown "${TEST_USER}:" "$1"; }
edit() { derive "${FILE}.new" "$@" && mv -f "${FILE}.new" "${FILE}"; }
setting() { in_session "gsettings get $*" 2>/dev/null; }
has_hello() { as_user "${BREW} list --versions hello" >/dev/null 2>&1; }

# After a restart, the session comes back by itself (GDM signs the account in)
if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

# --- An image of your own, the machine's side (once) -------------------------------------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    check "amethystora-image-check passes on the running system" /usr/libexec/amethystora-image-check
    # A small public image that no key of ours signed, its repository trusted with a throwaway key: cosign is not
    # in the image, and its public keys are ECDSA P-256 in PEM, which openssl makes
    REPO=quay.io/libpod/alpine
    KEY="/etc/pki/containers/${REPO//\//-}.pub"
    REGISTRIES="/etc/containers/registries.d/${REPO//\//-}.yaml"
    POLICY=/etc/containers/policy.json
    cp -a "${POLICY}" "${STATE}/policy.json"
    openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "${STATE}/cosign.key" 2>/dev/null
    openssl pkey -in "${STATE}/cosign.key" -pubout -out "${HOME_DIR}/cosign.pub" && chown "${TEST_USER}:" "${HOME_DIR}/cosign.pub"

    out="$(ame system trust-image ghcr.io/arsslensoft/amethystora "${HOME_DIR}/cosign.pub" 2>&1)"
    status=$?
    if [[ ${status} != 0 && "${out}" == *"Amethystora's own images are checked against Amethystora's key"* ]] &&
        cmp -s "${POLICY}" "${STATE}/policy.json"; then
        ok "trust-image refuses ghcr.io/arsslensoft/amethystora, and the policy stays as it was"
    else
        not_ok "trust-image did not refuse Amethystora's own repository" "exit status ${status}" "${out}"
    fi
    out="$(session_terminal 120 '[Pp]assword' "${TEST_USER}"$'\r' -- "ame system trust-image ${REPO} ~/cosign.pub")"
    status=$?
    verdict "ame system trust-image ${REPO} ~/cosign.pub, with the account's password for sudo" "${status}" \
        "$(grep -q "^Images from ${REPO} are now taken only with a signature that " <<<"${out}" || echo "it did not say so")" "${out}"
    problems="$(
        cmp -s "${HOME_DIR}/cosign.pub" "${KEY}" || echo "${KEY} is not the key given"
        { grep -qxF "  ${REPO}:" "${REGISTRIES}" && grep -qxF "    use-sigstore-attachments: true" "${REGISTRIES}"; } ||
            echo "${REGISTRIES}: $(cat "${REGISTRIES}" 2>&1)"
        jq -e --arg r "${REPO}" --arg k "${KEY}" --slurpfile was "${STATE}/policy.json" '.transports.docker[$r] == [{
            type: "sigstoreSigned", keyPath: $k, signedIdentity: {type: "matchRepository"}}]
            and del(.transports.docker[$r]) == $was[0]' "${POLICY}" >/dev/null 2>&1 ||
            echo "${POLICY}, for it: $(jq -c --arg r "${REPO}" '.transports.docker[$r]' "${POLICY}" 2>&1)"
        for file in "${POLICY}" "${KEY}" "${REGISTRIES}"; do
            [[ "$(stat -c '%U %a' "${file}" 2>&1)" == "root 644" ]] || echo "${file}: $(stat -c '%U %a' "${file}" 2>&1)"
        done
        restorecon -nv "${POLICY}" "${KEY}" "${REGISTRIES}" 2>&1
    )"
    verdict "it wrote the key, the registries.d file and a sigstoreSigned entry, root's, 0644, labelled as SELinux has them, and nothing else" \
        0 "${problems}" ""
    check "amethystora-image-check ${REPO} passes, as an image of one's own checks itself" /usr/libexec/amethystora-image-check "${REPO}"
    # What switching to an image the key did not sign does: refused, and nothing staged
    out="$(timeout 300 bootc switch --enforce-container-sigpolicy "${REPO}:latest" 2>&1)"
    status=$?
    staged="$(bootc status --json 2>/dev/null | jq -r '.status.staged != null')"
    [[ "${staged}" == false ]] || rpm-ostree cleanup --pending >/dev/null 2>&1
    if [[ ${status} != 0 && "${out}" == *ignature* && "${staged}" == false ]]; then
        ok "bootc switch --enforce-container-sigpolicy refuses an image from ${REPO} its key did not sign"
    else
        not_ok "bootc switch did not refuse ${REPO}:latest for its signature" "exit status ${status}, staged: ${staged}" "${out}"
    fi
    skip "ame system rebase refusing an unsigned image: it offers only Amethystora's four images under ghcr.io/arsslensoft" \
        "(amethystora-rollback-helper), all signed, so it cannot be pointed at one without switching; the refusal is the" \
        "signature policy's, which the bootc switch above applies as rebase's own commands do"
    # Taken back as it was: the policy from its copy (the manual's cp from /usr/etc is for after a switch)
    cp -a "${STATE}/policy.json" "${POLICY}" && restorecon "${POLICY}"
    rm -f "${KEY}" "${REGISTRIES}" "${HOME_DIR}/cosign.pub" "${STATE}/cosign.key"
    check "the trust taken back: the policy as it was, no key, no registries.d file" \
        sh -c "cmp '${POLICY}' '${STATE}/policy.json' && ! test -e '${KEY}' -o -e '${REGISTRIES}'"
fi

# --- Your setup as a file ----------------------------------------------------------------------------------
# The file lists the apps flatpak has, so the default ones first, which install on the first boots with a network
preinstalled() { [[ "$(systemctl show flatpak-preinstall.service -p SubState -p Result --value | paste -sd ' ')" == "exited success" ]]; }
if ! wait_for 600 preinstalled; then
    if ((SUITE_PHASE < 4)); then
        echo "# the default Flatpaks were still installing after 10 minutes: on with them after a restart"
        reboot_and_continue
    fi
    not_ok "flatpak-preinstall.service did not finish" "$(systemctl status flatpak-preinstall.service --no-pager 2>&1 | tail -n 8)"
    finish
fi
check "Homebrew is set up for ${TEST_USER}" as_user "${BREW} --version"

THEME="$(in_session "amethystora-theme current")"
OTHER=nord
[[ "${THEME}" != nord ]] || OTHER=everforest
BASE="$(in_session "ame setup save -" 2>&1)"
if jq -e '.settings | del(.layout) == {keybindings: {}}' <<<"${BASE}" >/dev/null 2>&1; then
    ok "before anything is changed, the file holds no setting: each is at the image's value"
else
    not_ok "an account that changed nothing has settings in its file" "$(jq -c .settings <<<"${BASE}" 2>&1 || echo "${BASE}")"
fi
BASE="$(jq -c .settings <<<"${BASE}" 2>/dev/null || echo '{}')"

# A key of those schemas that is not a list of keys is not a keybinding, and stays out: the volume step, a number
if [[ "$(setting org.gnome.settings-daemon.plugins.media-keys volume-step)" =~ ^[0-9]+$ ]]; then
    out="$(in_session "gsettings set org.gnome.settings-daemon.plugins.media-keys volume-step 9 && ame setup save -" 2>&1)"
    status=$?
    in_session "gsettings reset org.gnome.settings-daemon.plugins.media-keys volume-step"
    verdict "a changed volume step (media-keys, a number) stays out of the file, and save still writes it" "${status}" \
        "$(jq -e '.settings.keybindings["org.gnome.settings-daemon.plugins.media-keys"] // {} | has("volume-step") | not' \
            <<<"${out}" >/dev/null 2>&1 || echo "no file, or one with the volume step")" "${out}"
else
    skip "the volume step: org.gnome.settings-daemon.plugins.media-keys has no volume-step here"
fi

# Changed as a person would in Settings: the close key, the image's terminal key (custom20), the dock, the
# tiling's width key and the gap between its windows, the theme; and a keybinding of the account's own (custom90),
# whose command no file may carry
in_session "gsettings set ${WM} close \"['<Super>q']\"
    gsettings set ${CUSTOM20} binding \"'<Super><Alt>Return'\"
    gsettings set org.gnome.shell favorite-apps \"['org.gnome.Nautilus.desktop', 'org.gnome.Calculator.desktop']\"
    gsettings set ${TILING} cycle-width \"['<Super>F10']\"
    gsettings set ${TILING_SETTINGS} window-gap 12
    dconf write ${CUSTOM}/custom90/command \"'touch /tmp/vmlab-custom90'\"
    dconf write ${CUSTOM}/custom90/binding \"'<Super>F12'\"
    amethystora-theme set ${OTHER}" >/dev/null 2>&1
start=${SECONDS}
out="$(in_session "ame setup save" 2>&1)"
status=$?
echo "# ame setup save took $((SECONDS - start)) s"
verdict "ame setup save writes ~/.config/amethystora/setup.json" "${status}" \
    "$(grep -qF "Saved this machine's setup to ${FILE}:" <<<"${out}" && test -s "${FILE}" || echo "no file at ${FILE}")" "${out}"
holds "its settings are the dock, the close key, custom20's key, the tiling's width key and its gap as changed, and nothing else" \
    '.settings == ($base * $want)' --argjson base "${BASE}" --argjson want '{
        "favorites": ["org.gnome.Nautilus.desktop", "org.gnome.Calculator.desktop"],
        "keybindings": {"org.gnome.desktop.wm.keybindings": {"close": ["<Super>q"]}, "custom": {"custom20": ["<Super><Alt>Return"]},
            "org.gnome.shell.extensions.amethystora-ui.tiling.keybindings": {"cycle-width": ["<Super>F10"]}},
        "tiling": {"window-gap": 12}}'
holds "it holds the theme set and the default apps as flatpak lists them" \
    '.theme == $theme and any(.flatpaks.system[]; . == {app: "org.gnome.Calculator", origin: "flathub"})' --arg theme "${OTHER}"
holds "it holds no address and no command: no remote, Brewfile or template, not the account's own keybinding" '
    (keys - ["version", "image", "flatpaks", "brew", "containers", "extensions", "theme", "settings"]) == []
    and ([.. | objects | keys[]] | any(IN("remotes", "url", "brewfile", "templates", "managers", "aur", "command",
        "reference", "gpg_key", "created")) | not)
    and ([.. | strings] | any(test("://|touch |xdg-terminal-exec")) | not)
    and (.settings.keybindings.custom // {} | has("custom90") | not)'

out="$(in_session "ame setup diff" 2>&1)"
status=$?
json="$(in_session "ame setup diff --json" 2>&1)"
verdict "ame setup diff says the machine is as the file says" "${status}" \
    "$(grep -q "^This machine is as .* says\.$" <<<"${out}" && [[ "$(jq -c . <<<"${json}" 2>&1)" == "[]" ]] ||
        echo "--json: $(cut -c1-500 <<<"${json}")")" "${out}"
printf '{"version": 1, "settings": {"keybindings": {"custom": {"custom90": ["<Super>F11"]}}}}\n' >"${HOME_DIR}/vmlab-own.json"
chown "${TEST_USER}:" "${HOME_DIR}/vmlab-own.json"
out="$(in_session "ame setup diff --json ~/vmlab-own.json" 2>&1)"
if jq -e 'any(.[]; .area == "keybinding" and .key == "custom90" and .ok == false)' <<<"${out}" >/dev/null 2>&1; then
    ok "a file cannot set the account's own custom keybinding: only those the image's dconf has a command for"
else
    not_ok "a file may set the account's own custom keybinding custom90" "$(jq -c '[.[] | select(.area == "keybinding")]' <<<"${out}" 2>&1)"
fi

# The app to add: a small one on a runtime the default apps brought already, so that only the app downloads
runtimes="$(flatpak list --system --runtime --columns=ref)"
APP=""
for candidate in org.gnome.Sudoku org.gnome.Mahjongg org.gnome.Tetravex org.gnome.Mines org.gnome.design.Palette; do
    flatpak info --system "${candidate}" >/dev/null 2>&1 && continue
    runtime="$(LC_ALL=C flatpak remote-info --system flathub "${candidate}" 2>/dev/null | sed -n 's/^ *Runtime: //p')"
    [[ -n "${runtime}" ]] && grep -qxF "${runtime}" <<<"${runtimes}" && { APP="${candidate}"; break; }
done
[[ -n "${APP}" ]] || { APP=org.gnome.Sudoku; echo "# no candidate shares a runtime installed here: ${APP} brings its own"; }
echo "# the app added: ${APP}, and Homebrew's hello"

# The file gains the app and hello, and its close key, the tiling's gap and the theme change: diff names them,
# apply lists them, asks, does them, and run again has nothing to do
edit '.flatpaks.system += [{app: $app, origin: "flathub"}] | .brew.formulae += ["hello"]
    | .settings.keybindings["org.gnome.desktop.wm.keybindings"].close = ["<Super><Shift>q"]
    | .settings.tiling["window-gap"] = 16 | .theme = $theme' \
    --arg app "${APP}" --arg theme "${THEME}"
CHANGES=("  + the app ${APP}, from flathub" "  + the Homebrew formula hello"
    "  ~ the keybinding ${WM} close: <Super><Shift>q, in place of <Super>q" "  ~ the tiling's window-gap: 16, in place of 12"
    "  ~ the theme: ${THEME}, in place of ${OTHER}")
out="$(in_session "ame setup diff" 2>&1)"
status=$?
verdict "ame setup diff names the app and formula added to the file, and its key and theme" "${status}" \
    "$(lacking "${out}" "${CHANGES[@]}")" "${out}"
start=${SECONDS}
out="$(session_terminal 900 'Go ahead\?' y -- "ame setup apply")"
status=$?
echo "# ame setup apply took $((SECONDS - start)) s, with ${APP} and hello to download"
verdict "ame setup apply lists them, asks, and does them" "${status}" "$(lacking "${out}" "${CHANGES[@]}" "Done.")" "${out}"
check "${APP} is installed for the machine, from flathub" flatpak info --system "${APP}"
check "hello is installed from Homebrew" has_hello
same "the close key is the file's" "$(setting "${WM}" close)" "['<Super><Shift>q']"
same "the tiling's gap is the file's" "$(setting "${TILING_SETTINGS}" window-gap)" "16"
same "the theme is the file's" "$(in_session "amethystora-theme current")" "${THEME}"
out="$(in_session "ame setup apply" 2>&1)"
status=$?
verdict "run again, ame setup apply finds nothing to do, and asks nothing" "${status}" \
    "$(grep -q "^Nothing to do: this machine has everything " <<<"${out}" || echo "it did not say there was nothing to do")" "${out}"

# Without a terminal, apply asks nothing: it changes nothing unless --yes is given
edit "${TILING_KEY} = [\"<Super>F11\"]"
out="$(in_session "ame setup apply" 2>&1)"
status=$?
gap="$(setting "${TILING}" cycle-width)"
if [[ ${status} != 0 && "${out}" == *"apply asks before it changes anything, which needs a terminal, or --yes."* && "${gap}" == "['<Super>F10']" ]]; then
    ok "without a terminal or --yes, apply changes nothing and says why"
else
    not_ok "without a terminal or --yes, apply did not stop" "exit status ${status}, cycle-width ${gap}" "${out}"
fi
out="$(in_session "ame setup apply --yes" 2>&1)"
status=$?
gap="$(setting "${TILING}" cycle-width)"
verdict "ame setup apply --yes needs no terminal: the tiling's width key is the file's" "${status}" \
    "$([[ "${gap}" == "['<Super>F11']" ]] || echo "cycle-width is ${gap}")" "${out}"

# A file from before the tiling replaced PaperWM, with its gaps in it, is read and its gaps ignored beside the
# tiling's own
derive "${HOME_DIR}/vmlab-old.json" '.settings.paperwm = {"window-gap": 20}'
out="$(in_session "ame setup apply --yes ~/vmlab-old.json" 2>&1)"
status=$?
verdict "a file that still has PaperWM's gaps beside the tiling's settings is accepted, and they are ignored" "${status}" \
    "$(grep -q "^Nothing to do" <<<"${out}" || echo "it found something to do")" "${out}"

# A file somebody shared, with an address or a command in it, and the width key set to something else: refused,
# and the key stays as it was
refused() {
    derive "${HOME_DIR}/vmlab-bad.json" "$2"" | ${TILING_KEY} = [\"<Super>F9\"]"
    out="$(in_session "ame setup apply --yes ~/vmlab-bad.json" 2>&1)"
    status=$?
    gap="$(setting "${TILING}" cycle-width)"
    if [[ ${status} != 0 && "${out}" == *"is not a setup file this version of Amethystora reads:"* && "${gap}" == "['<Super>F11']" ]]; then
        ok "a file with $1 is refused before anything runs"
    else
        not_ok "a file with $1 was not refused, or something ran" "exit status ${status}, cycle-width ${gap}" "${out}"
    fi
}
refused "a Flatpak remote's address" '.remotes = {system: [{name: "elsewhere", url: "https://example.com/repo"}]}'
refused "a custom keybinding's command" '.settings.keybindings.custom.custom20 = {command: "touch /tmp/vmlab-ran"}'
refused "the tiling's gap outside its range" '.settings.tiling["window-gap"] = 500'
refused "the tiling's window rules" '.settings.tiling.winprops = ["{\"wm_class\": \"x\", \"scratch_layer\": true}"]'

# A file with less in it: --remove lists what it would take away and asks again; told no, it takes nothing
derive "${HOME_DIR}/vmlab-less.json" '.flatpaks.system -= [{app: $app, origin: "flathub"}] | .brew.formulae -= ["hello"]' \
    --arg app "${APP}"
out="$(session_terminal 120 'Take these away\?' n -- "ame setup apply --remove ~/vmlab-less.json")"
status=$?
problems="$(
    lacking "${out}" "Here, and not in the file:" "  - the app ${APP}" "  - the Homebrew formula hello"
    grep -qx "Done." <<<"${out}" && echo "it said Done."
    flatpak info --system "${APP}" >/dev/null 2>&1 || echo "${APP} was uninstalled"
    has_hello || echo "hello was uninstalled"
)"
verdict "ame setup apply --remove asks again before taking anything away, and told no takes nothing" "${status}" "${problems}" "${out}"
out="$(in_session "ame setup apply --remove --yes ~/vmlab-less.json" 2>&1)"
status=$?
if [[ ${status} != 0 && "${out}" == *"--remove asks before it takes anything away, which needs a terminal."* ]] &&
    flatpak info --system "${APP}" >/dev/null 2>&1; then
    ok "--yes does not answer --remove's question: without a terminal nothing is taken away"
else
    not_ok "apply --remove --yes without a terminal did not stop" "exit status ${status}" "${out}"
fi

# --- The setup beside a backup, and ame restore-setup --------------------------------------------------------
# ame backup setup, answered as a person would: a repository on this disk, a password, the home folder
out="$(session_terminal 300 'Repository' $'/var/tmp/vmlab-restic\r' 'Repository password' $'vmlab-backup\r' \
    'What to back up' $'\r' -- "ame backup setup")"
status=$?
verdict "ame backup setup makes a restic repository here and turns the daily run on" "${status}" \
    "$(lacking "${out}" "Set up, and running once a day. Files come back from it in Backups, in the app grid.")" "${out}"
start=${SECONDS}
in_session "ame backup now" >/dev/null 2>&1
echo "# ame backup now took $((SECONDS - start)) s"
result="$(user_systemctl show amethystora-backup.service -p Result --value)"
if [[ "${result}" == success ]]; then
    ok "ame backup now backs the account up"
else
    not_ok "the backup ended as ${result}" "$(journalctl -b _SYSTEMD_USER_UNIT=amethystora-backup.service --no-pager -o cat | tail -n 15)"
fi
restic_as() { as_user "set -a && . ~/.config/amethystora/backup.env && set +a && restic $*"; }
manifest="$(restic_as "dump --tag amethystora-setup latest /amethystora-setup.json" 2>&1)"
if jq -e --arg app "${APP}" '.version == 1 and any(.brew.formulae[]; . == "hello") and any(.flatpaks.system[]; .app == $app)
    and .settings.keybindings["org.gnome.shell.extensions.amethystora-ui.tiling.keybindings"]["cycle-width"] == ["<Super>F11"]
    and .settings.keybindings["org.gnome.desktop.wm.keybindings"].close == ["<Super><Shift>q"]
    and .settings.tiling["window-gap"] == 16' <<<"${manifest}" >/dev/null 2>&1; then
    ok "the backup stored the setup, tagged amethystora-setup, with its Homebrew formulae and settings"
else
    not_ok "the backup has no setup as the machine has it" "$(restic_as "snapshots --compact" 2>&1)" "$(cut -c1-2000 <<<"${manifest}")"
fi
# On the machine it came from, it offers only what cannot be told here already: the extensions, the theme and
# Homebrew. Escape at the choice runs none of them.
out="$(session_terminal 120 'Which of these' $'\e' -- "ame restore-setup")"
status=$?
problems="$(
    grep -q "^The setup of ${TEST_USER} on " <<<"${out}" || echo "it did not say whose setup it read"
    lacking "${out}" "  GNOME extensions: the $(jq '.extensions | length' <<<"${manifest}" 2>/dev/null) that were on" "  The theme: ${THEME}"
    grep -qE "^  Homebrew: [0-9]+ formulae and casks$" <<<"${out}" || echo "no Homebrew step"
    grep -E "^  (The system|Apps|Containers):" <<<"${out}" | sed 's/^/offered though here already: /'
    grep -q "^Done\. " <<<"${out}" && echo "it ran the steps"
)"
verdict "ame restore-setup reads the setup in the backup, lists its steps, and with none chosen runs none" "${status}" "${problems}" "${out}"

# --- Back as it was, for the suites after this one ------------------------------------------------------------
in_session "gsettings reset ${WM} close; gsettings reset ${CUSTOM20} binding; gsettings reset org.gnome.shell favorite-apps
    gsettings reset ${TILING} cycle-width; gsettings reset ${TILING_SETTINGS} window-gap; dconf reset -f ${CUSTOM}/custom90/
    [[ \"\$(amethystora-theme current)\" == ${THEME} ]] || amethystora-theme set ${THEME}
    ame backup off; ${BREW} uninstall --formula hello; rm -rf ~/.cache/restic" >/dev/null 2>&1
# Whatever ame backup setup wrote goes, for the suites that set up a backup of their own: the timer, the settings
# (backup.env and the password file it names), the last success's stamp, and the repository
user_systemctl disable --now amethystora-backup.timer >/dev/null 2>&1
BACKUP_LEFT=(/var/tmp/vmlab-restic "${HOME_DIR}/.config/amethystora/backup.env" "${HOME_DIR}/.config/amethystora/backup-password"
    "${HOME_DIR}/.local/state/amethystora/backup-last-success")
rm -rf "${BACKUP_LEFT[@]}" "${FILE}" "${HOME_DIR}"/vmlab-*.json
flatpak uninstall --system --noninteractive "${APP}" >/dev/null 2>&1
after="$(in_session "ame setup save -" 2>/dev/null)"
problems="$(
    jq -e --argjson base "${BASE}" --arg theme "${THEME}" '.settings == $base and .theme == $theme' <<<"${after}" >/dev/null 2>&1 ||
        echo "settings and theme: $(jq -c '{settings, theme}' <<<"${after}" 2>&1)"
    flatpak info --system "${APP}" >/dev/null 2>&1 && echo "${APP} is still installed"
    has_hello && echo "hello is still installed"
    user_systemctl is-enabled --quiet amethystora-backup.timer && echo "the daily backup is still on"
    for left in "${BACKUP_LEFT[@]}" "${FILE}"; do [[ ! -e "${left}" ]] || echo "${left} is still there"; done
)"
verdict "the account is as it was before this suite: settings, theme, apps, no backup, no setup file" 0 "${problems}" ""
finish
