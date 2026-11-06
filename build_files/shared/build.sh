#!/usr/bin/bash

set -eoux pipefail

# The image's own build, on the packages layer (plans/PLAN-build-layers.md): the base image prepared, the kernel,
# the packages and dx's packages are layers of their own (layers/), found or built by the Justfile, and so are the
# apps (mounted at /apps-stage) and the assets (at /assets). This is what changes with every commit.

mkdir -p /tmp/scripts/helpers
install -Dm0755 /ctx/build_files/shared/utils/ghcurl /tmp/scripts/helpers/ghcurl
export PATH="/tmp/scripts/helpers:$PATH"

# How long each step takes, one line each and one notice at the end, which the check run's annotations show
TIMES=()
step() {
    local start="${SECONDS}" name="$1" argument
    for argument in "$@"; do
        [[ "${argument}" == *.sh || "${argument}" == *.py ]] && { name="${argument}"; break; }
    done
    name="$(basename "${name}")"
    "$@"
    TIMES+=("${name}=$((SECONDS - start))")
    echo "build.sh: ${name} took $((SECONDS - start)) s"
}

copy_files() {
    echo "::group:: Copy Files"
    rsync -rvK /ctx/system_files/shared/ /
    echo "::endgroup::"
}
step copy_files

# Generate image-info.json
step /ctx/build_files/base/00-image-info.sh

# What the assets layer made: Sweet recoloured, the fonts and the emoji lists
step cp -a /assets/out/. /

# Amethystora Branding
step /ctx/build_files/base/06-branding.sh

# Install the GNOME Shell extensions written for this image
step /ctx/build_files/shared/build-gnome-extensions.sh

# The themes' interface fonts that Fedora does not package
step /ctx/build_files/base/09-fonts.sh install

# The emoji picker's lists and names, from Unicode's files
step /ctx/build_files/base/09-emoji.sh install

# The Amethystora icon packs' caches
step /ctx/build_files/base/10-icons.sh

# The Amethystora GTK theme, recoloured from Sweet
step /ctx/build_files/base/11-gtk-theme.sh install

# The login screen background, patched into GNOME Shell's own theme
step /ctx/build_files/base/12-login-screen.sh

# GNOME Tour's first page, which Fedora builds with Fedora's logo on it
step /ctx/build_files/base/12-tour.sh

# The Amethystora apps (Manual, Security, Updates, Notes, Logs, Backups, Control): one Rust program
step /ctx/build_files/base/13-apps.sh

# Amethystora Security: network protection's Suricata configuration, and the report's translations
step /ctx/build_files/base/14-security.sh

# The build tools the packages layer installed for the steps above (tools.list), out again once the last of them
# is done and before the initramfs is made, whose libraries' links they add: exactly what the layer added, less what
# another package needs (dx's too, which its layer installed)
remove_build_tools() {
    local tools=() keep=() count
    readarray -t tools </usr/lib/amethystora-build/tools.list
    while ((${#tools[@]})); do
        readarray -t keep < <(rpm -e --test "${tools[@]}" 2>&1 |
            sed -n 's/^[[:space:]]*\(.*\) is needed by (installed) .*$/\1/p' | sort -u |
            while read -r capability; do rpm -q --whatprovides "${capability}" --queryformat '%{NAME}\n'; done | sort -u)
        ((${#keep[@]})) || break
        count="${#tools[@]}"
        readarray -t tools < <(printf '%s\n' "${tools[@]}" | grep -vxF -f <(printf '%s\n' "${keep[@]}") || true)
        if ((${#tools[@]} == count)); then
            echo "::error::the build tools cannot be taken out: $(rpm -e --test "${tools[@]}" 2>&1 | head -n 5)"
            return 1
        fi
        echo "build.sh: keeping ${keep[*]}, which the image needs"
    done
    if ((${#tools[@]})); then
        rpm -e "${tools[@]}"
    fi
}
step remove_build_tools

# Rename the remaining Universal Blue files, commands and services to Amethystora
step /ctx/build_files/base/07-debrand.sh

# Signed updates, account lockout, firewall
step /ctx/build_files/base/08-hardening.sh


## late stage changes

# Systemd and Remove Items
step /ctx/build_files/base/17-cleanup.sh

# Run workarounds for lf (Likely not needed)
step /ctx/build_files/base/18-workarounds.sh

# Regenerate initramfs
step /ctx/build_files/base/19-initramfs.sh


if [ "${IMAGE_FLAVOR}" == "dx" ] ; then
  # Now we build DX!
  step /ctx/build_files/shared/build-dx.sh
fi


# What's new: the Manual's page and the live pill's card, from the notes of this build and the stream's last few
# releases (whats-new.json, which `just build` writes with release-commits.sh; without it, no notes)
step /usr/bin/python3 /ctx/build_files/shared/build-whats-new.py image /ctx/build_files/whats-new.json /

# The kinds of file nothing in the image opens, once every package and launcher is in place (dx's too), for Anything opens
step /usr/bin/python3 -I /ctx/build_files/shared/open-anything-types.py

# The AI agent's index, of the image as it now is
step /ctx/build_files/shared/build-agent-index.sh

# Where the source of every package in the image is published, for the GPL
step /ctx/build_files/shared/source-manifest.sh

# Validate all repos are disabled before committing
step /ctx/build_files/shared/validate-repos.sh

# Clean Up
echo "::group:: Cleanup"
step /ctx/build_files/shared/clean-stage.sh

echo "::endgroup::"

# Simple Tests
step /ctx/build_files/base/20-tests.sh

# Makes `/opt` writeable by default
# Needs to be here to make the main image build strict (no /opt there)
# This is for downstream images/stuff like k0s
rm -rf /opt && ln -s /var/opt /opt

set +x
echo "::notice::build.sh, in seconds: ${TIMES[*]}"
