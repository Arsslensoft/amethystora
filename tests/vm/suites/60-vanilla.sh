#!/usr/bin/bash
# Software from other distributions (amethystora-pkg), the NVIDIA check and "You're on the previous version"
# (plans/done/PLAN-vanilla-features.md, items 1 to 3), as a machine runs them. A phase each, for the time each takes:
#
#   1. a container made from the alpine template and used as the account would, the watcher's look at a fake
#      sudo and a stray launcher beside the launcher it exported, the daily upgrade and the report's check on it,
#      definitions imported, and a file that matches known malware refused
#   2. a .deb from a file: scanned, explained, asked about, then installed in a Debian container made for it
#   3. the NVIDIA check, here and on stand-in hardware; then everything amethystora-pkg made goes
#   4. a newer version of the image made the default while this one runs, as picking the second boot entry
#      leaves a machine, and "Keep this version"
# vmlab-paths: @sf/usr/bin/amethystora-pkg @sf/usr/share/amethystora/pkg/* @sf/usr/libexec/amethystora-pkg* @sf/etc/containers/* @sf/usr/libexec/amethystora-gpu-check @sf/usr/libexec/amethystora-update-alert* @sf/usr/share/applications/zz-amethystora-open-anything.desktop build_files/shared/open-anything-types.py @ui/modules/tools/opener.js
source "$(dirname "$0")/lib.sh"

HOME_DIR="/var/home/${TEST_USER}"
STATE=/var/lib/vmlab-tests/60-vanilla
HISTORY=/var/log/amethystora-security-events.jsonl
TEMPLATES=/usr/share/amethystora/pkg/templates
# A pinned template's image as amethystora-pkg pulls it: its repository and digest, without the tag
pull_ref() {
    local base repo
    base="$(jq -r .base "${TEMPLATES}/$1.json")"
    repo="${base%%@*}"
    printf '%s@%s\n' "${repo%:*}" "${base#*@}"
}
ALPINE="$(jq -r .base "${TEMPLATES}/alpine.json")"
ALPINE_PULL="$(pull_ref alpine)"
DEBIAN_PULL="$(pull_ref debian)"
LAUNCHERS="${HOME_DIR}/.local/share/applications"

mkdir -p "${STATE}"
# After a restart, the session comes back by itself (GDM signs the account in)
if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
TUID="$(id -u "${TEST_USER}")"
# A terminal in the account's session, for what asks there: term.py types the answers (in_terminal)
AS_TESTER=(runuser -u "${TEST_USER}" -- env "XDG_RUNTIME_DIR=/run/user/${TUID}"
    "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/${TUID}/bus" bash -lc)
# put PATH [MODE] <CONTENT: a file in the account's home, written as the account
put() { runuser -u "${TEST_USER}" -- sh -c 'mkdir -p "${1%/*}" && cat >"$1" && chmod "$2" "$1"' put "${HOME_DIR}/$1" "${2:-0644}"; }
# line_of TEXT STRING: the number of the first line of TEXT holding STRING
line_of() { grep -n -m1 -F -- "$2" <<<"$1" | cut -d: -f1; }
pkg_list() { in_session "amethystora-pkg $1 list --json" 2>/dev/null; }
plain_status() { in_session "podman container inspect vmlab-plain --format '{{.State.Status}}'" 2>/dev/null; }

