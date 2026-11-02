#!/usr/bin/bash
# The system spread over two disks with one copy of each file, as the installer leaves it when two disks are
# chosen, mirrored (ame system raid mirror, plans/done/PLAN-raid.md's 2a), started with its second disk dead and
# given it back. On SPARE-6 (24 GiB, more than the system's data), which stays unplugged until this suite. In
# steps, each in a phase of its own, a step that runs long going on in the next phase of the same boot:
#   plug        SPARE-6 plugged in (a power cycle)
#   spread      the system over the lab disk and SPARE-6, as the installer spreads it; offered and mirrored
#   mirroring   waited for: two copies, the boot menu's Start with a disk missing; then SPARE-6 unplugged
#   degraded    started with it chosen; written to; SPARE-6 back
#   back        the copies put back, nothing counted; then the system on one disk again, as the suites after
#               this one expect it
#   single      the boot menu's entry gone with the second disk
# Before 90-control-switch.sh, which must stay the last suite.
# vmlab-paths: @sf/usr/libexec/amethystora-raid* @sf/usr/lib/systemd/system/amethystora-raid* @sf/usr/lib/dracut/* @sf/usr/libexec/amethystora-boot-guard @sf/usr/lib/systemd/system/amethystora-boot-guard*
source "$(dirname "$0")/lib.sh"

RAID=/usr/libexec/amethystora-raid
TESTS=/var/lib/vmlab-tests
ENV_FILE="${TESTS}/85-raid-mirror.env"
SUDOERS=/etc/sudoers.d/zz-vmlab
AS_TESTER=(runuser -l "${TEST_USER}" -c)

# shellcheck source=/dev/null
[[ -f "${ENV_FILE}" ]] && source "${ENV_FILE}"
remember() {
    printf '%s=%q\n' "$1" "$2" >>"${ENV_FILE}"
    printf -v "$1" '%s' "$2"
}
step() { remember STEP "$1"; }
status_json() { runuser -u "${TEST_USER}" -- "${RAID}" status --json; }
system_json() { status_json | jq -c 'first(.pools[] | select(.system)) // empty'; }
control_json() { as_user "LC_ALL=C.UTF-8 LANGUAGE= /usr/libexec/amethystora-control-list --json"; }
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
# The lab disk's /boot is a folder of its btrfs, bind-mounted read-only
boot_rw() { mount -o "remount,bind,$1" /boot; }
choose_degraded() { boot_rw rw && sed -i '1i set default=amethystora-degraded # vmlab' /boot/grub2/custom.cfg; sync; boot_rw ro; }
unchoose_degraded() { boot_rw rw && sed -i '/# vmlab$/d' /boot/grub2/custom.cfg; sync; boot_rw ro; }

# The account only: everything here runs through runuser, with or without a session (83-logins leaves none)
getent passwd "${TEST_USER}" >/dev/null || { skip "no account ${TEST_USER} (10-account.sh)"; finish; }
[[ "${SUITE_PHASE}" == 1 ]] && { install -d -m 0700 "${TESTS}"; : >"${ENV_FILE}"; remember STEP plug; }
printf '%s ALL=(ALL) NOPASSWD: ALL\n' "${TEST_USER}" >"${SUDOERS}"
chmod 0440 "${SUDOERS}"
echo "# step ${STEP}, phase ${SUITE_PHASE}"

case "${STEP}" in
plug)
    next_boot '{"absent": []}'
    step spread
    reboot_and_continue
    ;;

spread)
    disk="$(readlink -f /dev/disk/by-id/ata-QEMU_HARDDISK_SPARE-6)"
    [[ -b "${disk}" ]] || { not_ok "SPARE-6 is not plugged in"; finish; }
    # What the installer makes of two disks: a partition on the second, joined to the system's btrfs, its data
    # single and its metadata raid1
    printf 'label: gpt\ntype=0FC63DAF-8483-4772-8E79-3D69D8477DE4\n' | sfdisk --quiet "${disk}" >/dev/null
    udevadm settle
    part="$(lsblk -nrpo PATH,TYPE "${disk}" | awk '$2 == "part" { print $1; exit }')"
    btrfs device add -f "${part}" /var >/dev/null && btrfs balance start -f -mconvert=raid1 -sconvert=raid1 /var >/dev/null
    holds "the system is spread over two disks with one copy of each file" "$(system_json)" \
        '(.devices | length) == 2 and .layout == "single" and (.problems | index("unprotected")) != null'
    holds "Control offers Protect the system's disks, and not the mirror onto a new disk" "$(control_json)" \
        'any(.items[]; .id == "raid-mirror" and .terminal and .command == ["ame", "system", "raid", "mirror"])
         and (any(.items[]; .id == "raid-mirror-disk") | not)'
    remember BLOB_SUM "$(dd if=/dev/urandom bs=1M count=256 status=none | tee "/var/home/${TEST_USER}/blob" | sha256sum | cut -d ' ' -f 1)"
    systemd-run --unit=vmlab-mirror --collect --quiet -p StandardOutput="file:${TESTS}/mirror.out" -p StandardError=inherit \
        python3 "$(dirname "$0")/term.py" 3000 'Keep a copy on each disk\?' y -- "${AS_TESTER[@]}" 'ame system raid mirror'
    step mirroring
    continue_phase
    ;;

