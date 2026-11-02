#!/usr/bin/bash
# vmlab-vm: installer
# The system's own disks (ame system raid mirror DISK, plans/done/PLAN-raid.md's 2b), on the lab's second machine,
# installed as the installer lays a disk out: an ESP, an ext4 /boot and btrfs on LUKS, on its 40 GiB disk, with
# SPARE-1 and 2 (8 GiB) for an encrypted pool and SPARE-3 and 4 (48 GiB) larger than it. In steps, each in a phase
# of its own, a step that runs long going on in the next phase of the same boot:
#   setup       the account, the layout, the firmware's entry as Anaconda makes it, the offer in the status and
#               Control, an encrypted pool beside the encrypted system; then the mirror onto SPARE-3 started
#   mirroring   waited for: the system on both disks, /boot a RAID1 of one half, the restart from the new disk
#               set; the passphrase typed at the restart
#   finished    asked once for both disks; started from the new disk, the first disk's /boot joined, both ESPs
#               pointed at the mirror; the TPM enrolled on the system's two disks and not on the pool's
#   tpm         started by the TPM and its PIN alone; an update, whose new kernel both halves of /boot get
#   updated     both halves and both ESPs alike; then the first disk unplugged
#   first-dead  started from the second disk with "Start with a disk missing"; then the first disk back
#   first-back  the disk that came back has its copies put back, and counts nothing; then SPARE-3 unplugged
#   second-dead started degraded from the first disk; SPARE-4 in SPARE-3's place, its ESP, /boot half and key
#   replaced    started by the TPM from the first disk and SPARE-4; nothing of SPARE-3 left
# vmlab-paths: @sf/usr/libexec/amethystora-raid* @sf/usr/lib/systemd/system/amethystora-raid* @sf/usr/lib/dracut/* @sf/usr/libexec/amethystora-boot-guard @sf/usr/lib/systemd/system/amethystora-boot-guard*
source "$(dirname "$0")/lib.sh"

RAID=/usr/libexec/amethystora-raid
TESTS=/var/lib/vmlab-tests
ENV_FILE="${TESTS}/55-raid-system.env"
SUDOERS=/etc/sudoers.d/zz-vmlab
PASS="${VMLAB_PASSPHRASE}"
PIN="${VMLAB_TPM_PIN}"
AS_TESTER=(runuser -l "${TEST_USER}" -c)
# With a terminal of its own, as a person's: runuser -c starts a new session without one, and sudo then runs
# systemd-cryptenroll without one, which waits for a password agent instead of asking on the terminal
AS_TESTER_PTY=(runuser -l "${TEST_USER}" --pty -c)
MIRROR_OUT="${TESTS}/mirror.out"

# shellcheck source=/dev/null
[[ -f "${ENV_FILE}" ]] && source "${ENV_FILE}"
remember() {
    printf '%s=%q\n' "$1" "$2" >>"${ENV_FILE}"
    printf -v "$1" '%s' "$2"
}
step() { remember STEP "$1"; }

