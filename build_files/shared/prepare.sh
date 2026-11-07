#!/usr/bin/bash
# The first step of the kernel layer (layers/kernel.Containerfile, plans/done/PLAN-build-layers.md): what comes before
# the kernel. The kernel and its modules are a step of their own, the only one the module signing key is mounted
# for. system_files is copied in by build.sh, over the packages.

set -eoux pipefail

echo "::group:: Prepare"

# Speeds up local builds
dnf config-manager setopt keepcache=1

# The base image's ujust, setup, signing, udev and update packages. system_files carries the image's own
# versions of the same files, which a later `dnf remove` would take away with the packages
dnf remove -y ublue-os-luks ublue-os-just ublue-os-udev-rules ublue-os-signing ublue-os-update-services
# They leave the folders they did not own, which 07-debrand.sh cannot move out of the base image's layer
find /usr/share/ublue-os -depth -type d -empty -delete

# Keep *-logos in RPM DB for downstream package installations
# We are not allowed to ship an empty fedora-logos package
dnf -y swap fedora-logos generic-logos
rpm --erase --nodeps --nodb generic-logos
# httpd's test page logos too: Fedora's trademark guidelines keep its logos out of a remix, and
# generic-logos-httpd provides the same system-logos-httpd that httpd requires
if rpm -q fedora-logos-httpd >/dev/null; then
    dnf -y swap fedora-logos-httpd generic-logos-httpd
fi
# The release packages go the same way: the guidelines name fedora-release beside fedora-logos, and
# Fedora ships generic-release to stand in for it. Only the package database changes hands. The files
# stay where they are: os-release is rewritten for this image by 00-image-info.sh, which also takes
# Fedora's name out of the rest, and the presets and the dist macros that came with Fedora's packages
# are what the system is built on. Their rpm-ostree polkit rules are replaced by the image's own, from
# system_files, which build.sh copies over them.
readarray -t FEDORA_RELEASE < <(rpm -qa --queryformat '%{NAME}\n' 'fedora-release*')
((${#FEDORA_RELEASE[@]}))
mkdir -p /tmp/generic-release
dnf -y download --destdir=/tmp/generic-release generic-release generic-release-common
rpm --erase --justdb --nodeps --noscripts --notriggers "${FEDORA_RELEASE[@]}"
rpm --install --justdb --nodeps --noscripts --notriggers /tmp/generic-release/generic-release-*.rpm
rm -rf /tmp/generic-release
# What needs a release package still finds one, and dnf still knows which Fedora this is built on
rpm -q --whatprovides system-release "system-release($(rpm -E %fedora))"

echo "::endgroup::"
