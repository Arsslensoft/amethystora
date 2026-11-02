#!/usr/bin/bash
# `ame system raid` (/usr/libexec/amethystora-raid, or the copy named first) against a stand-in machine,
# for 20-tests.sh, since the build has no disks to make a pool of: lsblk's JSON, a /sys tree, findmnt,
# btrfs filesystem show, fstab. No disk in use is ever offered, the layouts that fit each number of disks
# hold what btrfs's allocator gives them, the status reads a missing disk, error counts and what is left
# with one copy, the system's LUKS is told apart from a pool's, and without a terminal nothing is erased.

set -euo pipefail

RAID="${1:-/usr/libexec/amethystora-raid}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
mkdir -p "${T}/bin"
export AMETHYSTORA_RAID_ROOT="${T}/root" PATH="${T}/bin:${PATH}" LOG="${T}/log" STAND_IN="${T}"
: >"${LOG}"
S="${AMETHYSTORA_RAID_ROOT}/sys"

ROOT=11111111-0000-4000-8000-000000000000
DATA=22222222-0000-4000-8000-000000000000
BACKUP=33333333-0000-4000-8000-000000000000
JOINED=44444444-0000-4000-8000-000000000000
PHOTOS=55555555-0000-4000-8000-000000000000
HAND=66666666-0000-4000-8000-000000000000
export ROOT

# What reads the machine answers from the stand-ins; whatever would change a disk is only logged
cat >"${T}/bin/stub" <<'EOF'
#!/usr/bin/bash
case "${0##*/}" in
    lsblk) [[ " $* " == *" -J "* ]] && cat "${STAND_IN}/lsblk.json" ;;
    findmnt)
        case "$*" in
            *"-t btrfs"*) cat "${STAND_IN}/mounts" ;;
            *"--mountpoint /sysroot"*) echo "btrfs /dev/mapper/luks-c0[/root] ${ROOT}" ;;
            *"--mountpoint /boot"*OPTIONS*) echo "/dev/sda2 ext4 rw,relatime" ;;
            *"--mountpoint /boot"*) echo "/dev/sda2 ext4 b0000000-0000-4000-8000-000000000000" ;;
            *) exit 1 ;;
        esac
        ;;
    btrfs)
        [[ "$1 $2" == "filesystem show" && -f "${STAND_IN}/show${4//\//_}" ]] || exit 1
        cat "${STAND_IN}/show${4//\//_}"
        ;;
    df) printf 'Size Used\n%s %s\n' 1000204886016 400000000000 ;;
    systemctl) [[ "$1" == is-enabled ]] && echo enabled ;;
    *) echo "${0##*/} $*" >>"${LOG}" ;;
esac
EOF
chmod +x "${T}/bin/stub"
for tool in lsblk findmnt btrfs df systemctl sudo gum wipefs sfdisk cryptsetup mkfs.btrfs smartctl systemd-run mount; do
    ln -s stub "${T}/bin/${tool}"
done

# --- The machine -----------------------------------------------------------------------------------

btrfs_fs() {  # UUID LABEL KERNEL-NAME...
    local dir="${S}/fs/btrfs/$1" kname
    mkdir -p "${dir}/devices" "${dir}/devinfo" "${dir}/allocation/data" "${dir}/allocation/metadata"
    echo "$2" >"${dir}/label"
    echo none >"${dir}/exclusive_operation"
    echo 100000000000 >"${dir}/allocation/data/bytes_used"
    echo 1000000000 >"${dir}/allocation/metadata/bytes_used"
    shift 2
    for kname in "$@"; do mkdir -p "${dir}/devices/${kname}"; done
}
devinfo() {  # UUID DEVID MISSING WRITE READ FLUSH CORRUPTION GENERATION
    local dir="${S}/fs/btrfs/$1/devinfo/$2"
    mkdir -p "${dir}"
    echo "$3" >"${dir}/missing"
    printf 'write_errs %s\nread_errs %s\nflush_errs %s\ncorruption_errs %s\ngeneration_errs %s\n' \
        "$4" "$5" "$6" "$7" "$8" >"${dir}/error_stats"
}
profile() {  # UUID KIND PROFILE BYTES
    mkdir -p "${S}/fs/btrfs/$1/allocation/$2/$3"
    echo "$4" >"${S}/fs/btrfs/$1/allocation/$2/$3/total_bytes"
}
show() {  # TARGET, then "devid path" a line on stdin
    awk '{ printf "\tdevid %4s size 1000186000000 used 2000000000 path %s%s\n", $1, $2, ($2 ~ /^\// ? "" : " MISSING") }' \
        >"${T}/show${1//\//_}"
}

