#!/usr/bin/bash
# The packages layer (layers/packages.Containerfile, plans/done/PLAN-build-layers.md): what the image's build did with
# the packages, before system_files is copied over them (build.sh)

set -eoux pipefail

mkdir -p /tmp/scripts/helpers
install -Dm0755 /ctx/build_files/shared/utils/ghcurl /tmp/scripts/helpers/ghcurl
export PATH="/tmp/scripts/helpers:$PATH"

/ctx/build_files/base/04-packages.sh

# Where a missing command can come from: Fedora's commands and their packages, while Fedora's repositories are there
/usr/bin/bash /ctx/build_files/shared/build-missing-command-index.sh

/ctx/build_files/base/05-override-install.sh

# The tools the image's build uses and does not ship: glib-compile-resources and gresource (glib2-devel, for the
# extensions' schemas, the login screen and GNOME Tour) and msgfmt (gettext, for the translations). Installed here
# once rather than in and out by each script; build.sh takes exactly what this added out again (tools.list)
mkdir -p /usr/lib/amethystora-build
rpm -qa --queryformat '%{NAME}\n' | sort -u >/tmp/before
dnf5 -y install glib2-devel gettext
rpm -qa --queryformat '%{NAME}\n' | sort -u | comm -13 /tmp/before - >/usr/lib/amethystora-build/tools.list
rm -f /tmp/before
cat /usr/lib/amethystora-build/tools.list

# The source packages of what is installed, which the image's run of source-manifest.sh then finds in place
SOURCES_FETCH_ONLY=1 /ctx/build_files/shared/source-manifest.sh

# Debranding reads every file once here; the image's run reads only the files changed since this stamp
python3 /ctx/build_files/shared/debrand.py
touch /usr/lib/amethystora-build/debranded

rm -rf /tmp/* /var/tmp/*