if [[ "${SUITE_PHASE}" == 1 ]]; then
    touch "${STATE}/started"
    for kept in .local/share/containers .local/bin .local/share/applications/mimeinfo.cache; do
        [[ -e "${HOME_DIR}/${kept}" ]] && touch "${STATE}/had-${kept##*/}"
    done

    # The lists parse and carry the fields containers.md#in-scripts names, through each of the command's names
    check "amepkg templates list --json: the documented fields; alpine built in, pinned, verified" jq -e '
        all(.[]; (["name","description","base","manager","packages","own_home","unshare","built_in","verified","pinned"] - keys) == [])
        and any(.[]; .name == "alpine" and .built_in and .pinned and .verified)' <<<"$(in_session 'amepkg templates list --json' 2>/dev/null)"
    check "ame pkg managers list --json: the documented fields; apk built in" jq -e '
        all(.[]; (["name","need_sudo","noconfirm","install","remove","purge","search","show","list","update","upgrade",
            "autoremove","clean","built_in"] - keys) == []) and any(.[]; .name == "apk" and .built_in)' <<<"$(in_session 'ame pkg managers list --json' 2>/dev/null)"
    check "a .deb and an .rpm open in the image's package installer" in_session '
        [ "$(xdg-mime query default application/vnd.debian.binary-package)" = amethystora-pkg-install.desktop ] &&
        [ "$(xdg-mime query default application/x-rpm)" = amethystora-pkg-install.desktop ]'

    # A container from the smallest template's image (43 MiB)
    began=${SECONDS}
    out="$(in_session 'amepkg containers new --template alpine --name vmtest --no-prompt --yes' 2>&1)"
    echo "# the alpine container, pulled, made and brought up to date: $((SECONDS - began)) s"
    if grep -q "^Made vmtest\." <<<"${out}"; then ok "containers new makes vmtest from the alpine template"; else not_ok "containers new did not make vmtest" "$(tail -n 20 <<<"${out}")"; fi
    info="$(in_session 'podman container inspect pkg-vmtest' 2>/dev/null)"
    check "it is pkg-vmtest, labelled amethystora.pkg.*, and distrobox's manager=distrobox is kept" jq -e --arg base "${ALPINE}" '
        .[0].Config.Labels | .["amethystora.pkg.manager"] == "amethystora-pkg" and .["amethystora.pkg.name"] == "vmtest"
        and .["amethystora.pkg.template"] == "alpine" and .["amethystora.pkg.pkgmanager"] == "apk"
        and .["amethystora.pkg.image"] == $base and .manager == "distrobox"' <<<"${info}"
    check "it runs the template's pin, pulled by its digest" jq -e --arg ref "${ALPINE_PULL}" '.[0].ImageName == $ref' <<<"${info}"
    # policy.json asks quay.io/toolbx-images for a sigstore signature. Without the registries.d entry that says
    # where its signatures are, the same pin is refused, so the pull above passed the check.
    none="$(mktemp -d)"
    out="$(skopeo --registries.d "${none}" copy "docker://${ALPINE_PULL}" "dir:${none}/image" 2>&1)" && out="copied: ${out}"
    if grep -q "no signature exists" <<<"${out}"; then ok "the pin is taken only with its publisher's signature"; else not_ok "policy.json did not ask for the alpine image's signature" "${out}"; fi
    rm -rf "${none}"

    # htop: 286 KiB in Alpine 3.22's main repository, not in the toolbox image, and its package carries a
    # launcher, /usr/share/applications/htop.desktop (Terminal=true, so not hidden)
    launcher="${LAUNCHERS}/pkg-vmtest-htop.desktop"
    out="$(in_session 'amepkg vmtest install --yes htop' 2>&1)"
    if grep -q '^Exec=.*distrobox-enter.* -n pkg-vmtest ' "${launcher}" 2>/dev/null && grep -q '^Name=.* (on vmtest)$' "${launcher}"; then
        ok "install puts the package's launcher in the app grid, marked (on vmtest)"
    else
        not_ok "no launcher for htop in the app grid" "$(tail -n 15 <<<"${out}")"
    fi
    out="$(in_session 'amepkg vmtest run cat /etc/alpine-release' 2>/dev/null | tail -n 1)"
    if [[ "${out}" =~ ^3\.22\. ]]; then ok "run runs a command in it (Alpine ${out})"; else not_ok "run printed: ${out}"; fi
    check "containers list --json lists it with the documented fields, its package and its launcher" jq -e '
        map(.name) == ["vmtest"] and (.[0] | (["name","container","template","manager","image","status","home","init","unshare",
            "exported_apps","exported_bins","packages","files","aur_packages","aur_pending","created","last_upgrade",
            "last_upgrade_result"] - keys) == [] and .container == "pkg-vmtest" and .template == "alpine"
        and any(.packages[]; . == "htop") and any(.exported_apps[]; .file | endswith("/pkg-vmtest-htop.desktop")))' <<<"$(pkg_list containers)"
    # One the user made with distrobox alone, from the same image
    in_session "distrobox create --yes --no-entry --name vmlab-plain --image ${ALPINE_PULL}" >/dev/null 2>&1
    plain_before="$(plain_status)"

    # What a session finds first: a sudo in ~/.local/bin, and a launcher no container export made, beside the
    # one distrobox-export made. The watcher's first run only records what is there. A finding carries its run's
    # start, so no run may be under way when the time is taken: one its timer started earlier would find them
    # dated before it.
    watcher_idle() { [[ "$(systemctl show -p ActiveState --value amethystora-security-watch.service)" != activating ]]; }
    [[ -e /var/lib/amethystora/security/user-paths-seen ]] || systemctl start amethystora-security-watch.service
    wait_for 600 watcher_idle
    since="$(date +%s)"
    printf '#!/bin/sh\n# vmlab-%s\nexec /usr/bin/sudo "$@"\n' "${since}" | put .local/bin/sudo 0755
    printf '[Desktop Entry]\nType=Application\nName=VM lab stray\nExec=/usr/bin/true vmlab-%s\nNoDisplay=true\n' "${since}" |
        put .local/share/applications/vmlab-stray.desktop
    wait_for 600 watcher_idle
    check "the security watcher runs" systemctl start amethystora-security-watch.service
    found="$(jq -c --argjson t "${since}" 'select(.time >= $t and .key == "user-paths")' "${HISTORY}" 2>/dev/null)"
    expect() {
        local what="$1" filter="$2"
        if jq -e -s "any(.[]; ${filter})" <<<"${found}" >/dev/null 2>&1; then ok "found: ${what}"; else not_ok "not found: ${what}" "$(jq -r -s '.[] | "\(.level): \(.text)"' <<<"${found}" | tail -n 15)"; fi
    }
    expect "a sudo in ~/.local/bin, as critical" '.level == "critical" and (.text | contains(".local/bin/sudo"))'
    expect "a launcher no container export made" '.text | contains("vmlab-stray.desktop")'
    if [[ -f "${launcher}" ]] && ! jq -e -s 'any(.[]; .text | contains("pkg-vmtest-htop.desktop"))' <<<"${found}" >/dev/null; then
        ok "not found: the launcher distrobox-export made"
    else
        not_ok "the watcher reported the launcher distrobox-export made, or it was gone"
    fi
    rm -f "${HOME_DIR}/.local/bin/sudo" "${LAUNCHERS}/vmlab-stray.desktop"

    out="$(in_session 'amepkg vmtest remove --yes htop' 2>&1)"
    if [[ ! -e "${launcher}" ]]; then ok "remove takes the launcher out of the app grid"; else not_ok "remove left ${launcher}" "$(tail -n 10 <<<"${out}")"; fi

    # The daily upgrade, by its own unit, as the timer starts it (without the timer's hardware hold-back)
    since="$(date +%s)"
    check "amethystora-pkg-upgrade.service runs" user_systemctl start amethystora-pkg-upgrade.service
    sleep 2
    check "it logs a line of JSON per container to the journal, then one with the count" jq -e -s '
        any(.[]; .container == "vmtest" and .state == "ok") and any(.[]; .done and .count == 1 and .failed == 0)' \
        <<<"$(journalctl --since "@${since}" -t amethystora-pkg-upgrade -o cat --no-pager 2>/dev/null)"
    last_run="$(cat "${HOME_DIR}/.local/state/amethystora/pkg/last-run" 2>/dev/null)"
    if [[ "${last_run}" =~ ^[0-9]+$ ]] && ((last_run >= since)) && jq -e --argjson t "${since}" \
        'any(.[]; .name == "vmtest" and .last_upgrade >= $t and .last_upgrade_result == "ok")' <<<"$(pkg_list containers)" >/dev/null; then
        ok "it records the date, the container's and the run's"
    else
        not_ok "no date recorded (last-run: ${last_run:-none})"
    fi
    # Started by its timer with no network, as when it catches up at a login before Wi-Fi: it waits a minute,
    # then leaves the containers for the next day, saying why, rather than counting each one failed
    nmcli networking off
    out="$(in_session 'TRIGGER_UNIT=amethystora-pkg-upgrade.timer amethystora-pkg upgrade-all' 2>&1)"
    nmcli networking on
    if grep -qF 'Not upgrading the containers now: there is no network.' <<<"${out}"; then
        ok "with no network, the timer's run waits for it, then upgrades nothing and says why"
    else
        not_ok "the timer's run with no network" "$(tail -n 10 <<<"${out}")"
    fi
    check "the network is back" nm-online -q -t 120
    out="$(in_session 'amepkg vmlab-plain run true' 2>&1)"
    if [[ -n "${plain_before}" && "$(plain_status)" == "${plain_before}" ]] && grep -q "There is no container called vmlab-plain" <<<"${out}" &&
        jq -e 'all(.[]; .container != "vmlab-plain")' <<<"$(pkg_list containers)" >/dev/null; then
        ok "a distrobox container it did not make is not listed, refused by name, and left ${plain_before} by upgrade-all"
    else
        not_ok "the unlabelled container: ${plain_before:-not made} then $(plain_status)" "${out}"
    fi
    check "the security report's container check is ok for a container upgraded today" jq -e \
        '.checks[] | select(.id == "containers") | .state == "ok"' <<<"$(in_session '/usr/libexec/amethystora-security-status --json' 2>/dev/null)"

    # Apx's YAML: a stack, imported on a terminal, which shows it and asks; a package manager (model 1: each
    # command after the manager's name), refused without a terminal, then imported with --yes
    put vmlab-pkg/stack.yml <<'EOF'
