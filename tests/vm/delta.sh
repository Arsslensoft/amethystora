#!/usr/bin/bash
# delta.sh BASE TAG CTX [APPS]: in the lab, an image of the source tree at CTX built FROM BASE (the image a kept VM
# runs) without a whole build: its system_files copied over, and only the build steps those files feed run again
# (live.sh delta, .claude/skills/run-tests/live-fix.md). CTX/.live-changed lists the paths that changed since BASE's
# commit. APPS is the apps stage of a build of the same tree, for apps/; without it the apps stay BASE's. The result
# is localhost/amethystora-delta:TAG, pushed to the lab's registry (localhost:5000), from which the guest switches.
set -euo pipefail

BASE="$1" TAG="$2" CTX="$3" APPS="${4:-}"
changed() { grep -qE "$1" "${CTX}/.live-changed"; }
steps=()
changed '^system_files/.*/(gnome-shell/extensions|glib-2\.0/schemas)/|^build_files/shared/build-gnome-extensions\.sh' &&
    steps+=(build_files/shared/build-gnome-extensions.sh)
changed '^build_files/(base/09-emoji\.sh|shared/emoji-data\.py)' && steps+=(build_files/base/09-emoji.sh)
changed '^build_files/base/12-login-screen\.sh|amethystora-login-background|backgrounds/amethystora/amethystora-login' &&
    steps+=(build_files/base/12-login-screen.sh)
if changed '^apps/' && [[ -z "${APPS}" ]]; then echo "delta: apps/ changed, but no apps stage was given: the apps stay the base's" >&2; fi
[[ -z "${APPS}" ]] || steps+=(build_files/base/13-apps.sh)
changed '^po/|^build_files/base/14-security\.sh|^build_files/shared/ips-config\.py' && steps+=(build_files/base/14-security.sh)
# The agent's index is of the image as it is: anything in system_files may change it
changed '^system_files/' && steps+=(build_files/shared/build-agent-index.sh)
# A tree may bring a script of its own for steps the list above does not know (packages, hardening): run it first
[[ -f "${CTX}/.live-extra.sh" ]] && steps=(.live-extra.sh "${steps[@]}")
echo "delta: steps: ${steps[*]:-none}"

name="live-delta-${TAG}"
podman rm -f "${name}" >/dev/null 2>&1 || true
mounts=(-v "${CTX}:/ctx:ro")
[[ -z "${APPS}" ]] || mounts+=(--mount "type=image,src=${APPS},dst=/apps-stage")
# As the Containerfile's base stage does it: system_files over the image (dx's too when it is a dx image), then
# the steps, then a clean /var and /tmp, as `bootc container lint` wants them
podman run --name "${name}" "${mounts[@]}" --entrypoint /usr/bin/bash "${BASE}" -c '
    set -euo pipefail
    chmod 1777 /tmp; mkdir -p -m 1777 /var/tmp
    cp -a --no-preserve=ownership /ctx/system_files/shared/. /
    if grep -qs "\"amethystora-dx\"" /usr/share/amethystora/image-info.json /usr/share/ublue-os/image-info.json; then
        cp -a --no-preserve=ownership /ctx/system_files/dx/. /
    fi
    install -Dpm0644 -t /usr/share/licenses/amethystora /ctx/LICENSE /ctx/NOTICE
    for step in "$@"; do
        echo "== ${step}"
        started=${SECONDS}
        bash "/ctx/${step}" >"/tmp/$(basename "${step}").log" 2>&1 || { tail -n 30 "/tmp/$(basename "${step}").log"; exit 1; }
        echo "   $((SECONDS - started)) s"
    done
    test ! -s /etc/xdg/autostart/libcanberra-login-sound.desktop || rm -f /usr/share/gnome/autostart/libcanberra-login-sound.desktop
    glib-compile-schemas /usr/share/glib-2.0/schemas
    dnf5 clean all >/dev/null 2>&1 || true
    rm -rf /tmp/* /var/tmp/* /var/cache/* /var/log/* /var/lib/dnf
' delta "${steps[@]}"
# The base's own command and entrypoint, not the bash this ran with
cmd="$(podman image inspect -f '{{json .Config.Cmd}}' "${BASE}")"
entrypoint="$(podman image inspect -f '{{json .Config.Entrypoint}}' "${BASE}")"
podman commit -q --change "CMD ${cmd/#null/[]}" --change "ENTRYPOINT ${entrypoint/#null/[]}" "${name}" \
    "localhost/amethystora-delta:${TAG}" >/dev/null
podman rm -f "${name}" >/dev/null
# The lab's registry, which the guests reach at 10.0.2.2:5000 (QEMU's user network)
if ! podman container exists live-registry; then
    podman run -d --name live-registry -p 5000:5000 docker.io/library/registry:2 >/dev/null
fi
podman start live-registry >/dev/null
podman push -q --tls-verify=false "localhost/amethystora-delta:${TAG}" "localhost:5000/amethystora-delta:${TAG}"
echo "delta: localhost:5000/amethystora-delta:${TAG}, from ${BASE}"
