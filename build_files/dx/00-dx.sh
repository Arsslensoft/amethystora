#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -ouex pipefail

# Load secure COPR helpers
# shellcheck source=build_files/shared/copr-helpers.sh
source /ctx/build_files/shared/copr-helpers.sh

# DX packages from Fedora repos - common to all versions
FEDORA_PACKAGES=(
    7zip
    android-tools
    bcc
    bpftop
    bpftrace
    cascadia-code-fonts
    cockpit-bridge
    cockpit-machines
    cockpit-networkmanager
    cockpit-ostree
    cockpit-podman
    cockpit-selinux
    cockpit-storaged
    cockpit-system
    dbus-x11
    edk2-ovmf
    flatpak-builder
    gdb
    genisoimage
    git-subtree
    git-svn
    iotop-c
    libvirt
    libvirt-nss
    ltrace
    nicstat
    numactl
    osbuild-selinux
    perf
    podman-compose
    podman-machine
    podman-tui
    qemu
    qemu-char-spice
    qemu-device-display-virtio-gpu
    qemu-device-display-virtio-vga
    qemu-device-usb-redirect
    qemu-img
    qemu-system-x86-core
    qemu-user-binfmt
    qemu-user-static
    strace
    sysprof
    incus
    incus-agent
    lxc
    tiptop
    trace-cmd
    udica
    util-linux-script
    valgrind
    virt-manager
    virt-v2v
    virt-viewer
    wireshark
    wtype
    ydotool
)

echo "Installing ${#FEDORA_PACKAGES[@]} DX packages from Fedora repos..."
# negativo17's mkisofs claims to provide genisoimage at a higher version and would replace Fedora's;
# it mixes CDDL and GPL code, which Fedora's genisoimage (cdrkit) exists to avoid
dnf5 -y install --exclude=mkisofs,schily-libs "${FEDORA_PACKAGES[@]}"

# rocm doesn't work well on nvidia
if [[ ! "${IMAGE_NAME}" =~ nvidia ]]; then
  dnf install -y \
    rocm-hip \
    rocm-opencl \
    rocm-smi \
    rocminfo
fi

dnf config-manager addrepo --from-repofile=https://download.docker.com/linux/fedora/docker-ce.repo
sed -i "s/enabled=.*/enabled=0/g" /etc/yum.repos.d/docker-ce.repo
# Docker's RPM signing key ("Docker Release (CE rpm)"), as served on 2026-10-08
pin_repo_key /etc/yum.repos.d/docker-ce.repo 060A61C51B558A7F742B77AAC52FEB6B621E9F35
dnf -y install --enablerepo=docker-ce-stable \
    containerd.io \
    docker-buildx-plugin \
    docker-ce \
    docker-ce-cli \
    docker-compose-plugin \
    docker-model-plugin

# VSCodium, the MIT-licensed build of VS Code's source, in place of Microsoft's VS Code, whose licence
# does not allow it to be shared inside a published image. Its repository's key is pinned in
# system_files/dx (fingerprint 1302DE60231889FE1EBACADC54678CF75A278D9C) rather than fetched from GitLab,
# which puts the key behind a browser challenge.
tee /etc/yum.repos.d/vscodium.repo <<'EOF'
[vscodium]
name=VSCodium
baseurl=https://download.vscodium.com/rpms/
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-VSCodium
EOF
sed -i "s/enabled=.*/enabled=0/g" /etc/yum.repos.d/vscodium.repo
dnf -y install --enablerepo=vscodium \
    codium


# DX packages to exclude - common to all versions
EXCLUDED_PACKAGES=()

# Version-specific package exclusions for DX
case "$FEDORA_MAJOR_VERSION" in
    43)
        EXCLUDED_PACKAGES+=(mozilla-fira-mono-fonts)
        ;;
esac

# Remove excluded packages if they are installed
if [[ "${#EXCLUDED_PACKAGES[@]}" -gt 0 ]]; then
    readarray -t INSTALLED_EXCLUDED < <(rpm -qa --queryformat='%{NAME}\n' "${EXCLUDED_PACKAGES[@]}" 2>/dev/null || true)
    if [[ "${#INSTALLED_EXCLUDED[@]}" -gt 0 ]]; then
        dnf5 -y remove "${INSTALLED_EXCLUDED[@]}"
    else
        echo "No excluded packages found to remove."
    fi
fi

# docker.socket, podman.socket and libvirt-workaround.service, which comes from system_files/dx, are enabled by
# build-dx.sh: this runs in the packages-dx layer (plans/done/PLAN-build-layers.md)

# Disable RPM Fusion repos
for i in /etc/yum.repos.d/rpmfusion-*.repo; do
    if [[ -f "$i" ]]; then
        sed -i 's@enabled=1@enabled=0@g' "$i"
    fi
done

echo "::endgroup::"