# The system: one disk, LUKS under btrfs
btrfs_fs "${ROOT}" fedora dm-0
devinfo "${ROOT}" 1 0 0 0 0 0 0
profile "${ROOT}" data single 200000000000
mkdir -p "${S}/class/block/dm-0/dm" "${S}/class/block/dm-0/slaves/sda3" "${S}/block/sda/sda3/holders/dm-0"
echo "CRYPT-LUKS2-c0-luks-c0" >"${S}/class/block/dm-0/dm/uuid"

# data: two copies, encrypted, its second disk gone, an empty single/ left from mkfs
btrfs_fs "${DATA}" data dm-5
devinfo "${DATA}" 1 0 0 0 0 0 0
devinfo "${DATA}" 2 1 0 0 0 0 0
profile "${DATA}" data raid1 50000000000
profile "${DATA}" data single 0
profile "${DATA}" metadata raid1 1073741824
mkdir -p "${S}/class/block/dm-5/dm" "${S}/class/block/dm-5/slaves/sdm1" "${S}/block/sdm/sdm1/holders/dm-5"
echo "CRYPT-LUKS2-m1-raid-data-1" >"${S}/class/block/dm-5/dm/uuid"
printf '1 /dev/dm-5\n2 <missing>\n' | show /var/mnt/data

# backup: two copies, counting errors, and with single bytes written while a disk was away: more data than it
# held, and as much metadata, as the VM lab's pool had once a disk was back
btrfs_fs "${BACKUP}" backup dm-6 dm-7
devinfo "${BACKUP}" 1 0 0 3 0 0 0
devinfo "${BACKUP}" 2 0 0 0 0 1 0
profile "${BACKUP}" data raid1 50000000000
profile "${BACKUP}" data single 60000000000
profile "${BACKUP}" metadata raid1 1073741824
profile "${BACKUP}" metadata single 1073741824
printf '1 /dev/dm-6\n2 /dev/dm-7\n' | show /var/mnt/backup

# joined: combined, unencrypted, whose single data is what it was made as
btrfs_fs "${JOINED}" joined sdn1 sdo1
devinfo "${JOINED}" 1 0 0 0 0 0 0
devinfo "${JOINED}" 2 0 0 0 0 0 0
profile "${JOINED}" data single 80000000000
profile "${JOINED}" data raid1 0
profile "${JOINED}" metadata raid1 1073741824
printf '1 /dev/sdn1\n2 /dev/sdo1\n' | show /var/mnt/joined

# hand: two copies someone made by hand, mounted from its first disk, in no fstab line of ours
btrfs_fs "${HAND}" hand sdr1 sds1
devinfo "${HAND}" 1 0 0 0 0 0 0
devinfo "${HAND}" 2 0 0 0 0 0 0
profile "${HAND}" data raid1 1000000000

# sdk: held open by something lsblk does not show
mkdir -p "${S}/block/sdk/holders/dm-9"

cat >"${T}/mounts" <<EOF
${ROOT} /sysroot ro,relatime
${ROOT} /var rw,relatime
${DATA} /var/mnt/data rw,relatime
${BACKUP} /var/mnt/backup rw,relatime
${JOINED} /var/mnt/joined rw,relatime
${HAND} /mnt/hand rw,relatime
EOF