spare() { readlink -f "/dev/disk/by-id/ata-QEMU_HARDDISK_SPARE-$1"; }
status_json() { runuser -u "${TEST_USER}" -- "${RAID}" status --json; }
system_json() { status_json | jq -c 'first(.pools[] | select(.system)) // empty'; }
control_json() { as_user "LC_ALL=C.UTF-8 LANGUAGE= /usr/libexec/amethystora-control-list --json"; }
row_of() { jq -c --arg name "$2" '.items[] | select(.kind == "disks") | .disks[] | select(.id == $name)' <<<"$1"; }
holds() {
    if jq -e "$3" <<<"$2" >/dev/null 2>&1; then ok "$1"; else not_ok "$1" "${2:-(nothing)}"; fi
}
says() {
    local what="$1" out="$2" re
    shift 2
    for re in "$@"; do
        if ! grep -qE -- "${re}" <<<"${out}"; then
            not_ok "${what}" "not said: ${re}" "$(tail -n 25 <<<"${out}")"
            return 1
        fi
    done
    ok "${what}"
}
# The firmware's entry the machine started from, and the entries' numbers by the ESP they name
boot_current() { efibootmgr | sed -n 's/^BootCurrent: //p'; }
# The partition UUID of the ESP this start came from, which the BootCurrent entry names: the lab's disk has two
# entries for its ESP (shim's fallback made one at the first start), so entries are told apart by what they name
started_from() {
    efibootmgr -v | awk -v n="Boot$(boot_current)" 'index($0, n) == 1 && match(tolower($0), /hd\([0-9]+,gpt,[0-9a-f-]+/) {
        split(substr(tolower($0), RSTART, RLENGTH), p, ","); print p[3]; exit }'
}
partuuid() { lsblk -ndo PARTUUID "$1" | tr '[:upper:]' '[:lower:]'; }
entry_of() {  # PARTITION
    local partuuid
    partuuid="$(lsblk -ndo PARTUUID "$1" | tr '[:upper:]' '[:lower:]')"
    efibootmgr -v | awk -v p="${partuuid}" 'tolower($0) ~ p { sub(/^Boot/, "", $1); sub(/\*$/, "", $1); print $1; exit }'
}
md_members() { mdadm --detail /dev/md/md-boot 2>/dev/null | awk '$NF ~ /^\/dev\// && /active sync/ { print $NF }' | sort | paste -sd ' '; }
md_wait() { mdadm --wait /dev/md/md-boot >/dev/null 2>&1; true; }
# Both halves of /boot hold the same: md reads both and counts what differs
md_alike() {
    md_wait
    echo check >"/sys/block/$(basename "$(readlink -f /dev/md/md-boot)")/md/sync_action"
    sleep 2
    md_wait
    [[ "$(cat "/sys/block/$(basename "$(readlink -f /dev/md/md-boot)")/md/mismatch_cnt")" == 0 ]]
}
# COMMAND run in an ESP's top folder, where it is mounted (/boot/efi), or else mounted read-only for it: a second,
# read-only mount of one mounted already is refused
in_esp() {  # PARTITION COMMAND...
    local at mine="" code
    at="$(findmnt -rn -o TARGET --source "$1" | head -n 1)"
    if [[ -z "${at}" ]]; then
        at="$(mktemp -d)" mine=1
        mount -o ro "$1" "${at}" || { rmdir "${at}"; return 1; }
    fi
    (cd "${at}" && "${@:2}")
    code=$?
    [[ -z "${mine}" ]] || { umount "${at}"; rmdir "${at}"; }
    return "${code}"
}
# An ESP's boot files, checksummed
esp_sums() { in_esp "$1" sh -c 'find EFI -type f -print0 | sort -z | xargs -0 sha256sum'; }
luks_of() { cryptsetup luksUUID "$1"; }
has_tpm() { cryptsetup luksDump "$1" | grep -q systemd-tpm2; }
# "Start with a disk missing" is chosen in the boot menu itself, as a person would (next_boot's menu_last): a line
# in custom.cfg would stay on the half of /boot that was away, which GRUB may read once it is back
# The TPM, with the lab's PIN, on every disk of the system: ame security disk-unlock on-pin, which asks each
# disk's passphrase and the PIN twice
enrol_tpm() {
    in_terminal 600 'current passphrase' "${PASS}"$'\r' 'TPM2 PIN' "${PIN}"$'\r' 'TPM2 PIN' "${PIN}"$'\r' \
        '?current passphrase' "${PASS}"$'\r' '?TPM2 PIN' "${PIN}"$'\r' '?TPM2 PIN' "${PIN}"$'\r' \
        -- "${AS_TESTER_PTY[@]}" 'ame security disk-unlock on-pin'
}

abandon() {
    skip "$1"
    rm -f "${SUDOERS}"
    finish
}

[[ "${SUITE_PHASE}" == 1 ]] && { install -d -m 0700 "${TESTS}"; : >"${ENV_FILE}"; remember STEP setup; }
echo "# step ${STEP}, phase ${SUITE_PHASE}"

