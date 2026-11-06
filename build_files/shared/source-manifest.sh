#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# Where the source of everything in the image is, which the GPL and LGPL ask whoever distributes a
# program to say: every package with its licence, the source package it was built from, and where
# that exact source package is. Fedora keeps the source of each of its builds in Koji, at an address
# the name, version and release give. The other makers (RPM Fusion, negativo17, Fedora Copr) publish
# only their current build of each package, so the source packages of theirs under a licence that asks
# for the source are fetched here and travel in the image, in /usr/src/amethystora. For the rest, the
# package names its maker and the address its source is published at. NOTICE carries the written
# offer that covers all of it.
# Runs after dx, so that it lists every package the image ships.

MANIFEST=/usr/share/licenses/amethystora/SOURCES
SOURCES_DIR=/usr/src/amethystora
FEDORA="$(rpm -E %fedora)"
# shellcheck source=/dev/null
source /usr/lib/os-release

# The licences that ask for the source: the GPL family, the MPL, the EPL and the CDDL
COPYLEFT='GPL|MPL|EPL|CDDL'
RPMFUSION=https://download1.rpmfusion.org
NEGATIVO17=https://negativo17.org/repos
COPR=https://download.copr.fedorainfracloud.org/results
# A package built in Copr names only the owner as its vendor, so the projects this image and its base
# install from are listed here. One that is missing fails the build below, to be added.
COPR_PROJECTS=(ublue-os/packages ublue-os/staging ublue-os/akmods bieszczaders/kernel-cachyos-addons)
# NVIDIA's repository for its container toolkit, on the NVIDIA images, carries no source packages at
# all: the source of each release is the tag of its version in the repository the package names.
NO_SOURCE_PACKAGES="NVIDIA CORPORATION"
# Source packages never fetched: xone's common package is built from Microsoft's Windows driver packages, which
# its source package carries and no licence lets anyone pass on. What in it is under the GPL, a modprobe line, a
# README and the licence, is at the address SOURCES names, and the image does not ship the firmware it made.
NOT_FETCHED='^xone-kmod-common-'

fetch() {
    curl --fail --silent --location --retry 3 --output "${SOURCES_DIR}/$1" "$2/$1" ||
        { rm -f "${SOURCES_DIR}/$1" && return 1; }
}

# fetch_source <vendor> <source package>, from where that vendor publishes its source packages. One
# with no vendor was built outside any repository, as the kernel modules are, from a source package
# that RPM Fusion, negativo17 or Copr may publish all the same.
fetch_source() {
    local vendor="$1" srpm="$2" letter="${2:0:1}" base project primary location akmod
    [[ ${srpm} =~ ${NOT_FETCHED} ]] && return 1
    # Universal Blue's akmods build kmod-NAME from the akmod its maker publishes, whose source package is
    # NAME-kmod of the same version: fetched under that name, and named by its kmod's beside it
    if [[ ${vendor} == "(none)" && ${srpm} =~ ^kmod-(.+)-([^-]+-[^-]+)\.src\.rpm$ ]]; then
        akmod="${BASH_REMATCH[1]}-kmod-${BASH_REMATCH[2]}.src.rpm"
        if [[ -s "${SOURCES_DIR}/${akmod}" ]] || fetch_source "Fedora Copr" "${akmod}" ||
            fetch_source negativo17.org "${akmod}" || fetch_source "RPM Fusion" "${akmod}"; then
            ln -sf "${akmod}" "${SOURCES_DIR}/${srpm}"
            return
        fi
    fi
    if [[ ${vendor} == "RPM Fusion" || ${vendor} == "(none)" ]]; then
        # A Fedora not yet released (beta) is under development/ until its release
        for base in "${RPMFUSION}"/{free,nonfree}/fedora/{updates/"${FEDORA}"/SRPMS,{releases,development}/"${FEDORA}"/Everything/source/SRPMS}; do
            if fetch "${srpm}" "${base}/${letter,,}"; then return; fi
        done
    fi
    if [[ ${vendor} == "negativo17.org" || ${vendor} == "(none)" ]]; then
        # nvidia-580 is NVIDIA's long-term branch, which the -nvidia-lts images carry
        for base in "${NEGATIVO17}"/{multimedia,nvidia,nvidia-580}/fedora-"${FEDORA}"/SRPMS; do
            if fetch "${srpm}" "${base}"; then return; fi
        done
    fi
    if [[ ${vendor} == "Fedora Copr"* ]]; then
        for project in "${COPR_PROJECTS[@]}"; do
            base="${COPR}/${project}/fedora-${FEDORA}-$(uname -m)"
            # The repository's index says which build's folder holds the source package
            primary="$(curl --fail --silent --location --retry 3 "${base}/repodata/repomd.xml" |
                grep -oE 'repodata/[^"]*-primary\.xml\.gz' | sort -u)" || continue
            location="$(curl --fail --silent --location --retry 3 "${base}/${primary}" | gunzip -c |
                grep -oE 'href="[^"]+\.src\.rpm"' | cut -d '"' -f 2 | grep -F "/${srpm}" | tail -n 1)" || continue
            if fetch "${srpm}" "${base}/${location%/*}"; then return; fi
        done
    fi
    return 1
}