mkdir -p "${AMETHYSTORA_RAID_ROOT}/etc"
OPTIONS=compress=zstd:1,nofail,x-systemd.device-bound=no,x-systemd.device-timeout=30s,x-gvfs-show,x-amethystora-raid
cat >"${AMETHYSTORA_RAID_ROOT}/etc/fstab" <<EOF
UUID=${ROOT} / btrfs subvol=root,compress=zstd:1 0 0
UUID=${DATA} /var/mnt/data btrfs ${OPTIONS} 0 0
UUID=${BACKUP} /var/mnt/backup btrfs ${OPTIONS} 0 0
UUID=${JOINED} /var/mnt/joined btrfs ${OPTIONS} 0 0
UUID=${PHOTOS} /var/mnt/photos btrfs ${OPTIONS} 0 0
# UUID=77777777-0000-4000-8000-000000000000 /var/mnt/old btrfs ${OPTIONS} 0 0
EOF

disk() {  # NAME SIZE [KEY=JSON...] [CHILDREN-JSON]: a disk as lsblk -J -b gives it
    jq -cn --arg name "$1" --argjson size "$2" --argjson extra "${3:-"{}"}" --argjson children "${4:-null}" '
        {name: $name, kname: $name, path: "/dev/\($name)", type: "disk", size: $size, model: "Model \($name) ",
         serial: "S-\($name)", tran: "sata", ro: false, fstype: null, label: null, uuid: null, partlabel: null,
         mountpoints: [null]} + $extra + (if $children == null then {} else {children: $children} end)'
}
part() {  # NAME [KEY=JSON...] [CHILDREN-JSON]
    jq -cn --arg name "$1" --argjson extra "${2:-"{}"}" --argjson children "${3:-null}" '
        {name: $name, kname: $name, path: "/dev/\($name)", type: "part", size: 1000000000000, ro: false, fstype: null,
         label: null, uuid: null, partlabel: null, mountpoints: [null]} + $extra
        + (if $children == null then {} else {children: $children} end)'
}
crypt() {  # KERNEL-NAME [KEY=JSON...]
    jq -cn --arg kname "$1" --argjson extra "${2:-"{}"}" '
        {name: "luks-\($kname)", kname: $kname, path: "/dev/mapper/luks-\($kname)", type: "crypt", size: 999000000000,
         ro: false, fstype: "btrfs", label: null, uuid: null, partlabel: null, mountpoints: [null]} + $extra'
}
TB=1000204886016
{
    disk sda 512110190592 '{"tran": "nvme"}' "[$(part sda1 '{"fstype": "vfat", "parttype": "C12A7328-F81F-11D2-BA4B-00A0C93EC93B", "mountpoints": ["/boot/efi"]}'),
        $(part sda2 '{"fstype": "ext4", "mountpoints": ["/boot"]}'),
        $(part sda3 '{"fstype": "crypto_LUKS", "uuid": "c0"}' "[$(crypt dm-0 "{\"uuid\": \"${ROOT}\", \"mountpoints\": [\"/sysroot\", \"/var\"]}")]")]"
    disk sdb "${TB}" '{"model": " Blank One  "}'
    disk sdc 2000398934016 '{"model": "Blank, Two", "tran": "usb"}'
    disk sdd "${TB}" '{}' "[$(part sdd1 '{"fstype": "ext4", "mountpoints": ["/run/media/me/stick"]}')]"
    disk sde "${TB}" '{}' "[$(part sde1 '{"fstype": "swap", "mountpoints": ["[SWAP]"]}')]"
    disk sdf "${TB}" '{}' "[$(part sdf1 '{"fstype": "crypto_LUKS"}' "[$(crypt dm-3)]")]"
    disk sdg "${TB}" '{}' "[$(part sdg1 '{"fstype": "linux_raid_member"}' '[{"name": "md127", "kname": "md127", "type": "raid1", "mountpoints": [null]}]')]"
    disk sdh "${TB}" '{}' "[$(part sdh1 '{"fstype": "linux_raid_member"}')]"
    disk zram0 8589934592
    disk sdi "${TB}" '{"ro": true}'
    disk sdk "${TB}"
    disk sdl "${TB}" '{}' "[$(part sdl1 '{"fstype": "crypto_LUKS", "partlabel": "raid-photos-2", "uuid": "l1"}')]"
    disk sdm "${TB}" '{}' "[$(part sdm1 '{"fstype": "crypto_LUKS", "partlabel": "raid-data-1"}' \
        "[$(crypt dm-5 "{\"uuid\": \"${DATA}\", \"mountpoints\": [\"/var/mnt/data\"]}")]")]"
    disk sdp "${TB}" '{}' "[$(part sdp1 '{"fstype": "crypto_LUKS", "partlabel": "raid-backup-1"}' \
        "[$(crypt dm-6 "{\"uuid\": \"${BACKUP}\", \"mountpoints\": [\"/var/mnt/backup\"]}")]")]"
    disk sdq "${TB}" '{}' "[$(part sdq1 '{"fstype": "crypto_LUKS", "partlabel": "raid-backup-2"}' "[$(crypt dm-7 "{\"uuid\": \"${BACKUP}\"}")]")]"
    disk sdn "${TB}" '{}' "[$(part sdn1 "{\"fstype\": \"btrfs\", \"partlabel\": \"raid-joined-1\", \"uuid\": \"${JOINED}\", \"mountpoints\": [\"/var/mnt/joined\"]}")]"
    disk sdo "${TB}" '{}' "[$(part sdo1 "{\"fstype\": \"btrfs\", \"partlabel\": \"raid-joined-2\", \"uuid\": \"${JOINED}\"}")]"
    disk sdr "${TB}" '{}' "[$(part sdr1 "{\"fstype\": \"btrfs\", \"uuid\": \"${HAND}\", \"mountpoints\": [\"/mnt/hand\"]}")]"
    disk sds "${TB}" '{}' "[$(part sds1 "{\"fstype\": \"btrfs\", \"uuid\": \"${HAND}\"}")]"
} | jq -cs '{blockdevices: (. + [{name: "loop0", kname: "loop0", path: "/dev/loop0", type: "loop", size: 1000000000, mountpoints: [null]}])}' \
    >"${T}/lsblk.json"