case "${STEP}" in
setup)
    # The account, as 10-account.sh signs one in on the other machine
    if ! getent passwd "${TEST_USER}" >/dev/null; then
        useradd -m -G wheel -c "Test account" "${TEST_USER}"
        echo "${TEST_USER}:${TEST_USER}" | chpasswd
    fi
    printf '[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=%s\n' "${TEST_USER}" >/etc/gdm/custom.conf
    install -d -o "${TEST_USER}" -g "${TEST_USER}" "/var/home/${TEST_USER}/.config"
    install -o "${TEST_USER}" -g "${TEST_USER}" /dev/null "/var/home/${TEST_USER}/.config/gnome-initial-setup-done"
    systemctl restart gdm
    printf '%s ALL=(ALL) NOPASSWD: ALL\n' "${TEST_USER}" >"${SUDOERS}"
    chmod 0440 "${SUDOERS}"
    check "${TEST_USER} is signed in" wait_for 240 has_session
    check "${TEST_USER}'s sudo asks no password while this suite runs" runuser -u "${TEST_USER}" -- sudo -n true

    # The installer's layout
    root_part="$("${RAID}" root-luks)"
    remember FIRST_ROOT "${root_part}"
    remember FIRST_DISK "/dev/$(lsblk -ndo PKNAME "${root_part}")"
    remember FIRST_BOOT "$(findmnt -rn -o SOURCE --mountpoint /boot | grep '^/dev/' | head -n 1)"
    remember FIRST_ESP "$(findmnt -rn -o SOURCE --mountpoint /boot/efi | head -n 1)"
    if [[ "${root_part}" == /dev/vda3 && "${FIRST_BOOT}" == /dev/vda2 && "${FIRST_ESP}" == /dev/vda1 ]]; then
        ok "the system is laid out as the installer does it: ESP vda1, /boot vda2, LUKS vda3 under btrfs"
    else
        not_ok "the system is not laid out as the installer does it" "$(lsblk -o NAME,FSTYPE,MOUNTPOINTS)"
    fi
    # The firmware's entry for the system's disk, as Anaconda makes one: first in the boot order
    [[ -n "$(entry_of "${FIRST_ESP}")" ]] ||
        efibootmgr --quiet --create --disk "${FIRST_DISK}" --part 1 --label Amethystora --loader '\EFI\fedora\shimx64.efi' >/dev/null
    remember FIRST_ENTRY "$(entry_of "${FIRST_ESP}")"
    remember FIRST_ESP_PARTUUID "$(partuuid "${FIRST_ESP}")"
    [[ -n "${FIRST_ENTRY}" ]] && ok "the firmware has an entry for the system's disk (Boot${FIRST_ENTRY})" ||
        not_ok "no firmware entry for the system's disk" "$(efibootmgr -v)"

    status="$(status_json)"
    holds "the status offers the system's mirror onto SPARE-3 or SPARE-4, as large or larger than its disk, not the 8 GiB ones" \
        "$(jq -c .mirror_onto <<<"${status}")" \
        ".possible and .disk == \"${FIRST_DISK}\" and (.candidates | sort) == ([\"$(spare 3)\", \"$(spare 4)\"] | sort)"
    holds "Control offers Mirror the system onto a new disk, in a terminal" "$(control_json)" \
        'any(.items[]; .id == "raid-mirror-disk" and .terminal and .command == ["ame", "system", "raid", "mirror"])'

    # An encrypted pool beside the encrypted system
    out="$(in_terminal 600 'Which layout\?' $'\r' 'Encrypt the pool\?' $'\r' 'Type ERASE' $'ERASE\r' \
        -- "${AS_TESTER[@]}" "ame system raid create --name data $(spare 1) $(spare 2)")"
    says "create makes data on SPARE-1 and 2, saying the system disk is encrypted too" "${out}" \
        'kept on the system disk, which is encrypted too' 'The pool data is ready at /var/mnt/data'
    [[ "$("${RAID}" root-luks)" == /dev/vda3 ]] && ok "root-luks names the system's partition only, not the pool's" ||
        not_ok "root-luks names $("${RAID}" root-luks | paste -sd ' ')"

    # The mirror, in a terminal of its own: it outlasts this phase
    remember NEW_DISK "$(spare 3)"
    systemd-run --unit=vmlab-mirror --collect --quiet -p StandardOutput="file:${MIRROR_OUT}" -p StandardError=inherit \
        python3 "$(dirname "$0")/term.py" 3000 'Type ERASE' $'ERASE\r' 'passphrase of the system' "${PASS}"$'\r' \
        'Restart now\?' n -- "${AS_TESTER[@]}" "ame system raid mirror ${NEW_DISK}"
    step mirroring
    continue_phase
    ;;