name: vmlab-stack
base: quay.io/toolbx-images/alpine-toolbox:3.22
pkgmanager: apk
packages:
  - htop
builtin: false
EOF
    put vmlab-pkg/manager.yml <<'EOF'
name: vmlab-pm
model: 1
needsudo: true
cmdinstall: add
cmdremove: del
cmdpurge: del --purge
cmdsearch: search
cmdshow: info -a
cmdlist: list --installed
cmdupdate: update
cmdupgrade: upgrade
cmdautoremove: version
cmdclean: cache clean
EOF
    out="$(in_terminal 120 'Import the template vmlab-stack' 'y' -- "${AS_TESTER[@]}" 'cd && amepkg templates import vmlab-pkg/stack.yml')"
    status=$?
    shown="$(line_of "${out}" '"base": "quay.io/toolbx-images/alpine-toolbox:3.22"')"
    asked="$(line_of "${out}" 'Import the template vmlab-stack,')"
    if [[ ${status} -eq 0 && -n "${shown}" && -n "${asked}" ]] && ((shown < asked)) && grep -q "Imported the template vmlab-stack\." <<<"${out}"; then
        ok "templates import of a stack shows what it defines, asks, and keeps it once answered"
    else
        not_ok "templates import on a terminal (exit ${status})" "$(tail -n 20 <<<"${out}")"
    fi
    out="$(in_session 'amepkg managers import vmlab-pkg/manager.yml </dev/null' 2>&1)"
    if grep -q "Importing asks first, which needs a terminal, or --yes" <<<"${out}" && ! in_session 'amepkg managers show vmlab-pm' >/dev/null 2>&1 &&
        in_session 'amepkg managers import --yes vmlab-pkg/manager.yml' >/dev/null 2>&1; then
        ok "managers import keeps nothing unasked without a terminal, and --yes skips the question"
    else
        not_ok "managers import without a terminal: ${out}"
    fi
    check "the template is listed as yours, signature-checked though not pinned" jq -e 'any(.[]; .name == "vmlab-stack" and (.built_in | not)
        and .verified and (.pinned | not) and .manager == "apk" and .packages == ["htop"])' <<<"$(pkg_list templates)"
    check "the manager is listed as yours, each command after its name" jq -e 'any(.[]; .name == "vmlab-pm" and (.built_in | not)
        and .need_sudo and .install == "vmlab-pm add" and .clean == "vmlab-pm cache clean")' <<<"$(pkg_list managers)"
    in_session 'amepkg templates rm vmlab-stack --force; amepkg managers rm vmlab-pm --force' >/dev/null 2>&1

    # The virus scan comes first and stops on a detection: the EICAR test file, named as a package. clamd's
    # socket is not the account's, so amethystora-pkg loads the signatures with clamscan.
    clam_db() { compgen -G '/var/lib/clamav/main.c[lv]d' >/dev/null && compgen -G '/var/lib/clamav/daily.c[lv]d' >/dev/null; }
    if ! wait_for 600 clam_db; then
        not_ok "ClamAV has no signatures" "$(journalctl -b -u clamav-freshclam.service --no-pager -o cat | tail -n 10)"
    else
        printf '%s%s' 'X5O!P%@AP[4\PZX54(P^)7CC)7}$EICAR' '-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*' | put vmlab-pkg/vmlab-eicar.deb
        out="$(in_session 'amepkg install --yes vmlab-pkg/vmlab-eicar.deb </dev/null' 2>&1)"
        status=$?
        if [[ ${status} -ne 0 ]] && grep -q "Not installed: it matches known malware" <<<"${out}" && ! grep -q "Signature " <<<"${out}"; then
            ok "install refuses a file that matches known malware, before reading anything from it"
            touch "${STATE}/scan-works"
        else
            not_ok "the EICAR file was not stopped by the scan (exit ${status})" "$(tail -n 15 <<<"${out}")"
        fi
        rm -f "${HOME_DIR}/vmlab-pkg/vmlab-eicar.deb"
    fi
    reboot_and_continue