# --- What it says ----------------------------------------------------------------------------------

STATUS="$("${RAID}" status --json)"
pool() { jq -c --arg id "$1" '.pools[] | select(.id == $id)' <<<"${STATUS}"; }

# Only the two blank disks: nothing mounted, swap, open LUKS, md, read-only, zram, a loop device, held
# in sysfs alone, the second disk of a btrfs mounted from its first, or a disk of a pool that did not
# mount. The USB one is offered, and says so; makers' spaces are trimmed and their commas kept.
jq -e '[.spare[].path] == ["/dev/sdb", "/dev/sdc"]' <<<"${STATUS}" >/dev/null
jq -e '.spare[0].model == "Blank One" and (.spare[0].usb | not) and .spare[1].usb and .spare[1].model == "Blank, Two"' <<<"${STATUS}" >/dev/null

# data: the missing disk, nothing else wrong; an empty single/ is not data with one copy
jq -e '.managed and .mounted and .encrypted and .layout == "raid1" and .metadata == "raid1" and .survives == 1
    and .missing == 1 and .problems == ["missing"] and .state == "check" and (.degraded | not)
    and .devices[0].disk.path == "/dev/sdm" and .devices[0].kname == "dm-5" and .devices[1].missing
    and .mountpoint == "/var/mnt/data" and .target == "/var/mnt/data"' <<<"$(pool data)" >/dev/null
# backup: each error counter, by disk, and single bytes on a pool made with copies
jq -e '.layout == "raid1" and .metadata == "raid1" and .survives == 1
    and .errors == 4 and .devices[0].errors.read == 3 and .devices[1].errors.corruption == 1
    and .devices[1].error_count == 1 and .degraded and .problems == ["errors", "degraded"]
    and .devices[1].disk.path == "/dev/sdq"' <<<"$(pool backup)" >/dev/null