mirroring)
    if ! wait_for 1500 sh -c '! systemctl is-active --quiet vmlab-mirror'; then
        echo "# the mirror is still converting"
        continue_phase
    fi
    says "ame system raid mirror keeps a copy on each disk" "$(cat "${TESTS}/mirror.out")" \
        'the boot files, /boot and the EFI partition' 'Done: every file of the system is on each of its disks'
    holds "the system is two copies on two disks, fine" "$(system_json)" \
        '.layout == "raid1" and .metadata == "raid1" and .state == "ok" and (.devices | length) == 2'
    check "the boot menu has Start with a disk missing" grep -q "menuentry 'Start with a disk missing'" /boot/grub2/custom.cfg
    check "amethystora-raid-boot runs at each start of a system over several disks" "${RAID}" present-system
    choose_degraded
    next_boot '{"absent": [6]}'
    step degraded
    reboot_and_continue
    ;;

degraded)
    ok "the system started from its first disk with the second unplugged, from Start with a disk missing"
    check "it is mounted degraded" sh -c 'findmnt -rn -o OPTIONS --mountpoint /sysroot | grep -qw degraded'
    holds "the status: a disk of the system is missing, every file still there" "$(system_json)" \
        '.missing == 1 and .survives == 1 and (.problems | index("missing")) != null'
    says "ame system raid status says to replace it" "$(runuser -u "${TEST_USER}" -- "${RAID}" status)" \
        'A disk is missing\. Every file is still there.*ame system raid replace system'
    [[ "$(sha256sum "/var/home/${TEST_USER}/blob" | cut -d ' ' -f 1)" == "${BLOB_SUM}" ]] && ok "a file written before reads the same" ||
        not_ok "a file changed with a disk missing"
    check "${TEST_USER} writes with a disk missing" as_user 'head -c 64M /dev/urandom >~/written-degraded && sync'
    unchoose_degraded
    next_boot '{"absent": []}'
    step back
    reboot_and_continue
    ;;

back)
    holds "SPARE-6 is back, with the old copies of what it missed, and not failing" "$(system_json)" \
        '.missing == 0 and any(.devices[]; .returned) and (.problems | index("returned")) != null and .errors == 0'
    out="$(in_terminal 1500 'Put the copies back now\?' y -- "${AS_TESTER[@]}" 'ame system raid replace system')"
    says "ame system raid replace system puts the copies back" "${out}" 'Every file of (the )?system has its copies again'
    holds "the system is whole: nothing missing, back, counting or with one copy" "$(system_json)" \
        '.state == "ok" and all(.devices[]; (.returned | not) and .error_count == 0) and (.degraded | not)'
    out="$(btrfs scrub start -B /var 2>&1)"
    grep -qE 'Error summary: +no errors found' <<<"${out}" && ok "a second check of every copy finds nothing" ||
        not_ok "the second check found something" "${out}"
    # The system on one disk again, as the suites after this one expect it
    systemd-run --unit=vmlab-single --collect --quiet -p StandardOutput="file:${TESTS}/single.out" bash -c \
        "btrfs balance start -f -dconvert=single -mconvert=dup -sconvert=dup /var && btrfs device remove \$(btrfs filesystem show --raw /var | awk '/SPARE|sd/ { print \$2; exit }') /var"
    step single
    continue_phase
    ;;

single)
    if ! wait_for 1500 sh -c '! systemctl is-active --quiet vmlab-single'; then
        echo "# the system is still going back onto one disk"
        continue_phase
    fi
    [[ -z "$(system_json)" ]] && ok "the system is on one disk again" || not_ok "the system is on several disks still" "$(system_json)"
    systemctl restart amethystora-raid-boot.service
    check "and the boot menu's entry for a disk missing went with the second disk" sh -c \
        "! grep -qs amethystora-degraded /boot/grub2/custom.cfg"
    rm -f "${SUDOERS}"
    finish
    ;;

*) finish ;;
esac
