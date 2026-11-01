#!/usr/bin/bash
# Turns Amethystora into the live system of the live ISO (iso/live/Containerfile). The live session
# runs from the squashed image in memory; the system it installs is the published image itself, a copy
# of which this puts in the live system so that installing needs no network, and which the installed
# machine then follows from the registry.
#
# What it adds is Fedora's: the live boot (dracut-live), the live session (livesys-scripts), and the
# installer (anaconda-live with its web interface). Nothing from anywhere else, but the sealed images'
# own installer (amethystora-install-sealed), which downloads what it installs.

set -xeuo pipefail

: "${BASE_IMAGE:?the image the ISO installs}"
: "${BASE_DIGEST:?the digest of it that the workflow checked}"
LIVE=/src/live

mkdir -p "$(realpath /root)"

# --- The system to install ----------------------------------------------------------------------------

# Into the read-only image store that containers-common lists for root (additionalimagestores), where
# the installer's ostreecontainer finds it by name. The storage settings are only for this pull. It is
# pulled by digest, the one the live system is built from, and only then given the name it installs by.
cat >/etc/containers/storage.conf <<'EOF'
[storage]
driver = "overlay"
runroot = "/run/containers/storage"
graphroot = "/usr/lib/containers/storage"
EOF
podman pull "${BASE_IMAGE%:*}@${BASE_DIGEST}"
podman tag "${BASE_IMAGE%:*}@${BASE_DIGEST}" "${BASE_IMAGE}"
rm -f /etc/containers/storage.conf

# --- The live boot and the installer ------------------------------------------------------------------

dnf -y install dracut-live livesys-scripts anaconda-live anaconda-webui \
    libblockdev-btrfs libblockdev-lvm libblockdev-dm grub2-efi-x64-cdboot \
    cryptsetup dosfstools efibootmgr mokutil

# An initramfs that can find the squashed system on the stick and lay a writable layer over it
kernel="$(find /usr/lib/modules -mindepth 1 -maxdepth 1 -printf '%f\n' | sort -V | tail -n 1)"
DRACUT_NO_XATTR=1 dracut --force --zstd --reproducible --no-hostonly \
    --add "dmsquash-live dmsquash-live-autooverlay" "/usr/lib/modules/${kernel}/initramfs.img" "${kernel}"

# Fedora's GNOME live session: logged in as liveuser, with the installer in the dash
sed -i 's/^livesys_session=.*/livesys_session=gnome/' /etc/sysconfig/livesys
systemctl enable livesys.service livesys-late.service

# The ISO boots through the image's own shim and GRUB, signed by Fedora, so Secure Boot can stay on
mkdir -p /boot/efi
cp -a /usr/lib/efi/*/*/EFI /boot/efi/
cp /boot/efi/EFI/fedora/grubx64.efi /boot/efi/EFI/BOOT/fbx64.efi

rm -f /etc/localtime
systemd-firstboot --timezone UTC

# The live session writes to memory, and what the installer unpacks goes through /var/tmp first
install -Dpm0644 "${LIVE}/var-tmp.mount" /usr/lib/systemd/system/var-tmp.mount
systemctl enable var-tmp.mount

# --- The installer's look and what it installs -------------------------------------------------------

# The same look as the installer ISO's (iso/product), laid over Anaconda as that ISO's product.img is
cp -a /src/product/. /
# ...and a profile for this os-release, whose ID is the image's own rather than the installer ISO's
install -Dpm0644 "${LIVE}/amethystora-live.conf" /etc/anaconda/profile.d/amethystora-live.conf
# What Anaconda installs, and where it points the installed machine
sed "s|@IMAGE@|${BASE_IMAGE}|g" "${LIVE}/installer.ks" >/usr/share/anaconda/interactive-defaults.ks
# ...which takes its updates signature-checked from the first one
grep -q -- "--enforce-container-sigpolicy" /usr/share/anaconda/interactive-defaults.ks
if grep "bootc switch" /usr/share/anaconda/interactive-defaults.ks | grep -v -- "--enforce-container-sigpolicy"; then
    echo "installer.ks switches the installed machine without signature checking" >&2
    exit 1