fi

# A .deb of its own, so that nothing on Flathub or in Debian's repositories is offered in its place: a command,
# a launcher and an install script, no dependencies (apt installs it without asking). Named otherwise, it is a
# file for what the name is. Makes ${STATE}/deb-NAME/NAME_VERSION_all.deb.
make_deb() {  # [NAME VERSION]
    local name="${1:-vmlab-hello}" version="${2:-1.0}"
    local dir="${STATE}/deb-${name}"
    rm -rf "${dir}"
    mkdir -p "${dir}/control" "${dir}/data/usr/bin" "${dir}/data/usr/share/applications" || return 1
    printf 'Package: %s\nVersion: %s\nArchitecture: all\nMaintainer: VM lab <vmlab@example.invalid>\nDescription: What the VM lab installs from a file\n' \
        "${name}" "${version}" >"${dir}/control/control"
    printf '#!/bin/sh\nexit 0\n' >"${dir}/control/postinst"
    printf '#!/bin/sh\necho %s\n' "${name}" >"${dir}/data/usr/bin/${name}"
    chmod 0755 "${dir}/control/postinst" "${dir}/data/usr/bin/${name}"
    printf '[Desktop Entry]\nType=Application\nName=VM lab %s\nExec=%s\nTerminal=true\n' "${name#vmlab-}" "${name}" \
        >"${dir}/data/usr/share/applications/${name}.desktop"
    printf '2.0\n' >"${dir}/debian-binary"
    tar -C "${dir}/control" --owner=0 --group=0 -czf "${dir}/control.tar.gz" . &&
        tar -C "${dir}/data" --owner=0 --group=0 -czf "${dir}/data.tar.gz" . &&
        (cd "${dir}" && ar rcS "${name}_${version}_all.deb" debian-binary control.tar.gz data.tar.gz)
}

