#!/usr/bin/bash
# What a sealed image has to be before it is published (Containerfile.sealed). /run/target is the tree
# the digest covers, /run/uki the signed UKI, /run/kernel the kernel and initramfs inside it.

echo "::group:: ===$(basename "$0")==="

set -xeuo pipefail

T=/run/target
CERT=/tmp/amethystora-uki.pem
openssl x509 -inform DER -in "${T}/usr/share/amethystora/secure-boot/amethystora-uki.der" -out "${CERT}"
# rpm will not open a database on a read-only mount. The folder's contents, since the image may have it
# as a link to /usr/share/rpm
mkdir /tmp/rpmdb
cp -a "${T}/usr/lib/sysimage/rpm/." /tmp/rpmdb/
RPM=(rpm --dbpath /tmp/rpmdb)

# One UKI, named for its kernel and signed with the certificate the image ships, the one the installer
# and `ame security secure-boot` enrol
readarray -t UKIS < <(find /run/uki -maxdepth 1 -name '*.efi')
[[ ${#UKIS[@]} -eq 1 ]]
UKI="${UKIS[0]}"
KERNEL="$(basename "${UKI}" .efi)"
[[ -d "/run/kernel/${KERNEL}" ]]
sbverify --cert "${CERT}" "${UKI}"

# Its command line carries the digest of exactly this tree, which is what the machine checks at boot, and
# requires it ("composefs=?" would make fs-verity optional and the image unsealed). The image's own bootc
# wrote it (10-uki.sh). From 1.17 it names the digest's format, composefs.digest=v1-sha512-12:<digest>,
# and follows it with a bare composefs= of the older V2 digest for older clients: both are checked, each
# against its own digest. Before 1.17 there is only the bare composefs=, and no --erofs-version.
# amethystora-sealed-verity.service starts on either one
CMDLINE="$(ukify inspect "${UKI}" --json=short | jq -r '.".cmdline".text')"
BOOTC_VERSION="$("${RPM[@]}" -q --queryformat '%{VERSION}' bootc)"
if [[ "$(printf '%s\n' 1.17.0 "${BOOTC_VERSION}" | sort -V | head -n 1)" == 1.17.0 ]]; then
    DIGEST_V1="$(bootc container compute-composefs-digest --erofs-version=v1 "${T}")"
    DIGEST_V2="$(bootc container compute-composefs-digest --erofs-version=v2 "${T}")"
    [[ " ${CMDLINE} " == *" composefs.digest=v1-sha512-12:${DIGEST_V1} "* ]]
    [[ " ${CMDLINE} " == *" composefs=${DIGEST_V2} "* ]]
else
    DIGEST="$(bootc container compute-composefs-digest "${T}")"
    [[ " ${CMDLINE} " == *" composefs=${DIGEST} "* ]]
fi
[[ " ${CMDLINE} " != *" composefs="\?* && " ${CMDLINE} " != *" composefs.digest="\?* ]]

# ...and the kernel arguments of kargs.d: read-write btrfs root, the splash, no credentials from the
# firmware or the ESP, an initramfs that halts when it fails rather than going on with the boot, the
# hardening, no console size query, and lockdown on every image but the NVIDIA ones, as 08-hardening.sh
# sets it. No debug shell: nothing can be added to
# this command line on the machine, and a shell there would be root's, with the disk already unlocked.
for karg in rw rootflags=compress=zstd:1,subvol=root quiet rhgb systemd.import_credentials=no rd.shell=0 \
    rd.emergency=halt slab_nomerge init_on_alloc=1 page_alloc.shuffle=1 randomize_kstack_offset=on vsyscall=none \
    proc_mem.force_override=ptrace systemd.tty.term.console=dumb; do
    [[ " ${CMDLINE} " == *" ${karg} "* ]]
done
if [[ "$(jq -r '."image-flavor"' "${T}/usr/share/amethystora/image-info.json")" == nvidia-* ]]; then
    [[ " ${CMDLINE} " != *" lockdown="* ]]
else
    [[ " ${CMDLINE} " == *" lockdown=integrity "* ]]
fi
[[ "${CMDLINE}" != *debug_shell* && " ${CMDLINE} " != *" rd.shell "* && " ${CMDLINE} " != *" rd.shell=1 "* ]]

# The kernel and the initramfs are only in the UKI. The initramfs mounts composefs (bootc's module), stops
# a boot where it was mounted without fs-verity (amethystora-sealed), and can ask the TPM for the disk
# key (tpm2-tss), as 19-initramfs.sh's does
if compgen -G "${T}/usr/lib/modules/*/vmlinuz" || compgen -G "${T}/usr/lib/modules/*/initramfs.img"; then
    exit 1
fi
lsinitrd -m "/run/kernel/${KERNEL}/initramfs.img" | grep -x bootc
lsinitrd -m "/run/kernel/${KERNEL}/initramfs.img" | grep -x amethystora-sealed
lsinitrd "/run/kernel/${KERNEL}/initramfs.img" | grep 'initrd-root-fs.target.wants/amethystora-sealed-verity.service' >/dev/null
if dracut --list-modules 2>/dev/null | grep -x tpm2-tss >/dev/null; then
    lsinitrd -m "/run/kernel/${KERNEL}/initramfs.img" | grep -x tpm2-tss
fi

# Nothing of an ostree deployment's boot is left: no rpm-ostree, bootupd or GRUB
for package in rpm-ostree bootupd grub2-common grub2-efi-x64 grub2-tools; do
    if "${RPM[@]}" -q "${package}"; then
        exit 1
    fi
done
# ...and nothing that required rpm-ostree went with it: the power profiles stay
"${RPM[@]}" -q tuned tuned-ppd
# systemd-boot is, signed with the same key, and bootc installs it onto btrfs
"${RPM[@]}" -q systemd-boot-unsigned
sbverify --cert "${CERT}" "${T}/usr/lib/systemd/boot/efi/systemd-bootx64.efi.signed"
grep -rqx 'bootloader = "systemd"' "${T}/usr/lib/bootc/install/"
grep -rqx 'type = "btrfs"' "${T}/usr/lib/bootc/install/"
# shim stays, for the installer to put in front of systemd-boot (amethystora-install-sealed). Whose
# signatures it carries is printed: Microsoft's 2011 UEFI CA expired in June 2026, and machines that
# only know that one need a shim signed with it as well as with its 2023 successor.
SHIM="$(compgen -G "${T}/usr/lib/efi/shim/*/EFI/*/shimx64.efi" | head -n 1)"
[[ -n "${SHIM}" ]]
sbverify --list "${SHIM}" || true
# Upgrades are promised only from composefs installs made with bootc 1.16.0 or later
BOOTC="$("${RPM[@]}" -q --queryformat '%{VERSION}' bootc)"
[[ "$(printf '%s\n' 1.16.0 "${BOOTC}" | sort -V | head -n 1)" == 1.16.0 ]]

# What tells the scripts they run sealed, the sealed stream, and the one way the apps read the
# deployments without rpm-ostree
test -f "${T}/usr/share/amethystora/sealed"
[[ "$(jq -r '."image-tag"' "${T}/usr/share/amethystora/image-info.json")" == *-sealed ]]
test -x "${T}/usr/libexec/amethystora-deployments"
grep -q '/usr/libexec/amethystora-deployments' "${T}/usr/share/polkit-1/rules.d/50-amethystora-deployments.rules"
test ! -e "${T}/etc/rpm-ostreed.conf"

# Updates are still only accepted with this image's signature. Whether bootc's composefs backend applies
# policy.json to every pull is not settled in its documentation: check it on a booted machine before
# publishing (AGENTS.md, Sealed images)
jq -e '.transports.docker["ghcr.io/arsslensoft"] | any(.type == "sigstoreSigned")' "${T}/etc/containers/policy.json"

# The source of every package is listed, after the packages changed (00-sealed.sh)
diff <("${RPM[@]}" -qa --queryformat '%{NAME}\n' | grep -vx gpg-pubkey | sort) \
    <(awk -F'\t' 'NF == 4 && $1 != "PACKAGE" { print $1 }' "${T}/usr/share/licenses/amethystora/SOURCES" | sort)

rm -rf /tmp/rpmdb "${CERT}"

echo "::endgroup::"
