#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# Revert back to upstream defaults
dnf config-manager setopt keepcache=0
dnf versionlock clear

# This comes last because we can't *ever* afford to ship fedora flatpaks on the image
systemctl disable flatpak-add-fedora-repos.service
systemctl mask flatpak-add-fedora-repos.service
rm -f /usr/lib/systemd/system/flatpak-add-fedora-repos.service

rm -rf /.gitkeep
# What the packages layers left for the image's build (layers/packages.sh): the build tools' list, the debranding stamp
rm -rf /usr/lib/amethystora-build
find /var/* -maxdepth 0 -type d \! -name cache -exec rm -fr {} \;
find /var/cache/* -maxdepth 0 -type d \! -name libdnf5 \! -name rpm-ostree -exec rm -fr {} \;
rm -rf /tmp && mkdir -p -m 1777 /tmp
rm -rf /boot && mkdir -p /boot

echo "::endgroup::"