if [[ "${SUITE_PHASE}" == 2 ]]; then
    if [[ ! -e "${STATE}/scan-works" ]]; then
        skip "a .deb from a file: the virus scan did not work in phase 1, and the install would stop to ask about going on without it"
    elif ! make_deb; then
        not_ok "could not build the test .deb"
    else
        put vmlab-pkg/vmlab-hello_1.0_all.deb <"${STATE}/deb-vmlab-hello/vmlab-hello_1.0_all.deb"
        began=${SECONDS}
        out="$(in_terminal 840 'Install vmlab-hello 1\.0 from this file\?' 'y' -- "${AS_TESTER[@]}" \
            'cd && amepkg install vmlab-pkg/vmlab-hello_1.0_all.deb')"
        status=$?
        echo "# the .deb, from the file to installed in a new Debian container: $((SECONDS - began)) s"
        scanned="$(line_of "${out}" 'No known malware in it.')"
        named="$(line_of "${out}" 'vmlab-hello 1.0')"
        scripts="$(line_of "${out}" 'It runs scripts of its own as it installs')"
        limit="$(line_of "${out}" 'Containers are not a sandbox')"
        asked="$(line_of "${out}" 'from this file?')"
        if [[ -n "${scanned}" && -n "${named}" ]] && ((scanned < named)); then
            ok "the file is scanned for viruses first"
        else
            not_ok "no clean virus scan before the rest (exit ${status})" "$(head -n 20 <<<"${out}")"
        fi
        if [[ -n "${named}" && -n "${scripts}" && -n "${asked}" ]] && ((named < scripts && scripts < asked)) &&
            grep -qE 'From +VM lab <vmlab@example.invalid>' <<<"${out}" && grep -qE 'Signature +Not signed, as \.deb files normally are not' <<<"${out}"; then
            ok "then what it says about itself: name, version, maker, unsigned as a .deb normally is, and its install script"
        else
            not_ok "the file's provenance was not shown before the question" "$(head -n 30 <<<"${out}")"
        fi
        if [[ -n "${limit}" && -n "${asked}" ]] && ((limit < asked)); then ok "it asks, saying first that a container is not a sandbox"; else not_ok "no word on the sandbox before the question"; fi
        if [[ ${status} -eq 0 ]] && grep -qF "There is no container for deb packages yet, so debian is made from the template debian." <<<"${out}" &&
            grep -qF "Installed vmlab-hello in debian." <<<"${out}"; then
            ok "a Debian container is made from its template for it, and the file installed there"
        else
            not_ok "the .deb was not installed (exit ${status})" "$(tail -n 25 <<<"${out}")"
        fi
        check "its launcher is in the app grid, marked (on debian)" grep -q '^Name=VM lab hello (on debian)$' "${LAUNCHERS}/pkg-debian-vmlab-hello.desktop"
        out="$(in_session 'amepkg debian run vmlab-hello' 2>/dev/null | tail -n 1)"
        if [[ "${out}" == vmlab-hello ]] && jq -e 'any(.[]; .name == "debian" and .template == "debian"
            and any(.files[]; .name == "vmlab-hello" and .version == "1.0"))' <<<"$(pkg_list containers)" >/dev/null; then
            ok "it runs in the container, which lists the file it came from"
        else
            not_ok "vmlab-hello in the Debian container: ${out:-nothing}"
        fi

        # A file for an app that is on Flathub (org.gnome.meld) and in Debian's repositories: both are offered
        # first, and picking the repositories' asks nothing about Flathub a second time. apt's own question is
        # answered no, so that nothing is downloaded.
        in_session 'amepkg debian update' >/dev/null 2>&1
        if make_deb meld 0.0.1 && put vmlab-pkg/meld_0.0.1_all.deb <"${STATE}/deb-meld/meld_0.0.1_all.deb"; then
            out="$(in_terminal 600 'Safer ways to the same software' $'\e[B\r' 'Do you want to continue' $'n\r' \
                -- "${AS_TESTER[@]}" 'cd && amepkg install vmlab-pkg/meld_0.0.1_all.deb')"
            if grep -qF 'Flathub: Meld' <<<"${out}" && grep -qF "debian's own repositories: meld" <<<"${out}" &&
                grep -qF 'Do you want to continue' <<<"${out}" && ! grep -qF 'is on Flathub as' <<<"${out}"; then
                ok "a file for an app on Flathub offers it and the repositories first, and asks about Flathub once"
            else
                not_ok "the safer ways for meld's file" "$(tail -n 25 <<<"${out}")"
            fi
        else
            not_ok "could not build or place the meld .deb"
        fi
    fi
    skip "an .rpm from a file: the same steps into a Fedora container, a second large image, with rpm -K for the signature"
    skip "the AUR: an Arch image, and paru built from source after its PKGBUILD is read, past a phase's half hour"
    reboot_and_continue
