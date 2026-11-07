#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# Rename everything the upstream layers (the ublue-os base image, ublue-os/brew and the ublue COPR
# packages) ship as Universal Blue to Amethystora: paths, commands, services, settings and text.
# Amethystora's own files in system_files already use the final names and win over upstream ones. The packages
# layers read every file of the packages once (plans/done/PLAN-build-layers.md): this reads only what came after them,
# system_files and brew's files among it, though it renames and retargets every name
python3 /ctx/build_files/shared/debrand.py --newer /usr/lib/amethystora-build/debranded

# Rebuild the compiled settings and caches from the renamed sources
glib-compile-schemas /usr/share/glib-2.0/schemas
if command -v dconf >/dev/null; then
    dconf update
fi
if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache -f /usr/share/icons/hicolor
fi

echo "::endgroup::"
