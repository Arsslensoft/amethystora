#!/usr/bin/bash
# The packages-dx layer (layers/packages-dx.Containerfile, plans/PLAN-build-layers.md): dx's packages, on the
# packages layer, before system_files is copied over them (build.sh, build-dx.sh)

set -eoux pipefail

# shellcheck source=build_files/shared/copr-helpers.sh
source /ctx/build_files/shared/copr-helpers.sh

# Only Fedora's own repositories, as 17-cleanup.sh leaves them in the image, whose build ran dx's packages after it:
# negativo17's fedora-multimedia outranks Fedora's and would take dx's unrestricted installs
disable_third_party_repos

# The key of VSCodium's repository, which 00-dx.sh names (system_files/dx)
install -Dpm0644 -t /etc/pki/rpm-gpg /ctx/system_files/dx/etc/pki/rpm-gpg/RPM-GPG-KEY-VSCodium

/ctx/build_files/dx/00-dx.sh

SOURCES_FETCH_ONLY=1 /ctx/build_files/shared/source-manifest.sh

python3 /ctx/build_files/shared/debrand.py
touch /usr/lib/amethystora-build/debranded

rm -rf /tmp/* /var/tmp/*