mirroring)
    if ! wait_for 1500 sh -c '! systemctl is-active --quiet vmlab-mirror'; then
        echo "# the mirror is still copying"
        continue_phase
    fi
    out="$(cat "${MIRROR_OUT}" 2>/dev/null)"
    says "ame system raid mirror copies the system onto SPARE-3 and leaves one restart" "${out}" \
        'The system goes onto this disk' 'Every file of the system is on both disks now'
    pool="$(system_json)"
    holds "the system is two copies on two disks, its second SPARE-3's" "${pool}" \
        "(.devices | length) == 2 and .layout == \"raid1\" and .metadata == \"raid1\" and .encrypted
         and any(.devices[]; .disk.serial == \"SPARE-3\") and (.problems | index(\"restart\")) != null"
    remember NEW_ESP "$(lsblk -nrpo PATH "${NEW_DISK}" | sed -n 2p)"
    # By its partition's UUID too: with SPARE-3 unplugged later, SPARE-4 takes its /dev/sdX name
    remember NEW_ESP_PARTUUID "$(lsblk -ndo PARTUUID "${NEW_ESP}" | tr '[:upper:]' '[:lower:]')"
    remember NEW_BOOT "$(lsblk -nrpo PATH "${NEW_DISK}" | sed -n 3p)"
    remember NEW_ROOT "$(lsblk -nrpo PATH,TYPE "${NEW_DISK}" | awk '$2 == "part" { p = $1 } END { print p }')"
    remember NEW_LUKS "$(luks_of "${NEW_ROOT}")"
    remember FIRST_LUKS "$(luks_of "${FIRST_ROOT}")"
    holds "/boot is a RAID1 of two, with the new disk's half in it alone until the restart" \
        "$(mdadm --detail /dev/md/md-boot 2>&1 | jq -Rsc .)" \
        "test(\"Raid Level : raid1\") and test(\"Raid Devices : 2\") and test(\"${NEW_BOOT}\") and test(\"Version : 1.0\")"
    check "the new disk's LUKS opens with the system's passphrase" sh -c "printf '%s' '${PASS}' | cryptsetup open --test-passphrase --key-file - ${NEW_ROOT}"
    check "its line in /etc/crypttab" grep -qE "^luks-${NEW_LUKS} UUID=${NEW_LUKS} none discard,nofail$" /etc/crypttab
    check "its rd.luks.uuid in every boot entry, beside the first disk's" sh -c \
        "for e in /boot/loader/entries/*.conf; do grep -q 'rd.luks.uuid=luks-${NEW_LUKS}' \$e && grep -q 'rd.luks.uuid=luks-${FIRST_LUKS}' \$e || exit 1; done"
    check "the boot menu has Start with a disk missing, degraded and nofail" grep -q "rootflags=subvol=root,degraded.*rd.luks.options=nofail" /boot/grub2/custom.cfg
    check "/boot/efi is nofail: the first disk's ESP is one of two" sh -c "awk '\$2 == \"/boot/efi\"' /etc/fstab | grep -q nofail"
    remember NEW_ENTRY "$(entry_of "${NEW_ESP}")"
    holds "the firmware has an entry for the new disk's shim, after the first, and the next start is from it" \
        "$(efibootmgr | jq -Rsc .)" \
        "test(\"BootNext: ${NEW_ENTRY}\") and test(\"BootOrder: [0-9A-F,]*${FIRST_ENTRY},${NEW_ENTRY}\") and test(\"Amethystora \\\\(\")"
    holds "the new ESP has the boot files, pointed at /boot's mirror" "$(esp_sums "${NEW_ESP}" | jq -Rsc .)" \
        'test("EFI/fedora/shimx64.efi") and test("EFI/fedora/bootuuid.cfg") and test("EFI/BOOT/BOOTX64.EFI")'
    holds "the status: restart to finish" "$(runuser -u "${TEST_USER}" -- "${RAID}" status | jq -Rsc .)" 'test("Restart to finish")'
    # The restart, the passphrase typed at its prompt
    next_boot '{"type_passphrase": true}'
    step finished
    reboot_and_continue
    ;;

finished)
    [[ "${VMLAB_PROMPTS}" == 1 ]] && ok "the passphrase was asked once for both disks of the system" ||
        not_ok "the passphrase was asked ${VMLAB_PROMPTS:-?} times for the system's two disks"
    [[ "$(started_from)" == "${NEW_ESP_PARTUUID}" ]] && ok "the machine started from the new disk (Boot$(boot_current))" ||
        not_ok "the machine started from Boot$(boot_current), not the new disk's Boot${NEW_ENTRY}"
    wait_for 300 sh -c '[ "$(systemctl show -p ActiveState --value amethystora-raid-boot.service)" != activating ]'
    log="$(journalctl -b -u amethystora-raid-mirror.service -u amethystora-raid-boot.service --no-pager -o cat)"
    says "amethystora-raid-boot finished the mirror" "${log}" 'The system is on both disks, and starts from either'
    check "nothing is left to finish" test ! -e /var/lib/amethystora/raid/mirror.json
    md_wait
    members="$(md_members)"
    [[ "${members}" == "${FIRST_BOOT} ${NEW_BOOT}" || "${members}" == "${NEW_BOOT} ${FIRST_BOOT}" ]] &&
        ok "/boot's mirror has both halves: ${members}" || not_ok "/boot's mirror has ${members:-nothing}" "$(cat /proc/mdstat)"
    md_uuid="$(blkid -s UUID -o value /dev/md/md-boot)"
    check "/boot is mounted from the mirror" sh -c "findmnt -rn -o SOURCE --mountpoint /boot | grep -q '^/dev/md'"
    check "fstab mounts /boot by the mirror's filesystem" grep -qE "^UUID=${md_uuid}[[:space:]]+/boot[[:space:]]" /etc/fstab
    in_esp "${FIRST_ESP}" grep -q "${md_uuid}" EFI/fedora/bootuuid.cfg && in_esp "${NEW_ESP}" grep -q "${md_uuid}" EFI/fedora/bootuuid.cfg &&
        ok "both ESPs point at the mirror" ||
        not_ok "an ESP does not point at the mirror (${md_uuid})" "$(in_esp "${FIRST_ESP}" cat EFI/fedora/bootuuid.cfg)" "$(in_esp "${NEW_ESP}" cat EFI/fedora/bootuuid.cfg)"
    holds "the status: the system is two copies, its boot files on both disks, fine" "$(system_json)" \
        '.state == "ok" and .boot.members == 2 and (.boot.degraded | not) and .mirror == null'
    echo "# bootupctl status: $(bootupctl status 2>&1 | paste -sd ' ')"
    # The TPM: the system's two disks get its key, the pool's none
    out="$(enrol_tpm)"
    says "ame security disk-unlock enrols the TPM with a PIN" "${out}" 'Done'
    has_tpm "${FIRST_ROOT}" && has_tpm "${NEW_ROOT}" && ok "the TPM holds a key for both of the system's disks" ||
        not_ok "the TPM key is not on both system disks" "$(cryptsetup luksDump "${NEW_ROOT}" | grep -A3 Tokens)"
    pool_tpm=""
    for p in $(lsblk -nrpo PATH,PARTLABEL | awk '$2 ~ /^raid-data-/ { print $1 }'); do has_tpm "${p}" && pool_tpm+=" ${p}"; done
    [[ -z "${pool_tpm}" ]] && ok "no TPM key on the pool's disks" || not_ok "the TPM got a key for the pool's${pool_tpm}"
    # The next start by the TPM and its PIN alone
    next_boot '{"credentials": {"cryptsetup.passphrase": null}}'
    step tpm
    reboot_and_continue
    ;;