# joined: combined is single by choice, fine, and survives nothing
jq -e '.layout == "single" and .survives == 0 and (.degraded | not) and .problems == [] and .state == "ok"
    and (.encrypted | not)' <<<"$(pool joined)" >/dev/null
# photos: expected, not mounted, its one disk that is there found by its partition's name
jq -e '.managed and (.mounted | not) and .encrypted and .problems == ["unmounted"]
    and [.devices[].disk.path] == ["/dev/sdl"]' <<<"$(pool photos)" >/dev/null
# hand: made by hand, watched all the same, and changed by nothing here
jq -e '(.managed | not) and .layout == "raid1" and .mountpoint == "/mnt/hand"' <<<"$(pool hand)" >/dev/null
# The system is on one disk: no pool, and nothing to mirror. The commented line is no pool either.
jq -e '([.pools[].id] | sort) == ["backup", "data", "hand", "joined", "photos"] and (.system_single | not) and .raid
    and .scrub_timer == "enabled"' <<<"${STATUS}" >/dev/null
grep -q "A disk is missing. Every file is still there" <<<"$("${RAID}" status)"
"${RAID}" present

# The system's LUKS, which TPM unlocking enrols, is the one under its own btrfs, not a pool's
[[ "$("${RAID}" root-luks)" == /dev/sda3 ]]

# The layouts each number of disks is offered, two copies first
TB_GIB=931
layouts() { "${RAID}" layouts "$@"; }
jq -e '[.[].id] == ["raid1", "single"] and all(.[]; .metadata == "raid1")' <<<"$(layouts "${TB}" "${TB}")" >/dev/null
jq -e '[.[].id] == ["raid1", "raid1c3", "raid5", "single"] and all(.[]; .metadata == "raid1c3")' \
    <<<"$(layouts "${TB}" "${TB}" "${TB}")" >/dev/null
jq -e '[.[].id] == ["raid1", "raid1c3", "raid10", "raid5", "raid6", "single"]' <<<"$(layouts "${TB}" "${TB}" "${TB}" "${TB}")" >/dev/null
jq -e '.[0].warning == null and (.[] | select(.id == "single") | .warning | test("No copies"))' <<<"$(layouts "${TB}" "${TB}")" >/dev/null
jq -e 'all(.[] | select(.id == "raid5" or .id == "raid6"); .warning | test("not be used in production"))' \
    <<<"$(layouts "${TB}" "${TB}" "${TB}" "${TB}")" >/dev/null
# What each holds, in whole GiB of the 931 a 1 TB disk has, and the 1863 of a 2 TB one
usable() {  # LAYOUT GIB SIZE...
    local id="$1" gib="$2"
    shift 2
    jq -e --arg id "${id}" --argjson bytes "$((gib * 1073741824))" '.[] | select(.id == $id) | .usable == $bytes' \
        <<<"$(layouts "$@")" >/dev/null || { echo "${id} on $* does not hold ${gib} GiB"; false; }
}
TB2=2000398934016
usable raid1 "${TB_GIB}" "${TB}" "${TB}"
usable raid1 "${TB_GIB}" "${TB}" "${TB2}"
usable raid1 $((2 * TB_GIB)) "${TB}" "${TB}" "${TB2}"
usable raid1c3 "${TB_GIB}" "${TB}" "${TB}" "${TB}"
usable raid5 $((2 * TB_GIB)) "${TB}" "${TB}" "${TB}"
usable raid6 $((2 * TB_GIB)) "${TB}" "${TB}" "${TB}" "${TB}"
usable raid10 $((2 * TB_GIB)) "${TB}" "${TB}" "${TB}" "${TB}"
usable single $((TB_GIB + 1863)) "${TB}" "${TB2}"