fi

# --- Phase 4: "You're on the previous version" (item 3) --------------------------------------------------
# A newer version of the image, made here from the running one with a label added, imported and deployed as
# the default without being staged: the running version is then the second of two and older than the first,
# what picking the second entry at the boot menu leaves. Not staged, since a staged one needs a restart, and
# the lab's disks lose it at shutdown: bootc install to-disk leaves /boot out of /etc/fstab, so systemd's
# gpt-auto puts the ESP's automount there and ostree cannot remount /boot to finish it. Signing in again
# stands for that boot's login.
if [[ "${SUITE_PHASE}" == 4 ]]; then
    NEWER=ostree-unverified-image:containers-storage:localhost/vmlab-newer
    stamp="${HOME_DIR}/.local/state/amethystora/previous-version"
    alert="notify-send.*previous version"
    deployments() { user_bus /usr/libexec/amethystora-deployments 2>/dev/null; }
    original="$(rpm-ostree status --json | jq -r '.deployments[] | select(.booted) | .checksum')"
    made=false
    if [[ "$(rpm-ostree status --json | jq '.deployments | length')" != 1 ]]; then
        skip "the previous version: the machine started with more than one deployment, which this would change"
    else
        began=${SECONDS}
        if out="$({ bootc image copy-to-storage &&
            printf 'FROM localhost/bootc\nLABEL org.amethystora.vmlab=newer\n' | podman build -q -t localhost/vmlab-newer -f - &&
            unshare -m sh -c 'mount -o remount,rw /sysroot && ostree container image pull /sysroot/ostree/repo "$1"' sh "${NEWER}"; } 2>&1)" &&
            commit="$(sed -n 's/^Wrote: .* => //p' <<<"${out}")" && [[ -n "${commit}" ]] &&
            printf '[origin]\ncontainer-image-reference=%s\n' "${NEWER}" >"${STATE}/newer.origin" &&
            out="$(ostree admin deploy --origin-file="${STATE}/newer.origin" "${commit}" 2>&1)"; then
            made=true
            echo "# a newer version of the image, made and deployed: $((SECONDS - began)) s"
        else
            not_ok "could not make a newer version to test with" "$(tail -n 15 <<<"${out}")"
        fi
    fi
    if [[ "${made}" == true ]]; then
        check "amethystora-deployments, read as the account, lists the newer version first, not staged, the running one second" jq -e '
            .deployments | length == 2 and (.[0].booted | not) and (.[0].staged | not) and .[1].booted
            and .[0].timestamp > .[1].timestamp' <<<"$(deployments)"
        # A login starts the alert's unit, which graphical-session.target wants for every account: started here as
        # that target starts it. Restarting GDM for a new login, as this did first, is not something a slow VM's
        # automatic login always survives.
        check "the alert starts with every account's graphical session" \
            systemctl --global is-enabled --quiet amethystora-update-alert.service
        rm -f "${stamp}"
        user_systemctl restart amethystora-update-alert.service
        if wait_for 240 pgrep -u "${TEST_USER}" -f "${alert}" && [[ -e "${stamp}" ]]; then
            ok "at login, a notification says You're on the previous version"
        else
            not_ok "no notification at login" "$(journalctl -b _SYSTEMD_USER_UNIT=amethystora-update-alert.service --no-pager -o cat | tail -n 10)"
        fi
        pkill -u "${TEST_USER}" -f "${alert}"
        # Once a boot, logouts or not: the mark is this boot's id, in the account's state, not in /run/user
        if [[ "$(cat "${stamp}" 2>/dev/null)" == "$(cat /proc/sys/kernel/random/boot_id)" ]]; then
            user_systemctl restart amethystora-update-alert.service
            if wait_for 30 pgrep -u "${TEST_USER}" -f "${alert}"; then
                not_ok "a second login in the same boot is told again"
                pkill -u "${TEST_USER}" -f "${alert}"
            else
                ok "a second login in the same boot is not told again"
            fi
        else
            not_ok "the alert's mark is not this boot's id" "$(cat "${stamp}" 2>&1)"
        fi
        skip "the notification opening System Updates, and its Keep this version: a click, then an administrator's password at pkexec"
        out="$(bootc rollback 2>&1)"
        if jq -e '.deployments[0].booted' <<<"$(deployments)" >/dev/null; then
            ok "bootc rollback, what Keep this version runs, makes the running version the default"
        else
            not_ok "bootc rollback did not make the running version the default" "${out}"
        fi
        if has_session; then
            rm -f "${stamp}"
            in_session 'timeout 60 /usr/libexec/amethystora-update-alert' >/dev/null 2>&1
            if [[ ! -e "${stamp}" ]]; then ok "after it, the alert has nothing to say"; else not_ok "the alert still says so on the default version"; fi
            pkill -u "${TEST_USER}" -f "${alert}"
        fi
        # Back as it was: the running version the default and alone, the newer one's image gone from ostree and
        # from podman
        rpm-ostree status --json | jq -e '.deployments[0].booted' >/dev/null || rpm-ostree rollback >/dev/null 2>&1
        [[ "$(rpm-ostree status --json | jq '.deployments | length')" -gt 1 ]] && rpm-ostree cleanup -r >/dev/null 2>&1
        unshare -m sh -c 'mount -o remount,rw /sysroot && ostree container image remove --repo /sysroot/ostree/repo "$1" &&
            ostree prune --repo /sysroot/ostree/repo --refs-only' sh "${NEWER}" >/dev/null 2>&1
        podman rmi -f localhost/vmlab-newer localhost/bootc >/dev/null 2>&1
        if jq -e --arg c "${original}" '(.deployments | length) == 1 and .deployments[0].booted and .deployments[0].checksum == $c' \
            <<<"$(rpm-ostree status --json)" >/dev/null &&
            ! ostree container image list --repo /sysroot/ostree/repo 2>/dev/null | grep -q vmlab-newer &&
            ! podman image exists localhost/bootc; then
            ok "the machine is left on its one deployment, the default, with no image of this suite's"
        else
            not_ok "the deployments are not as they were" "$(rpm-ostree status 2>&1 | head -n 30)"
        fi
    fi
    rm -rf "${STATE}" "${stamp}"
    finish