tpm)
    ok "the system started with the TPM and its PIN, no passphrase given"
    check "the pool data opened at boot too" findmnt -rn --mountpoint /var/mnt/data
    check "the boot menu's Start with a disk missing keeps the TPM's option" grep -q "rd.luks.options=tpm2-device=auto,nofail" /boot/grub2/custom.cfg
    # An update: a new deployment, written to /boot, the mirror
    check "a new deployment (a kernel argument added)" rpm-ostree kargs --append=vmlab.update=1
    step updated
    reboot_and_continue
    ;;

updated)
    check "the machine runs the new deployment" grep -qw vmlab.update=1 /proc/cmdline
    check "both halves of /boot are alike after the update" md_alike
    first="$(esp_sums "${FIRST_ESP}")" second="$(esp_sums "${NEW_ESP}")"
    [[ -n "${first}" && "${first}" == "${second}" ]] && ok "both ESPs hold the same boot files" ||
        not_ok "the ESPs differ" "$(diff <(echo "${first}") <(echo "${second}"))"
    out="$(bootupctl update 2>&1)"
    echo "# bootupctl update: $(paste -sd ' ' <<<"${out}")"
    first="$(esp_sums "${FIRST_ESP}")" second="$(esp_sums "${NEW_ESP}")"
    [[ "${first}" == "${second}" ]] && ok "and still after bootupctl update" || not_ok "bootupctl update left the ESPs different"
    # The first disk dead: the machine starts from the second disk, chosen degraded in the boot menu
    remember REF_BOOT "$(($(systemctl show graphical.target -p ActiveEnterTimestampMonotonic --value) / 1000000))"
    next_boot '{"system_absent": true, "menu_last": true, "credentials": {"cryptsetup.passphrase": null}}'
    step first-dead
    reboot_and_continue
    ;;