# The system on one disk, laid out as the installer does it, on UEFI: it can be mirrored onto either blank disk,
# both larger than its own. Sealed, or started without UEFI, it cannot.
mkdir -p "${S}/firmware/efi"
jq -e '.mirror_onto == {possible: true, reason: null, disk: "/dev/sda", size: 512110190592, candidates: ["/dev/sdb", "/dev/sdc"]}' \
    <<<"$("${RAID}" status --json)" >/dev/null
grep -q "ame system raid mirror DISK copies it onto another" <<<"$("${RAID}" status)"
mkdir -p "${AMETHYSTORA_RAID_ROOT}/usr/share/amethystora" && touch "${AMETHYSTORA_RAID_ROOT}/usr/share/amethystora/sealed"
jq -e '.mirror_onto.reason == "sealed" and (.mirror_onto.possible | not)' <<<"$("${RAID}" status --json)" >/dev/null
rm "${AMETHYSTORA_RAID_ROOT}/usr/share/amethystora/sealed" && rmdir "${S}/firmware/efi"
jq -e '.mirror_onto.reason == "no-uefi"' <<<"$("${RAID}" status --json)" >/dev/null
"${RAID}" mirror </dev/null >/dev/null 2>&1 && false

# A disk that was missing and is back counts the old copies it has as errors: it is told apart, and is no
# failing disk
mkdir -p "${AMETHYSTORA_RAID_ROOT}/var/lib/amethystora/raid"
echo "{\"${BACKUP}\": {\"2\": 1700000000}}" >"${AMETHYSTORA_RAID_ROOT}/var/lib/amethystora/raid/missing.json"
jq -e '.devices[1].returned and (.devices[0].returned | not) and .errors == 3 and .problems == ["errors", "returned", "degraded"]' \
    <<<"$("${RAID}" status --json | jq -c '.pools[] | select(.id == "backup")')" >/dev/null
rm "${AMETHYSTORA_RAID_ROOT}/var/lib/amethystora/raid/missing.json"

