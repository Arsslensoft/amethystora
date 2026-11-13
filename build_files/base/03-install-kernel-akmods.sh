#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# shellcheck source=build_files/shared/copr-helpers.sh
source /ctx/build_files/shared/copr-helpers.sh

# Beta Updates Testing Repo...
if [[ "${UBLUE_IMAGE_TAG}" == "beta" ]]; then
    dnf5 config-manager setopt updates-testing.enabled=1
fi

# Remove Existing Kernel
for pkg in kernel kernel-core kernel-modules kernel-modules-core kernel-modules-extra; do
    rpm --erase $pkg --nodeps
done

# Fetch Common AKMODS & Kernel RPMS, by the digest the Justfile resolved from
# akmods:${AKMODS_FLAVOR}-<fedora>-${KERNEL} and checked against Universal Blue's key
skopeo copy --retry-times 3 docker://ghcr.io/ublue-os/akmods@"${AKMODS_DIGEST:?checked by the Justfile}" dir:/tmp/akmods
AKMODS_TARGZ=$(jq -r '.layers[].digest' </tmp/akmods/manifest.json | cut -d : -f 2)
tar -xvzf /tmp/akmods/"$AKMODS_TARGZ" -C /tmp/
mv /tmp/rpms/* /tmp/akmods/
# NOTE: kernel-rpms should auto-extract into correct location

# Install Kernel
dnf5 -y install \
    /tmp/kernel-rpms/kernel-[0-9]*.rpm \
    /tmp/kernel-rpms/kernel-core-*.rpm \
    /tmp/kernel-rpms/kernel-modules-*.rpm

# TODO: Figure out why akmods cache is pulling in akmods/kernel-devel
dnf5 -y install \
    /tmp/kernel-rpms/kernel-devel-*.rpm

dnf5 versionlock add kernel kernel-devel kernel-devel-matched kernel-core kernel-modules kernel-modules-core kernel-modules-extra

# Everyone
# NOTE: we won't use dnf5 copr plugin for ublue-os/akmods until our upstream provides the COPR standard naming
sed -i 's@enabled=0@enabled=1@g' /etc/yum.repos.d/_copr_ublue-os-akmods.repo
# Its signing key, as the Copr project served it on 2026-10-08
pin_repo_key /etc/yum.repos.d/_copr_ublue-os-akmods.repo 19E98556EE61C8EF1FABFEF88F7CFDB74F8B54DD

# RPMFUSION Dependent AKMODS
if [[ "${UBLUE_IMAGE_TAG}" == "beta" ]]; then
    rpmfusion_release_install free || true
    rpmfusion_release_install nonfree || true
    readarray -t RPMFUSION_MASTER < <(rpmfusion_master)
    dnf5 -y install "${RPMFUSION_MASTER[@]}" \
        v4l2loopback /tmp/akmods/kmods/*v4l2loopback*.rpm || true
    dnf5 -y remove rpmfusion-free-release || true
    dnf5 -y remove rpmfusion-nonfree-release || true
else
    rpmfusion_release_install free nonfree
    readarray -t RPMFUSION_MASTER < <(rpmfusion_master)
    dnf5 -y install "${RPMFUSION_MASTER[@]}" \
        v4l2loopback /tmp/akmods/kmods/*v4l2loopback*.rpm
    dnf5 -y remove rpmfusion-free-release rpmfusion-nonfree-release
fi

# Xbox controllers: xpadneo over Bluetooth (rumble, battery, the right mappings), and xone for Microsoft's
# wireless adapter and the headsets on it; wired ones are the kernel's own xpad. Built for this kernel and
# signed with Universal Blue's key, like v4l2loopback. The tarball also carries wl and openrazer, which stay
# out (plans/done/PLAN-driver-compatibility.md, "Not doing").
dnf5 -y install \
    /tmp/akmods/kmods/kmod-xpadneo-*.rpm /tmp/akmods/common/xpadneo-kmod-common-*.rpm \
    /tmp/akmods/kmods/kmod-xone-*.rpm /tmp/akmods/common/xone-kmod-common-*.rpm
# xone's common package carries Microsoft's firmware for the adapter, which no licence lets anyone pass on, under
# the names an older xone asked for: `ame apps xbox-adapter on` fetches it from Microsoft on the machine instead.
# Its blacklist of mt76x2u, which would take Wi-Fi from every MT7612U USB stick, is copied into /etc by the same
# command only, from here.
rm -f /usr/lib/firmware/xow_dongle*.bin
install -Dpm0644 /usr/lib/modprobe.d/xone.conf /usr/share/amethystora/xone/xone.conf
rm -f /usr/lib/modprobe.d/xone.conf

# DisplayLink (USB docks and adapters): the prebuilt evdi kmod is only published in akmods-extra, which is
# not built for the main kernel. Build it here against the image kernel from negativo17's
# akmod, the same way akmods-extra does, then drop the build tooling.
KERNEL_VERSION="$(rpm -q kernel-core --queryformat '%{VERSION}-%{RELEASE}.%{ARCH}')"
dnf5 -y install akmods
dnf5 -y mark dependency akmods
# Without scriptlets: akmod-evdi's %post (akmods-ostree-post, run because os-release has OSTREE_VERSION) builds
# as root, which akmodsbuild refuses when /var is writable as it is here, and fails the transaction.
# akmods below builds as its own user instead. displaylink requires the kmod, so it goes in the same transaction.
dnf5 -y install --enablerepo=fedora-multimedia --setopt=tsflags=noscripts \
    akmod-evdi \
    displaylink \
    libevdi
# Secure Boot only loads evdi when it is signed with a key enrolled in MOK (ame security secure-boot). The signing key
# is not in this step, which runs third-party packages and scripts with the network on: a key they could read would
# sign kernel code for every machine that enrolled its certificate. akmods builds evdi unsigned (kmodtool's own
# switch, off for this build only) and 03-sign-evdi.sh, a step of its own with no network, signs the result
# (Containerfile).
printf '%%_kmodtool_signmodules 0\n' | install -Dpm0644 /dev/stdin /etc/rpm/macros.zz-amethystora-nosign
CFLAGS="-fno-pie -no-pie" akmods --force --kernels "${KERNEL_VERSION}" --kmod evdi
rm -f /etc/rpm/macros.zz-amethystora-nosign
EVDI_KO="/usr/lib/modules/${KERNEL_VERSION}/extra/evdi/evdi.ko.xz"
modinfo "${EVDI_KO}" >/dev/null ||
    { find /var/cache/akmods/evdi/ -name '*.log' -print -exec cat {} \; && exit 1; }
dnf5 -y remove akmod-evdi
# Any key akmods made on its own: never ship it
AKMODS_CERT=/etc/pki/akmods/certs/amethystora-modules.der
find /etc/pki/akmods -type f -o -type l 2>/dev/null | while read -r file; do
    [[ "${file}" == "${AKMODS_CERT}" ]] || rpm -qf "${file}" >/dev/null 2>&1 || rm -f "${file}"
done
# DisplayLinkManager starts when a DisplayLink device is there, at boot as well as on plug, and stops when it
# goes: the package's udev rule matches DisplayLink's own USB vendor id (17e9), which rebranded docks keep.
# It is not enabled, as the package's preset (skipped above with the scriptlets) would have it: from boot, it
# ran on every machine, dock or not.
systemctl disable displaylink.service

# Nvidia AKMODS: akmods-nvidia-open for the -nvidia-open images, akmods-nvidia-lts (NVIDIA's closed driver of
# its 580 branch, for Maxwell, Pascal and Volta) for the -nvidia-lts ones, each with the same layout
if [[ "${IMAGE_NAME}" =~ nvidia ]]; then
    # Fetch Nvidia RPMs
    skopeo copy --retry-times 3 docker://ghcr.io/ublue-os/akmods-nvidia-"${IMAGE_NAME##*-nvidia-}"@"${AKMODS_NVIDIA_DIGEST:?checked by the Justfile}" dir:/tmp/akmods-rpms
    NVIDIA_TARGZ=$(jq -r '.layers[].digest' </tmp/akmods-rpms/manifest.json | cut -d : -f 2)
    tar -xvzf /tmp/akmods-rpms/"$NVIDIA_TARGZ" -C /tmp/
    mv /tmp/rpms/* /tmp/akmods-rpms/

    # Exclude the Golang Nvidia Container Toolkit in Fedora Repo
    # Exclude for non-beta.... doesn't appear to exist for F42 yet?
    if [[ "${UBLUE_IMAGE_TAG}" != "beta" ]]; then
        dnf5 config-manager setopt excludepkgs=golang-github-nvidia-container-toolkit
    else
        # Monkey patch right now...
        if ! grep -q negativo17 <(rpm -qi mesa-dri-drivers); then
            dnf5 -y swap --repo=updates-testing \
                mesa-dri-drivers mesa-dri-drivers
        fi
    fi

    # Install Nvidia RPMs
    IMAGE_NAME="${BASE_IMAGE_NAME}" AKMODNV_PATH="/tmp/akmods-rpms" MULTILIB=0 /tmp/akmods-rpms/ublue-os/nvidia-install.sh
    rm -f /usr/share/vulkan/icd.d/nouveau_icd.*.json
    ln -sf libnvidia-ml.so.1 /usr/lib64/libnvidia-ml.so
    # Where NVIDIA's driver is not loaded (no NVIDIA card, a card this flavor's driver does not drive, Secure Boot
    # refusing the module), these restarted until systemd gave up, and the machine started degraded: skipped then
    for unit in nvidia-persistenced.service nvidia-cdi-refresh.service; do
        mkdir -p "/usr/lib/systemd/system/${unit}.d"
        printf '[Unit]\nConditionPathExists=/sys/module/nvidia\n' >"/usr/lib/systemd/system/${unit}.d/10-amethystora-driver.conf"
    done
    mkdir -p /usr/lib/bootc/kargs.d
    tee /usr/lib/bootc/kargs.d/00-nvidia.toml <<EOF
kargs = ["rd.driver.blacklist=nouveau", "modprobe.blacklist=nouveau", "nvidia-drm.modeset=1", "initcall_blacklist=simpledrm_platform_driver_init"]
EOF
fi

# This is a step of its own (Containerfile): what it unpacked stays out of its layer
rm -rf /tmp/akmods /tmp/akmods-rpms /tmp/kernel-rpms /tmp/rpms

echo "::endgroup::"