first-dead)
    [[ "$(started_from)" == "${NEW_ESP_PARTUUID}" ]] && ok "with the first disk gone, the firmware started the second (Boot$(boot_current))" ||
        not_ok "the machine started from Boot$(boot_current)"
    check "the system is mounted degraded" sh -c 'findmnt -rn -o OPTIONS --mountpoint /sysroot | grep -qw degraded'
    check "/boot is the mirror's half that is left" sh -c "findmnt -rn -o SOURCE --mountpoint /boot | grep -q '^/dev/md'"
    booted="$(($(systemctl show graphical.target -p ActiveEnterTimestampMonotonic --value) / 1000000))"
    echo "# desktop at ${booted} s, ${REF_BOOT} s with both disks"
    ((booted - REF_BOOT <= 60)) && ok "the start waited $((booted - REF_BOOT)) s more at most for the missing disk" ||
        not_ok "the start took $((booted - REF_BOOT)) s longer" "$(systemd-analyze critical-chain 2>&1 | head -n 15)"
    holds "the status: a disk of the system is missing, every file still there" "$(system_json)" \
        '.missing == 1 and (.problems | index("missing")) != null'
    holds "Control offers Replace a disk… for the system" "$(row_of "$(control_json)" system)" \
        'any(.actions[]; .id == "replace" and .terminal and .command == ["ame", "system", "raid", "replace", "system"])'
    check "the missing disk is written down" jq -e 'length > 0' /var/lib/amethystora/raid/missing.json
    check "${TEST_USER} writes while the disk is missing" as_user 'head -c 64M /dev/urandom >~/degraded && sync'
    step first-back
    next_boot '{"credentials": {"cryptsetup.passphrase": null}}'
    reboot_and_continue
    ;;

first-back)
    [[ "$(started_from)" == "${FIRST_ESP_PARTUUID}" ]] && ok "with the first disk back, the machine starts from it again" ||
        not_ok "the machine started from Boot$(boot_current)"
    holds "the status: the first disk is back, not failing" "$(system_json)" \
        '.missing == 0 and any(.devices[]; .returned) and (.problems | index("returned")) != null and (.problems | index("errors")) == null'
    holds "Control offers Put the copies back… for the system" "$(row_of "$(control_json)" system)" \
        'any(.actions[]; .id == "replace" and .command == ["ame", "system", "raid", "replace", "system"])'
    out="$(in_terminal 1500 'Put the copies back now\?' y -- "${AS_TESTER[@]}" 'ame system raid replace system')"
    says "ame system raid replace system puts the copies back" "${out}" 'is back, with old copies' 'Every file of (the )?system has its copies again'
    md_wait
    holds "the system is whole again: nothing missing, back or counting" "$(system_json)" \
        '.state == "ok" and .missing == 0 and .errors == 0 and all(.devices[]; (.returned | not) and .error_count == 0) and .boot.members == 2'
    out="$(btrfs scrub start -B /var 2>&1)"
    grep -qE 'Error summary: +no errors found' <<<"${out}" && ok "a second check of every copy finds nothing" ||
        not_ok "the second check found something" "${out}"
    check "both halves of /boot are alike" md_alike
    # The second disk dead: started degraded from the first, and replaced by SPARE-4
    step second-dead
    next_boot '{"absent": [3], "menu_last": true, "credentials": {"cryptsetup.passphrase": null}}'
    reboot_and_continue
    ;;