fi

# ...and keeps the image's firewall, without the ssh Anaconda adds by itself
grep -qx -- "firewall --use-system-defaults" /usr/share/anaconda/interactive-defaults.ks

# The sealed images (:stable-sealed), which Anaconda cannot install yet, have an installer of their own:
# a whole disk, the image downloaded from the registry, shim placed in front of systemd-boot. Amethystora's
# own images only: an image of one's own built on one (its template runs this too) has no sealed variant
if [[ "${BASE_IMAGE}" == ghcr.io/arsslensoft/* ]]; then
    install -Dpm0755 "${LIVE}/amethystora-install-sealed" /usr/bin/amethystora-install-sealed
    install -Dpm0644 "${LIVE}/amethystora-install-sealed.desktop" \
        /usr/share/applications/amethystora-install-sealed.desktop
fi

# Anaconda's welcome window for the live session names Fedora, as its desktop entries do: this is a
# system built on Fedora, not Fedora, and its trademark guidelines ask for its name to stay off it
# Each file changed here says so, as the GPL asks of a changed file (NOTICE names the changes too)
changed='Changed by Amethystora when its live ISO is built (iso/live/build.sh)'
{ grep -rlI --null 'Fedora' /usr/share/anaconda/gnome /usr/share/applications/liveinst.desktop 2>/dev/null || true; } |
    while IFS= read -r -d '' file; do
        sed -i 's/\bFedora\b/Amethystora/g' "${file}"
        if [[ ${file} == *.desktop || "$(head -c2 "${file}")" == '#!' ]]; then
            echo "# ${changed}" >>"${file}"
        fi
    done

# liveinst takes a release package whose release starts with 0. for a pre-release, and Fedora's
# generic-release is one (44-0.2): the web installer would show a "Pre-release" badge on a stable image
sed -i -E 's/(ANACONDA_ISFINAL=)"?false"?/\1"true"/' /usr/bin/liveinst
echo "# ${changed}" >>/usr/bin/liveinst
if grep -nE 'ANACONDA_ISFINAL="?false' /usr/bin/liveinst; then
    echo "liveinst still calls the live system a pre-release" >&2
    exit 1
fi
# ...and its last page, on every release, asks for feedback on Fedora's forum, with a QR code for it
webui=/usr/share/cockpit/anaconda-webui
zcat "${webui}/index.js.gz" | grep -c 'className:"feedback-section"' >/dev/null
{ zcat "${webui}/index.css.gz"; echo "/* ${changed} */"; echo '.feedback-section { display: none !important; }'; } |
    gzip -9n >"${webui}/index.css.gz.new"
mv "${webui}/index.css.gz.new" "${webui}/index.css.gz"

# --- The live session --------------------------------------------------------------------------------

# Nothing that would download, scan or set up a machine for good runs in a session that ends at
# shutdown: the apps come with the first boot of the installed system
systemctl disable uupd.timer flatpak-preinstall.service fwupd-refresh.timer amethystora-gpu-check.timer \
    amethystora-signed-updates.service amethystora-clamav-scan.timer amethystora-lynis-audit.timer \
    amethystora-security-watch.timer clamav-freshclam.service clamd@scan.service rpm-ostree-countme.service \
    brew-setup.service
systemctl --global disable amethystora-pkg-upgrade.timer amethystora-update-alert.service \
    amethystora-security-alert.service

# What Titanoboa makes the ISO from (the container-native ISO contract): its label and boot menu, every
# entry of which carries the image's kernel arguments from kargs.d, which a live boot does not read
install -Dpm0644 "${LIVE}/iso.yaml" /usr/lib/bootc-image-builder/iso.yaml
for karg in $(sed -n 's/^kargs = \[\(.*\)\]$/\1/p' /usr/lib/bootc/kargs.d/12-amethystora-console.toml | tr -d '",'); do
    if grep -E '^ +linux: ' "${LIVE}/iso.yaml" | grep -vqF " ${karg}"; then
        echo "iso/live/iso.yaml: a boot entry lacks ${karg}" >&2
        exit 1
    fi
done

dnf clean all