# In a file, not a variable: the build runs with xtrace, which would print all of it
PACKAGES=/tmp/packages.tsv
rpm -qa --queryformat '%{NAME}\t%{LICENSE}\t%{SOURCERPM}\t%{VENDOR}\t%{URL}\n' |
    awk -F'\t' '$3 != "(none)"' | sort >"${PACKAGES}"

# The GPL and LGPL ask for the very source a binary was built from, so one of theirs that its maker has
# stopped publishing fails the build: the base image is then older than that maker's repository, and
# the next base image has the current build. The other licences are met by saying where the source is,
# which SOURCES does either way, and so is a package no repository publishes a source package for.
# One the image carries already is not fetched again: the sealed images run this a second time, on
# packages its maker may have replaced since.
mkdir -p "${SOURCES_DIR}"
while IFS=$'\t' read -r vendor srpm family; do
    if [[ -s "${SOURCES_DIR}/${srpm}" ]] || fetch_source "${vendor}" "${srpm}"; then
        continue
    fi
    if [[ ${vendor} != "(none)" && ${vendor} != "${NO_SOURCE_PACKAGES}" && ${family} == gpl && ! ${srpm} =~ ${NOT_FETCHED} ]]; then
        echo "::error::${srpm} is under the GPL, and ${vendor} no longer publishes it"
        exit 1
    fi
    echo "::warning::no source package is published for ${srpm}; SOURCES names where its source is"
done < <(awk -F'\t' -v copyleft="${COPYLEFT}" '$4 != "Fedora Project" && $2 ~ copyleft {
    print $4 "\t" $3 "\t" ($2 ~ /GPL/ ? "gpl" : "other") }' "${PACKAGES}" | sort -u)

# The packages layers fetch the source packages of what they installed, and leave SOURCES to the image's run
# (plans/PLAN-build-layers.md)
if [[ -n "${SOURCES_FETCH_ONLY:-}" ]]; then
    rm -f "${PACKAGES}"
    echo "::endgroup::"
    exit 0
fi

{
    cat <<EOF
# Sources of ${PRETTY_NAME}

Each package in this image, with its licence, the source package it was built from, and where that
source package is: an address in Fedora's build system for what Fedora built, a file in
${SOURCES_DIR} for the source packages this image carries, and otherwise the package's maker and the
address its source is published at. Everything not installed from a package is built from
https://github.com/arsslensoft/amethystora at commit ${BUILD_ID:-unknown}, whose build_files pin each
upstream release they fetch. The written offer for the source of anything under a licence that asks
for it is in NOTICE, beside this file.

EOF
    printf 'PACKAGE\tLICENCE\tSOURCE PACKAGE\tSOURCE\n'
    awk -F'\t' -v OFS='\t' -v sources="${SOURCES_DIR}" '{
        source = ($4 == "(none)" ? "" : $4) ($4 == "(none)" || $5 == "(none)" ? "" : ", ") ($5 == "(none)" ? "" : $5)
        if ($4 == "Fedora Project") {
            nvr = $3
            sub(/\.src\.rpm$/, "", nvr)
            n = split(nvr, part, "-")
            name = substr(nvr, 1, length(nvr) - length(part[n - 1]) - length(part[n]) - 2)
            source = "https://kojipkgs.fedoraproject.org/packages/" name "/" part[n - 1] "/" part[n] "/src/" $3
        } else if (system("test -s \"" sources "/" $3 "\"") == 0) {
            source = sources "/" $3
        }
        print $1, $2, $3, source
    }' "${PACKAGES}"
} >"${MANIFEST}"
rm -f "${PACKAGES}"

grep -qP '^kernel-core\t' "${MANIFEST}"

echo "::endgroup::"