second-dead)
    [[ "$(started_from)" == "${FIRST_ESP_PARTUUID}" ]] && ok "with SPARE-3 gone, the machine starts from the first disk" ||
        not_ok "the machine started from Boot$(boot_current)"
    check "the system is mounted degraded" sh -c 'findmnt -rn -o OPTIONS --mountpoint /sysroot | grep -qw degraded'
    systemd-run --unit=vmlab-replace --collect --quiet -p StandardOutput="file:${TESTS}/replace.out" -p StandardError=inherit \
        python3 "$(dirname "$0")/term.py" 3000 'Which disk of the system goes\?' $'\r' 'Which disk takes its place\?' $'\r' \
        'Type ERASE' $'ERASE\r' 'passphrase of the system' "${PASS}"$'\r' \
        'current passphrase' "${PASS}"$'\r' 'TPM2 PIN' "${PIN}"$'\r' 'TPM2 PIN' "${PIN}"$'\r' \
        '?current passphrase' "${PASS}"$'\r' '?TPM2 PIN' "${PIN}"$'\r' '?TPM2 PIN' "${PIN}"$'\r' \
        -- "${AS_TESTER_PTY[@]}" 'ame system raid replace system'
    step replacing
    continue_phase
    ;;

replacing)
    if ! wait_for 1500 sh -c '! systemctl is-active --quiet vmlab-replace'; then
        echo "# the replace is still copying"
        continue_phase
    fi
    out="$(cat "${TESTS}/replace.out" 2>/dev/null)"
    says "ame system raid replace system puts SPARE-4 in SPARE-3's place, and the TPM gets its key" "${out}" \
        'is replaced by' 'of the system is replaced' 'Done'
    remember REPLACED_DISK "$(spare 4)"
    remember REPLACED_ROOT "$(lsblk -nrpo PATH,TYPE "${REPLACED_DISK}" | awk '$2 == "part" { p = $1 } END { print p }')"
    md_wait
    holds "the system is whole on the first disk and SPARE-4" "$(system_json)" \
        '.missing == 0 and .layout == "raid1" and (.degraded | not) and .boot.members == 2 and any(.devices[]; .disk.serial == "SPARE-4")'
    check "SPARE-3's key is out of the boot entries and /etc/crypttab" sh -c \
        "! grep -qs '${NEW_LUKS}' /boot/loader/entries/*.conf /etc/crypttab"
    check "SPARE-4's key is in /etc/crypttab and every boot entry" sh -c \
        "u=\$(cryptsetup luksUUID ${REPLACED_ROOT}); grep -q \$u /etc/crypttab && for e in /boot/loader/entries/*.conf; do grep -q \"rd.luks.uuid=luks-\$u\" \$e || exit 1; done"
    check "the firmware's entry for SPARE-3 is gone, SPARE-4 has one" sh -c \
        "! efibootmgr -v | grep -qi '${NEW_ESP_PARTUUID}' && efibootmgr | grep -q 'Amethystora ($(basename "${REPLACED_DISK}"))'"
    has_tpm "${REPLACED_ROOT}" && ok "the TPM holds a key for SPARE-4" || not_ok "no TPM key on SPARE-4"
    step replaced
    next_boot '{"absent": [3], "credentials": {"cryptsetup.passphrase": null}}'
    reboot_and_continue
    ;;

replaced)
    ok "the system started by the TPM from the first disk and SPARE-4"
    holds "the system is fine" "$(system_json)" '.state == "ok" and .missing == 0 and .boot.members == 2'
    check "both halves of /boot are alike" md_alike
    rm -f "${SUDOERS}"
    finish
    ;;

*) finish ;;
esac