fi

# --- Phase 3 ---------------------------------------------------------------------------------------------

# The NVIDIA check (item 2). Here: a QEMU display on the main image, so nothing to say and nothing kept.
GPU_STATE=/var/lib/amethystora/gpu-check
INFO=/usr/share/amethystora/image-info.json
gpu_was="$(cat "${GPU_STATE}" 2>/dev/null)"
systemctl start amethystora-gpu-check.service
result="$(systemctl show -p Result --value amethystora-gpu-check.service)"
if [[ "${result}" == exec-condition && ! -e "${GPU_STATE}" ]]; then
    ok "the NVIDIA check says nothing here (no NVIDIA card, the main image), and keeps no state"
else
    not_ok "the NVIDIA check ended ${result}, state ${gpu_was:-none}" "$(journalctl -b -u amethystora-gpu-check.service --no-pager -o cat | tail -n 10)"
fi
# Elsewhere: its PCI devices and the image's flavour stood in for, in a mount namespace of its own. 99 when the
# stand-ins could not be put in place.
gpu_check() {
    local devices="$1" flavor="$2"
    shift 2
    jq --arg f "${flavor}" '."image-flavor" = $f' "${INFO}" >"${STATE}/image-info.json"
    # shellcheck disable=SC2016
    unshare -m bash -c 'mount --bind "$1" /sys/bus/pci/devices && mount --bind "$2" "$3" || exit 99
        shift 3; exec timeout 120 /usr/libexec/amethystora-gpu-check "$@"' gpu-check \
        "${STATE}/pci-${devices}" "${STATE}/image-info.json" "${INFO}" "$@"
}
# Cards by vendor and device ID: an RTX 4090 (Ada), a GTX 1080 (Pascal), and QEMU's own display
for card in nvidia:0x10de:0x2684 pascal:0x10de:0x1b80 qemu:0x1234:0x1111; do
    IFS=: read -r name vendor device <<<"${card}"
    mkdir -p "${STATE}/pci-${name}/0000:01:00.0"
    echo 0x030000 >"${STATE}/pci-${name}/0000:01:00.0/class"
    echo "${vendor}" >"${STATE}/pci-${name}/0000:01:00.0/vendor"
    echo "${device}" >"${STATE}/pci-${name}/0000:01:00.0/device"
done
offered="$(gpu_check pascal main offer)"
if [[ "${offered}" == "nvidia-lts-card amethystora-nvidia-lts" ]]; then
    ok "a GeForce 10 card is offered amethystora-nvidia-lts, NVIDIA's driver that still drives it"