# Over several disks, the system gets the boot menu's "Start with a disk missing" at each start: the default
# deployment's entry, its ostree= link as ostree wrote it (the only kind ostree-prepare-root follows), mounted
# degraded with every LUKS device nofail, after what custom.cfg held already; and loses it again on one disk.
# Root's, as amethystora-raid-boot.service runs it.
if [[ ${EUID} -eq 0 ]]; then
    B="${AMETHYSTORA_RAID_ROOT}/boot"
    mkdir -p "${B}/loader/entries" "${B}/grub2"
    KARGS="root=UUID=${ROOT} rootflags=subvol=root rw rd.luks.uuid=luks-c0 rd.luks.options=tpm2-device=auto ostree=/ostree/boot.1/default/abc/0"
    entry() {  # FILE INDEX BOOTCSUM: an ostree boot entry, the default one with index 0
        printf '%s\n' "title Amethystora (ostree:$2)" "version $((2 - $2))" "options ${KARGS/abc\/0/$3\/$2}" \
            "linux /ostree/default-$3/vmlinuz-7" "initrd /ostree/default-$3/initramfs-7.img" >"${B}/loader/entries/$1"
    }
    entry ostree-1.conf 1 old
    entry ostree-2.conf 0 abc
    echo 'menuentry "Mine" { true }' >"${B}/grub2/custom.cfg"
    devinfo "${ROOT}" 2 0 0 0 0 0 0
    "${RAID}" present-system
    "${RAID}" boot
    grep -qx 'menuentry "Mine" { true }' "${B}/grub2/custom.cfg"
    grep -qx "menuentry 'Start with a disk missing' --unrestricted --id amethystora-degraded {" "${B}/grub2/custom.cfg"
    LINUX="    linux /ostree/default-abc/vmlinuz-7 root=UUID=${ROOT} rootflags=subvol=root,degraded rw rd.luks.uuid=luks-c0"
    grep -qxF "${LINUX} rd.luks.options=tpm2-device=auto,nofail ostree=/ostree/boot.1/default/abc/0" "${B}/grub2/custom.cfg"
    grep -qxF "    initrd /ostree/default-abc/initramfs-7.img" "${B}/grub2/custom.cfg"
    before="$(cat "${B}/grub2/custom.cfg")"
    "${RAID}" boot
    [[ "$(cat "${B}/grub2/custom.cfg")" == "${before}" && "$(grep -c amethystora-degraded "${B}/grub2/custom.cfg")" == 1 ]]
    rm -r "${S}/fs/btrfs/${ROOT}/devinfo/2"
    "${RAID}" present-system
    "${RAID}" boot
    [[ "$(cat "${B}/grub2/custom.cfg")" == 'menuentry "Mine" { true }' ]]
    "${RAID}" present-system && false
    # The boot guard writes this file too and takes /run/amethystora/boot-cfg.lock (it makes the lock when it is on): with the
    # lock there the entry is written as before, and waits while another holds it
    if [[ ! -e /run/amethystora/boot-cfg.lock ]] && mkdir -p /run/amethystora && : >/run/amethystora/boot-cfg.lock; then
        devinfo "${ROOT}" 2 0 0 0 0 0 0
        "${RAID}" present-system
        "${RAID}" boot
        [[ "$(grep -c amethystora-degraded "${B}/grub2/custom.cfg")" == 1 ]]
        rm -r "${S}/fs/btrfs/${ROOT}/devinfo/2"
        flock /run/amethystora/boot-cfg.lock /usr/bin/sleep 2 &
        /usr/bin/sleep 0.5
        started=${SECONDS}
        "${RAID}" boot
        ((SECONDS - started >= 1))
        wait
        [[ "$(cat "${B}/grub2/custom.cfg")" == 'menuentry "Mine" { true }' ]]
        compgen -G "${B}/grub2/custom.cfg.new*" && false
        rm -f /run/amethystora/boot-cfg.lock
        rmdir /run/amethystora 2>/dev/null || true
    fi
    # With the boot guard on (its mark) what is written to /boot is flushed to the disk, GRUB reading ext4 without its
    # journal; with it off nothing is frozen and raid does exactly what it did
    mkdir -p "${T}/fz"
    printf '#!/bin/sh\necho "$*" >>%s\n' "${T}/fz/log" >"${T}/fz/fsfreeze"
    chmod +x "${T}/fz/fsfreeze"
    : >"${T}/fz/log"
    guard_dir=/var/lib/amethystora/boot-guard
    if [[ ! -e "${guard_dir}/on" ]]; then
        devinfo "${ROOT}" 2 0 0 0 0 0 0
        PATH="${T}/fz:${PATH}" "${RAID}" present-system
        PATH="${T}/fz:${PATH}" "${RAID}" boot
        [[ ! -s "${T}/fz/log" ]]
        made=false
        [[ -d "${guard_dir}" ]] || { mkdir -p "${guard_dir}" && made=true; }
        : >"${guard_dir}/on"
        rm -r "${S}/fs/btrfs/${ROOT}/devinfo/2"
        PATH="${T}/fz:${PATH}" "${RAID}" present-system
        PATH="${T}/fz:${PATH}" "${RAID}" boot
        rm -f "${guard_dir}/on"
        ${made} && rmdir "${guard_dir}"
        [[ "$(cat "${T}/fz/log")" == "$(printf -- '-f /boot\n-u /boot')" ]]
        [[ "$(cat "${B}/grub2/custom.cfg")" == 'menuentry "Mine" { true }' ]]
    fi
fi

# Nothing above touched a disk, and without a terminal create refuses before it asks anything
[[ ! -s "${LOG}" ]]
"${RAID}" create </dev/null >/dev/null 2>&1 && false
"${RAID}" create /dev/sdb /dev/sdc </dev/null >/dev/null 2>&1 && false
[[ ! -s "${LOG}" ]]

echo "test-raid: every check passed"
