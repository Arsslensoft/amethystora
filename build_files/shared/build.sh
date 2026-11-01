#!/usr/bin/bash

set -eoux pipefail

# The rest of the build, after prepare.sh and the kernel (03-install-kernel-akmods.sh), each a step of
# its own in the Containerfile

mkdir -p /tmp/scripts/helpers
install -Dm0755 /ctx/build_files/shared/utils/ghcurl /tmp/scripts/helpers/ghcurl
export PATH="/tmp/scripts/helpers:$PATH"

# Install Additional Packages
/ctx/build_files/base/04-packages.sh

# Where a missing command can come from: Fedora's commands and their packages, while Fedora's repositories are there
/usr/bin/bash /ctx/build_files/shared/build-missing-command-index.sh

# Install Overrides and Fetch Install
/ctx/build_files/base/05-override-install.sh

# Amethystora Branding
/ctx/build_files/base/06-branding.sh

# Install the GNOME Shell extensions written for this image
/ctx/build_files/shared/build-gnome-extensions.sh

# The themes' interface fonts that Fedora does not package
/ctx/build_files/base/09-fonts.sh

# The emoji picker's lists and names, from Unicode's files
/ctx/build_files/base/09-emoji.sh

# The Amethystora icon packs' caches
/ctx/build_files/base/10-icons.sh

# The Amethystora GTK theme, recoloured from Sweet
/ctx/build_files/base/11-gtk-theme.sh

# The login screen background, patched into GNOME Shell's own theme
/ctx/build_files/base/12-login-screen.sh

# GNOME Tour's first page, which Fedora builds with Fedora's logo on it
/ctx/build_files/base/12-tour.sh

# The Amethystora apps (Manual, Security, Updates, Notes, Logs, Backups, Control): one Rust program
/ctx/build_files/base/13-apps.sh

# Amethystora Security: network protection's Suricata configuration, and the report's translations
/ctx/build_files/base/14-security.sh

# Rename the remaining Universal Blue files, commands and services to Amethystora
/ctx/build_files/base/07-debrand.sh

# Signed updates, account lockout, firewall
/ctx/build_files/base/08-hardening.sh


## late stage changes

# Systemd and Remove Items
/ctx/build_files/base/17-cleanup.sh

# Run workarounds for lf (Likely not needed)
/ctx/build_files/base/18-workarounds.sh

# Regenerate initramfs
/ctx/build_files/base/19-initramfs.sh

if [ "${IMAGE_FLAVOR}" == "dx" ] ; then
  # Now we build DX!
  /ctx/build_files/shared/build-dx.sh
fi

# What's new: the Manual's page and the live pill's card, from the notes of this build and the stream's last few
# releases (whats-new.json, which `just build` writes with release-commits.sh; without it, no notes)
/usr/bin/python3 /ctx/build_files/shared/build-whats-new.py image /ctx/build_files/whats-new.json /

# The AI agent's index, of the image as it now is
/ctx/build_files/shared/build-agent-index.sh

# Where the source of every package in the image is published, for the GPL
/ctx/build_files/shared/source-manifest.sh

# Validate all repos are disabled before committing
/ctx/build_files/shared/validate-repos.sh

# Clean Up
echo "::group:: Cleanup"
/ctx/build_files/shared/clean-stage.sh

echo "::endgroup::"

# Simple Tests
/ctx/build_files/base/20-tests.sh