else
    not_ok "a GeForce 10 card is offered: ${offered:-nothing}"
fi
rm -f "${GPU_STATE}"
check "with an NVIDIA card on the main image, it has something to say" gpu_check nvidia main pending
offer="notify-send.*This machine has an NVIDIA graphics card"
gpu_check nvidia main >/dev/null 2>&1 &
if wait_for 60 pgrep -u "${TEST_USER}" -f "${offer}"; then
    args="$(pgrep -a -u "${TEST_USER}" -f "${offer}")"
    if [[ "${args}" == *amethystora-nvidia-open* && "${args}" == *--action=switch=* && "${args}" == *--action=never=* ]]; then
        ok "it offers amethystora-nvidia-open in a notification, with Switch images… and Don't ask again"
    else
        not_ok "the offer: ${args}"
    fi
else
    not_ok "no notification offers the NVIDIA image"
fi
pkill -u "${TEST_USER}" -f "${offer}"
wait
skip "Switch images… opening ame system rebase in a terminal: a click on the notification"
gpu_check nvidia main pending
said=$?
if [[ ${said} -eq 1 && "$(cat "${GPU_STATE}" 2>/dev/null)" == nvidia-card ]]; then ok "once seen, it is not said again on this machine"; else not_ok "said again (exit ${said}, state $(cat "${GPU_STATE}" 2>/dev/null))"; fi
check "an NVIDIA image on a machine with no NVIDIA card has the reverse to say" gpu_check qemu nvidia-open pending
echo never >"${GPU_STATE}"
gpu_check nvidia main pending
said=$?
gpu_check qemu nvidia-open pending
said+=" $?"
if [[ "${said}" == "1 1" ]]; then ok "Don't ask again stops it for good"; else not_ok "after Don't ask again, pending exits ${said}"; fi
if [[ -n "${gpu_was}" ]]; then echo "${gpu_was}" >"${GPU_STATE}"; else rm -f "${GPU_STATE}"; fi

# Everything amethystora-pkg made goes, and rm asks first
out="$(in_session 'amepkg containers rm vmtest </dev/null' 2>&1)"
if grep -q "needs a terminal to confirm, or --force" <<<"${out}" && jq -e 'any(.[]; .name == "vmtest")' <<<"$(pkg_list containers)" >/dev/null; then
    ok "containers rm without a terminal or --force removes nothing"
else
    not_ok "containers rm without --force: ${out}"
fi
deb_launcher="${LAUNCHERS}/pkg-debian-vmlab-hello.desktop"
[[ -e "${deb_launcher}" ]] && had_deb_launcher=", and the Debian one's launcher with it"
in_session 'amepkg containers rm --force vmtest; amepkg containers rm --force debian; distrobox rm --force vmlab-plain' >/dev/null 2>&1
if jq -e 'length == 0' <<<"$(pkg_list containers)" >/dev/null && [[ ! -e "${deb_launcher}" ]]; then
    ok "containers rm --force removes them${had_deb_launcher:-}"
else
    not_ok "left after containers rm --force: $(pkg_list containers | jq -r 'map(.name) | join(" ")') $(ls "${deb_launcher}" 2>/dev/null)"
fi
in_session "podman rmi --ignore ${ALPINE_PULL} ${DEBIAN_PULL}" >/dev/null 2>&1
if [[ ! -e "${STATE}/had-containers" ]]; then
    in_session 'podman system reset --force' >/dev/null 2>&1
    rm -rf "${HOME_DIR}/.local/share/containers"
fi
rm -rf "${HOME_DIR}/vmlab-pkg" "${HOME_DIR}/.local/state/amethystora/pkg" "${HOME_DIR}/.local/share/amethystora/pkg" \
    "${HOME_DIR}/.cache/amethystora/pkg"
find "${HOME_DIR}/.local/share/icons" -name 'htop.*' -newer "${STATE}/started" -delete 2>/dev/null
[[ -e "${STATE}/had-mimeinfo.cache" ]] || rm -f "${LAUNCHERS}/mimeinfo.cache"
[[ -e "${STATE}/had-bin" ]] || rmdir "${HOME_DIR}/.local/bin" 2>/dev/null
if ! in_session "podman image exists ${ALPINE_PULL} || podman image exists ${DEBIAN_PULL}" >/dev/null 2>&1 &&
    [[ ! -e "${HOME_DIR}/vmlab-pkg" && ! -e "${HOME_DIR}/.local/state/amethystora/pkg" ]]; then
    ok "nothing of this suite is left in ${TEST_USER}'s home or podman"
else
    not_ok "this suite left images or files in ${TEST_USER}'s home"
fi

reboot_and_continue
