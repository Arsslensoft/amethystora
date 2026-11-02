#!/usr/bin/bash
# Pools of disks (ame system raid) on the five blank SATA disks the lab adds, SPARE-1 to SPARE-5 (8 GiB, the
# fifth 10 GiB), as plans/done/PLAN-raid.md's "Check on a machine" has them, over ten starts of the machine
# (phases 8 to 10 are one start, split to stay under the lab's 30 minutes a phase):
#   1. parity (raid5, not encrypted) on SPARE-3 to 5, the disks named; data on the two left with Enter, Enter,
#      Enter and ERASE (two copies, encrypted). Their lines, key, owner, and Files showing them.
#   2. Both back after a restart, data unlocked by its key with nothing asked; the recovery key; the status
#      without root, and its speed; no TPM unlock for a pool; Control's words; a scrub cut short, then
#      resumed; a disk of parity taken away while mounted, and what the hourly check makes of it.
#   3. The disk back; parity forgotten, its files left; joined (combined, encrypted) made on two of its disks.
#   4. joined back after the restart, then forgotten; data filled, and one of its disks made to vanish.
#   5. data did not open and the boot did not wait for it; opened with a disk missing, written to; a replace
#      refused while it is too full, one stopped part way; the disk replaced by SPARE-5, its copies put back.
#   6. data after the restart checked clean; SPARE-4 added, then pulled out while mounted (encrypted).
#   7. SPARE-4 back: back, not failing, its copies put back. A file's copy given bad sectors for the next start.
#   8. The damaged disk: the file reads right, a scrub repairs its copy and counts the errors, the notification's
#      Open Control clicked, the disk replaced by SPARE-3 while it still reads.
#   9. A copy overwritten behind btrfs's back repaired by a scrub.
#  10. A suspend (the kernel's own test of it, to the devices) cuts a scrub short, which the next run resumes. A
#      disk of data unplugged.
#  11. data did not open, and the boot did not wait; opened with the disk gone, written to; the disk plugged in.
#  12. It is back and its copies put back; data forgotten, leaving nothing behind.
# The script asks sudo for each step that needs root, so tester's sudo asks no password while this suite runs.
# vmlab-paths: @sf/usr/libexec/amethystora-raid* @sf/usr/lib/systemd/system/amethystora-raid* @sf/usr/libexec/amethystora-disk-health @sf/usr/lib/systemd/system/amethystora-disk-health*
source "$(dirname "$0")/lib.sh"

RAID=/usr/libexec/amethystora-raid
STATE=/var/lib/amethystora/raid
TESTS=/var/lib/vmlab-tests
ENV_FILE="${TESTS}/50-raid.env"
SUDOERS=/etc/sudoers.d/zz-vmlab
OPTIONS=compress=zstd:1,nofail,x-systemd.device-bound=no,x-systemd.device-timeout=30s,x-gvfs-show,x-amethystora-raid
CRYPT_OPTIONS=luks,discard,nofail,x-systemd.device-timeout=30s
# systemd's recovery-key form: modhex, eight groups of eight
RECOVERY='[cbdefghijklnrtuv]{8}(-[cbdefghijklnrtuv]{8}){7}'
AS_TESTER=(runuser -l "${TEST_USER}" -c)

# What one phase leaves the next
# shellcheck source=/dev/null
[[ -f "${ENV_FILE}" ]] && source "${ENV_FILE}"
remember() {
    printf '%s=%q\n' "$1" "$2" >>"${ENV_FILE}"
    printf -v "$1" '%s' "$2"
}

# SPARE-N's /dev/sdX, which can change from one start to the next
spare() { readlink -f "/dev/disk/by-id/ata-QEMU_HARDDISK_SPARE-$1"; }
# A pool's partitions, "path raid-NAME-N" a line
parts_of() { lsblk -nrpo PATH,PARTLABEL | awk -v prefix="raid-$1-" 'index($2, prefix) == 1 { print $1, $2 }'; }
crypt_unit() { printf 'systemd-cryptsetup@%s.service\n' "$(systemd-escape "$1")"; }
# The status as Control reads it: the account's, without root
status_json() { runuser -u "${TEST_USER}" -- "${RAID}" status --json; }
pool_json() { status_json | jq -c --arg name "$1" '.pools[] | select(.name == $name)'; }
# Control's list as its window gets it, in English, and a pool's row of its Disks card
control_json() { as_user "LC_ALL=C.UTF-8 LANGUAGE= /usr/libexec/amethystora-control-list --json"; }
row_of() { jq -c --arg name "$2" '.items[] | select(.kind == "disks") | .disks[] | select(.id == $name)' <<<"$1"; }
run_check() { systemctl start amethystora-raid-check.service; }
check_skipped() { [[ "$(systemctl show amethystora-raid-check.service -p Result --value)" == exec-condition ]]; }
scrub_now_done() { ! systemctl is-active --quiet amethystora-raid-scrub-now.service; }
told_in_unit() { systemctl list-units --no-legend --plain --type=service | grep -qF 'Tell about the pools of disks'; }
# Seconds from the start of the machine to the desktop's target
graphical_at() { echo $(($(systemctl show graphical.target -p ActiveEnterTimestampMonotonic --value) / 1000000)); }
ms() {
    local start
    start="$(date +%s%N)"
    "$@" >/dev/null 2>&1
    echo $((($(date +%s%N) - start) / 1000000))
}

# holds WHAT JSON FILTER: ok when the jq filter is true of the JSON, not ok showing the JSON
holds() {
    if jq -e "$3" <<<"$2" >/dev/null 2>&1; then ok "$1"; else not_ok "$1" "${2:-(nothing)}"; fi
}
# says WHAT OUTPUT REGEX...: ok when every regular expression is in what a command printed
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
has_line() { grep -qxF -- "$2" "$1" 2>/dev/null || { grep -v '^#' "$1" 2>&1 | tail -n 10; false; }; }
# A pool forgotten leaves nothing on the system disk: no fstab or crypttab line, key, folder or open disk
forgotten() {
    local left
    left="$(grep -s " /var/mnt/$1 " /etc/fstab; grep -s "^raid-$1-" /etc/crypttab
        ls -d "/etc/cryptsetup-keys.d/raid-$1.key" "/var/mnt/$1" /dev/mapper/raid-"$1"-* 2>/dev/null)"
    [[ -z "${left}" ]] || { printf '%s\n' "${left}"; return 1; }
}
nothing_left() {
    local left
    left="$(grep -s x-amethystora-raid /etc/fstab; grep -s '^raid-' /etc/crypttab
        ls -d /etc/cryptsetup-keys.d/raid-*.key /dev/mapper/raid-* 2>/dev/null)"
    [[ -z "${left}" ]] || { printf '%s\n' "${left}"; return 1; }
}
# The recovery key opens PARTITION from slot 1, typed as at systemd's prompt (a line on stdin), or as the
# bytes of a key file, which is how luksAddKey was given it
opens_typed() { printf '%s\n' "$2" | cryptsetup open --test-passphrase --key-slot 1 "$1"; }
opens_as_file() { printf '%s' "$2" | cryptsetup open --test-passphrase --key-slot 1 --key-file - "$1"; }
# The check's notification, waiting in the test account's session with its Open Control button: its summary,
# and something its body names
notified() {
    pgrep -u "${TEST_USER}" -af notify-send | grep -F -- '--action=open=Open Control' | grep -F -- "$1" | grep -qF -- "${2:-$1}"
}

# Where a copy of a file's first block is, on a disk of the pool: "SPARE-N SECTOR...", its eight 512-byte sectors
# on that disk, or with "bytes" "SPARE-N OFFSET", its byte offset. btrfs-map-logical gives each copy's device and
# offset on it; under LUKS the data starts after its header, and the partition where the table says. Never on a disk
# SKIP names ("SPARE-3 SPARE-5").
bad_place() {  # FILE [bytes]
    local logical member line device offset part disk serial luks start
    logical="$(filefrag -v "$1" 2>/dev/null | awk '$1 == "0:" { sub(/\.\..*/, "", $4); print $4 * 4096; exit }')"
    member="$(btrfs filesystem show --raw "$(dirname "$1")" 2>/dev/null | awk '$1 == "devid" && $NF ~ /^\// { print $NF; exit }')"
    [[ -n "${logical}" && -n "${member}" ]] || return 1
    while read -r line; do
        device="$(awk '{ for (i = 1; i < NF; i++) if ($i == "device") print $(i + 1) }' <<<"${line}")"
        offset="$(awk '{ for (i = 1; i < NF; i++) if ($i == "physical") print $(i + 1) }' <<<"${line}")"
        [[ -n "${device}" && -n "${offset}" ]] || continue
        device="$(readlink -f "${device}")"
        part="$(find "/sys/class/block/${device##*/}/slaves" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | head -n 1)"
        luks=0
        if [[ -n "${part}" ]]; then
            luks="$(cryptsetup luksDump "/dev/${part}" | awk '/^Data segments/ { s = 1 } s && $1 == "offset:" { print $2; exit }')"
        else
            part="${device##*/}"
        fi
        disk="$(lsblk -ndo PKNAME "/dev/${part}")"
        serial="$(lsblk -ndo SERIAL "/dev/${disk}")"
        start="$(cat "/sys/class/block/${part}/start")"
        [[ "${serial}" == SPARE-* && " ${SKIP:-} " != *" ${serial} "* ]] || continue
        offset=$((start * 512 + luks + offset))
        if [[ "${2:-}" == bytes ]]; then
            echo "${serial} ${offset}"
        else
            echo "${serial} $(seq -s ' ' $((offset / 512)) $((offset / 512 + 7)))"
        fi
        return 0
    done < <(btrfs-map-logical -l "${logical}" "${member}" 2>/dev/null)
    return 1
}
# A file's checksum, read in the background so that a read the kernel never finishes (a process in D state, which
# no signal ends) cannot hold the suite: nothing, and false, after SECONDS
bounded_sum() {  # FILE SECONDS
    local out pid i
    out="$(mktemp)"
    # Nothing of the suite's own output held by it, or the lab's helper would wait on it all the same
    (sha256sum "$1" >"${out}" 2>&1) </dev/null >/dev/null 2>&1 &
    pid=$!
    for ((i = 0; i < $2; i++)); do
        kill -0 "${pid}" 2>/dev/null || break
        sleep 1
    done
    if kill -0 "${pid}" 2>/dev/null; then
        rm -f "${out}"
        return 1
    fi
    cut -d ' ' -f 1 "${out}"
    rm -f "${out}"
}
# GNOME Shell, and an app's window, through the accessibility tree, as the account (atspi.py)
ATSPI=/run/vmlab-raid/atspi.py
install -D -m 0755 "$(dirname "$0")/atspi.py" "${ATSPI}"
shell() { user_bus python3 "${ATSPI}" gnome-shell "$@"; }
shell_app() { user_bus python3 "${ATSPI}" "$@"; }

# For a phase that cannot go on: what is left of the pools undone by hand, and the end of the suite
abandon() {
    local name mapping
    skip "$1"
    for name in data parity joined; do
        umount "/var/mnt/${name}" 2>/dev/null
        rmdir "/var/mnt/${name}" 2>/dev/null
    done
    for mapping in /dev/mapper/raid-*; do
        [[ -e "${mapping}" ]] && cryptsetup close "${mapping##*/}"
    done
    sed -i '/x-amethystora-raid/d' /etc/fstab
    [[ -f /etc/crypttab ]] && sed -i '/^raid-/d' /etc/crypttab
    rm -f /etc/cryptsetup-keys.d/raid-*.key "${SUDOERS}" /etc/udev/rules.d/99-vmlab-damaged.rules
    systemctl daemon-reload
    finish
}

# The notifications need the session, which GDM brings back after each restart
wait_for 300 has_session || abandon "no session for ${TEST_USER} (10-account.sh)"
[[ "${SUITE_PHASE}" == 1 || -s "${ENV_FILE}" ]] || abandon "the first phase left nothing in ${ENV_FILE}"

case "${SUITE_PHASE}" in
1)
    install -d -m 0700 "${TESTS}"
    : >"${ENV_FILE}"
    printf '%s ALL=(ALL) NOPASSWD: ALL\n' "${TEST_USER}" >"${SUDOERS}"
    chmod 0440 "${SUDOERS}"
    check "${TEST_USER}'s sudo asks no password while this suite runs" sh -c "visudo -cqf ${SUDOERS} && runuser -u ${TEST_USER} -- sudo -n true"

    # On in every image, and costing nothing without a pool: the check stops at its ExecCondition
    for timer in amethystora-raid-check.timer amethystora-raid-scrub.timer; do
        check "${timer} is enabled" systemctl is-enabled --quiet "${timer}"
    done
    echo "# the check with no pool: $(ms run_check) ms"
    if check_skipped; then
        ok "with no pool, the check stops at its ExecCondition"
    else
        not_ok "with no pool, the check ran" "$(journalctl -b -u amethystora-raid-check.service --no-pager -o cat | tail -n 10)"
    fi
    holds "the five blank disks are offered, and nothing of the system's" "$(status_json)" \
        '.pools == [] and ([.spare[].serial] | sort) == ["SPARE-1", "SPARE-2", "SPARE-3", "SPARE-4", "SPARE-5"]'

    # parity: the disks named, then the third layout and the second answer (not encrypted)
    out="$(in_terminal 600 'Which layout\?' $'\e[B\e[B\r' 'Encrypt the pool\?' $'\e[B\r' 'Type ERASE' $'ERASE\r' \
        -- "${AS_TESTER[@]}" "ame system raid create --name parity $(spare 3) $(spare 4) $(spare 5)")"
    says "create makes parity of SPARE-3, 4 and 5, shown with btrfs's warning first" "${out}" \
        'should not be used in production' 'Layout: Parity \(data raid5, metadata raid1c3\)' 'Encrypted: no' \
        'The pool parity is ready at /var/mnt/parity'
    grep -q ' /var/mnt/parity ' /etc/fstab || abandon "parity was not made, so the rest cannot follow"

    # data: nothing named, every answer as it comes
    out="$(in_terminal 600 'Which disks\?' $'\r' 'Which layout\?' $'\r' 'Encrypt the pool\?' $'\r' 'Type ERASE' $'ERASE\r' \
        -- "${AS_TESTER[@]}" 'ame system raid create')"
    says "Enter, Enter, Enter and ERASE make data: both spare disks, two copies, encrypted" "${out}" \
        'Everything on these 2 disks is erased' 'Layout: Two copies \(data raid1, metadata raid1\)' 'Encrypted: yes' \
        'Name: data, at /var/mnt/data' 'The pool data is ready at /var/mnt/data'
    says "create says what the encryption protects with the system disk not encrypted" "${out}" \
        'kept on the system disk, which is not encrypted'
    says "create says a pool is not a backup" "${out}" 'A pool is not a backup'
    if [[ "$(grep -oE "${RECOVERY}" <<<"${out}" | wc -l)" -eq 1 ]]; then
        ok "create shows the recovery key once, in systemd's form"
        remember DATA_KEY "$(grep -oE "${RECOVERY}" <<<"${out}")"
    else
        not_ok "create did not show one recovery key" "$(tail -n 15 <<<"${out}")"
    fi
    grep -q ' /var/mnt/data ' /etc/fstab || abandon "data was not made, so the rest cannot follow"

    status="$(status_json)"
    remember PARITY_UUID "$(jq -r '.pools[] | select(.name == "parity") | .uuid' <<<"${status}")"
    remember DATA_UUID "$(jq -r '.pools[] | select(.name == "data") | .uuid' <<<"${status}")"
    holds "parity: raid5 data, raid1c3 metadata, on SPARE-3, 4 and 5, not encrypted" "$(pool_json parity)" \
        '.mounted and .layout == "raid5" and .metadata == "raid1c3" and (.encrypted | not)
         and ([.devices[].disk.serial] | sort) == ["SPARE-3", "SPARE-4", "SPARE-5"]'
    holds "data: raid1 data and metadata, on SPARE-1 and 2, encrypted" "$(pool_json data)" \
        '.mounted and .layout == "raid1" and .metadata == "raid1" and .encrypted
         and ([.devices[].disk.serial] | sort) == ["SPARE-1", "SPARE-2"]'
    check "parity's line in /etc/fstab" has_line /etc/fstab "UUID=${PARITY_UUID} /var/mnt/parity btrfs ${OPTIONS} 0 0"
    check "data's line in /etc/fstab" has_line /etc/fstab "UUID=${DATA_UUID} /var/mnt/data btrfs ${OPTIONS} 0 0"
    check "parity has no line in /etc/crypttab, and no key" sh -c '! grep -qs "^raid-parity-" /etc/crypttab && [ ! -e /etc/cryptsetup-keys.d/raid-parity.key ]'
    for name in parity data; do
        owner="$(stat -c %U "/var/mnt/${name}")"
        [[ "${owner}" == "${TEST_USER}" ]] && ok "/var/mnt/${name} belongs to ${TEST_USER}, who made it" ||
            not_ok "/var/mnt/${name} belongs to ${owner}"
        check "${TEST_USER} writes in /var/mnt/${name}" as_user "echo ${name} >/var/mnt/${name}/marker"
    done

    # data's disks: one partition each, LUKS2 labelled with the pool, a crypttab line each with the one key
    [[ "$(parts_of data | wc -l)" -eq 2 ]] && ok "data's two disks each have a partition raid-data-N" ||
        not_ok "data's partitions are not raid-data-1 and 2" "$(lsblk -o NAME,PARTLABEL,FSTYPE,LABEL)"
    while read -r part label; do
        check "${label}'s line in /etc/crypttab: the pool's key file, nofail, 30 s" \
            has_line /etc/crypttab "${label} UUID=$(cryptsetup luksUUID "${part}") /etc/cryptsetup-keys.d/raid-data.key ${CRYPT_OPTIONS}"
        found="$(blkid -p -s LABEL -o value "${part}")"
        [[ "${found}" == raid-data ]] && cryptsetup isLuks --type luks2 "${part}" && ok "${label} is LUKS2 labelled raid-data" ||
            not_ok "${label} is not LUKS2 labelled raid-data (${found})"
    done < <(parts_of data)
    key=/etc/cryptsetup-keys.d/raid-data.key
    found="$(stat -c '%a %U %s' "${key}" 2>&1)"
    [[ "${found}" == "400 root 64" ]] && ok "data's key is 64 bytes, root's, mode 0400" || not_ok "data's key is ${found} (mode, owner, bytes)"
    relabel="$(restorecon -n -v /etc/cryptsetup-keys.d "${key}" 2>&1)"
    if [[ -z "${relabel}" ]]; then ok "data's key carries the label SELinux's policy gives it ($(stat -c %C "${key}"))"; else not_ok "data's key is not labelled as the policy says" "${relabel}"; fi

    gio_shows() { in_session "gio mount -l" | grep -qF "file:///var/mnt/$1"; }
    for name in parity data; do
        check "Files shows ${name} (x-gvfs-show)" wait_for 60 gio_shows "${name}"
    done
    reboot_and_continue
    ;;

2)
    for name in data parity; do
        check "${name} is mounted after the restart" findmnt -rn --mountpoint "/var/mnt/${name}"
    done
    # data opened at boot by systemd-cryptsetup with the key file: nothing asked, nothing denied
    while read -r _ label; do
        unit="$(crypt_unit "${label}")"
        check "${label} was unlocked at boot" systemctl is-active --quiet "${unit}"
        asked="$(journalctl -b -u "${unit}" -o cat | grep -i passphrase)"
        [[ -z "${asked}" ]] && ok "${label} asked for nothing at boot" || not_ok "${label} asked for a passphrase at boot" "${asked}"
    done < <(parts_of data)
    denied="$({ ausearch -m AVC,USER_AVC -ts boot -i 2>/dev/null; journalctl -b -o cat -g 'avc: +denied' 2>/dev/null; } |
        grep -E 'cryptsetup-keys|raid-data')"
    [[ -z "${denied}" ]] && ok "SELinux denied nothing on data's key at boot" || not_ok "SELinux denied something on data's key at boot" "${denied}"

    # One recovery key opens every disk of data, typed at a prompt or given as a key file
    while read -r part label; do
        check "the recovery key, typed, opens ${label}" opens_typed "${part}" "${DATA_KEY}"
        check "the recovery key, as --key-file -, opens ${label}" opens_as_file "${part}" "${DATA_KEY}"
    done < <(parts_of data)

    # The status reads which disk is which without root, and fast enough for Control
    user_show() {  # NAME DISKS
        local out
        out="$(runuser -u "${TEST_USER}" -- btrfs filesystem show --raw "/var/mnt/$1" 2>&1)"
        [[ "$(grep -c ' path /dev/' <<<"${out}")" -eq "$2" ]] || { printf '%s\n' "${out}"; return 1; }
    }
    check "btrfs filesystem show lists data's two disks for ${TEST_USER}" user_show data 2
    check "btrfs filesystem show lists parity's three disks for ${TEST_USER}" user_show parity 3
    status_json >/dev/null
    mapfile -t times < <(for _ in 1 2 3; do ms status_json; done | sort -n)
    echo "# status --json with two pools, as ${TEST_USER}: ${times[*]} ms"
    ((times[1] < 1000)) && ok "status --json answers well inside Control's second (${times[1]} ms)" ||
        not_ok "status --json takes ${times[1]} ms, more than Control's second"
    if ((times[1] >= 1000)); then
        # Where the time went: each step of the status traced with its start, the slowest first
        runuser -u "${TEST_USER}" -- env PS4='+ ${EPOCHREALTIME} ' bash -x "${RAID}" status --json 2>&1 >/dev/null |
            awk 'match($0, /^\++ [0-9]+\.[0-9]+ /) { split(substr($0, 1, RLENGTH), f, " "); t = f[2]
                    if (prev != "") printf "%6.0f ms  %s\n", (t - prev) * 1000, substr(line, 1, 140)
                    prev = t; line = substr($0, RLENGTH + 1) }' | sort -rn | head -n 12 | sed 's/^/# slow: /'
    fi

    # TPM unlock is for the system's disk, and an encrypted pool is not that
    luks="$("${RAID}" root-luks)"
    [[ -z "${luks}" ]] && ok "root-luks names nothing beside an encrypted pool" || not_ok "root-luks names ${luks}"
    holds "Security's disk-unlock tile says the system's disk is not encrypted" \
        "$(LC_ALL=C.UTF-8 LANGUAGE='' /usr/libexec/amethystora-security-status --json | jq -c '.checks[] | select(.id == "disk-unlock")')" \
        ".state == \"unavailable\" and any(.details[]; . == \"The system's disk is not encrypted\")"
    says "ame security disk-unlock finds nothing to enrol" "$(as_user 'ame security disk-unlock </dev/null' 2>&1)" \
        'The system is not on an encrypted partition'
    skip "disk-unlock enrolling the system's disk only, beside an encrypted pool: this machine's system disk is not encrypted"

    # A scrub cut short is resumed at the next run. A suspend cuts it short since 6.19; the lab has no
    # suspend, so it is cancelled, which btrfs reports the same way.
    check "${TEST_USER} writes 2 GiB to parity" as_user 'head -c 2G /dev/urandom >/var/mnt/parity/blob && sync'
    systemctl start amethystora-raid-scrub.service &
    job=$! cancelled=false
    while kill -0 "${job}" 2>/dev/null; do
        if [[ "${cancelled}" == false ]] && pgrep -f 'btrfs scrub start -B -R /var/mnt/parity' >/dev/null &&
            btrfs scrub cancel /var/mnt/parity >/dev/null 2>&1; then
            cancelled=true
        fi
        sleep 0.2
    done
    wait "${job}"
    file="${STATE}/scrub-${PARITY_UUID}.json"
    if [[ "${cancelled}" != true ]]; then
        skip "a scrub cut short, then resumed: parity's scrub could not be caught running"
    else
        holds "a scrub cut short is written down as not finished" "$(cat "${file}" 2>&1)" '.finished == false'
        systemctl start amethystora-raid-scrub.service &
        job=$! seen=""
        while kill -0 "${job}" 2>/dev/null; do
            seen+="$(pgrep -af 'btrfs scrub (start|resume) -B -R /var/mnt/parity' | grep -oE 'scrub (start|resume)')"$'\n'
            sleep 0.2
        done
        wait "${job}"
        if grep -qx 'scrub resume' <<<"${seen}" && ! grep -qx 'scrub start' <<<"${seen}"; then
            ok "the next run resumes it"
        else
            not_ok "the next run did not resume it" "seen: $(sort -u <<<"${seen}" | paste -sd ' ')" \
                "$(journalctl -b -u amethystora-raid-scrub.service --no-pager -o cat | tail -n 15)"
        fi
        holds "and writes it down as finished, with nothing it could not repair" "$(cat "${file}" 2>&1)" '.finished and .uncorrectable == 0'
    fi
    holds "data's scrub is clean" "$(cat "${STATE}/scrub-${DATA_UUID}.json" 2>&1)" '.finished and .uncorrectable == 0 and .corrected == 0'

    # Control: each layout's words, and what each card runs
    control="$(control_json)"
    holds "Control says data is two copies on two disks, encrypted, checked" "$(row_of "${control}" data)" \
        '.state == "ok" and (.text | contains("Two copies, on 2 disks.") and contains("Encrypted.") and contains("Every copy last checked on"))'
    holds "Control says parity is parity on three disks, with the advice after a power cut" "$(row_of "${control}" parity)" \
        '.text | contains("Parity, on 3 disks.") and contains("After a power cut, check every copy.") and (contains("Encrypted.") | not)'
    holds "Control's Forget runs ame system raid forget NAME in a terminal, for each pool" "${control}" \
        '[.items[] | select(.kind == "disks") | .disks[] | .id as $id | .actions[] | select(.id == "forget")
          | .terminal and .command == ["ame", "system", "raid", "forget", $id]] | length == 2 and all'
    holds "Control makes no pool with no disk to spare" "${control}" \
        "any(.items[]; .id == \"raid-create\" and (.available | not) and .reason == \"Needs two disks besides the system's.\")"
    holds "Control: the monthly check is on, and Check every copy now runs ame system raid scrub now" "${control}" \
        'any(.items[]; .id == "raid-scrub" and .state == "on")
         and any(.items[]; .id == "raid-scrub-now" and .command == ["ame", "system", "raid", "scrub", "now"])'

    # A disk of parity taken away while mounted. The lab cannot unplug one, so the kernel is told it is gone.
    run_check
    holds "the check writes down every disk of each pool by its serial (disks.json)" "$(cat "${STATE}/disks.json" 2>&1)" \
        '([.devices[].serial] | sort) == ["SPARE-1", "SPARE-2", "SPARE-3", "SPARE-4", "SPARE-5"]'
    devid="$(pool_json parity | jq -r '.devices[] | select(.disk.serial == "SPARE-3") | .devid')"
    gone="$(spare 3)"
    since="$(date '+%F %T')"
    echo 1 >"/sys/block/${gone##*/}/device/delete"
    check "parity takes a write with SPARE-3 gone" as_user 'echo after >/var/mnt/parity/after && sync'
    sleep 5
    kernel="$(journalctl -k --since "${since}" --no-pager -o cat | grep -i btrfs)"
    grep -qiE 'degraded|missing' <<<"${kernel}" && ok "the kernel says parity goes on without SPARE-3" ||
        not_ok "the kernel did not say parity goes on degraded" "${kernel:-(no btrfs line since the disk went)}"
    holds "the status reads disk ${devid} of parity as missing, and names it SPARE-3" "$(pool_json parity)" \
        '.missing == 1 and any(.problems[]; . == "missing") and any(.devices[]; .missing and .disk.serial == "SPARE-3")'
    run_check
    holds "the check writes it into last.json" "$(cat "${STATE}/last.json" 2>&1)" ".[\"${PARITY_UUID}/missing/${devid}\"] == 1"
    check "the check tells ${TEST_USER}: A disk of the pool parity is missing, naming SPARE-3, with Open Control" \
        wait_for 60 notified "A disk of the pool parity is missing" "SPARE-3"
    check "the notification waits for its answer in a unit of its own" told_in_unit
    holds "Control says a disk of parity is missing with every file still there, and offers Replace a disk" "$(row_of "$(control_json)" parity)" \
        '.state == "check" and (.text | contains("A disk is missing. Every file is still there"))
         and any(.actions[]; .id == "replace" and .terminal and .command == ["ame", "system", "raid", "replace", "parity"])'
    # Its Open Control button is clicked in phase 8
    reboot_and_continue
    ;;

3)
    grep -q ' /var/mnt/parity ' /etc/fstab || abandon "parity is not there"
    check "SPARE-3 is back after the restart" test -b "$(spare 3)"
    holds "parity is mounted with its three disks again" "$(pool_json parity)" '.mounted and .missing == 0 and (.devices | length) == 3'
    echo "# parity with SPARE-3 back: $(pool_json parity | jq -c '{problems, errors, degraded}')"

    # Forget: the lines go, the files stay on the disks, and the disks are spare again
    out="$(in_terminal 120 'Forget parity\?' y -- "${AS_TESTER[@]}" 'ame system raid forget parity')"
    says "ame system raid forget parity" "${out}" 'parity is forgotten'
    check "nothing of parity is left: fstab line, folder" forgotten parity
    mkdir -p /run/vmlab-parity
    if mount -o ro "UUID=${PARITY_UUID}" /run/vmlab-parity; then
        [[ "$(cat /run/vmlab-parity/marker 2>&1)" == parity ]] && ok "parity's files are still on its disks" ||
            not_ok "parity's disks mount, but its file is not what was written"
        umount /run/vmlab-parity
    else
        not_ok "parity's disks no longer mount"
    fi
    holds "its disks are spare again, though they hold the forgotten pool" "$(status_json)" \
        '([.spare[].serial] | sort) == ["SPARE-3", "SPARE-4", "SPARE-5"]'

    # joined: two of them named, the second layout (Combined), encrypted
    out="$(in_terminal 600 'Which layout\?' $'\e[B\r' 'Encrypt the pool\?' $'\r' 'Type ERASE' $'ERASE\r' \
        -- "${AS_TESTER[@]}" "ame system raid create --name joined $(spare 3) $(spare 4)")"
    says "create shows what a disk holds before it is erased: the forgotten parity" "${out}" 'now: btrfs "parity"'
    says "create makes joined, combined, with its warning" "${out}" 'No copies\. This joins the disks' \
        'The pool joined is ready at /var/mnt/joined'
    key="$(grep -oE "${RECOVERY}" <<<"${out}" | head -n 1)"
    [[ -n "${key}" ]] && remember JOINED_KEY "${key}"
    holds "joined: single data, raid1 metadata, encrypted, on SPARE-3 and 4" "$(pool_json joined)" \
        '.mounted and .layout == "single" and .metadata == "raid1" and .encrypted
         and ([.devices[].disk.serial] | sort) == ["SPARE-3", "SPARE-4"]'
    reboot_and_continue
    ;;

4)
    if grep -q ' /var/mnt/joined ' /etc/fstab; then
        check "joined is mounted after the restart" findmnt -rn --mountpoint /var/mnt/joined
        holds "Control says joined is combined, on two disks, encrypted, with no check of copies it has not got" \
            "$(row_of "$(control_json)" joined)" \
            '.text | contains("Combined, on 2 disks.") and contains("Encrypted.") and (contains("Every copy") | not)'
        # Forget an encrypted pool: its key goes too, and the recovery key still opens its disks
        out="$(in_terminal 120 'Forget joined\?' y -- "${AS_TESTER[@]}" 'ame system raid forget joined')"
        says "forgetting joined says the recovery key is then the only way in" "${out}" \
            'the recovery key is the only way into its disks' 'joined is forgotten'
        check "nothing of joined is left: fstab and crypttab lines, key, folder, open disks" forgotten joined
        while read -r part label; do
            check "the recovery key still opens ${label}" opens_typed "${part}" "${JOINED_KEY}"
        done < <(parts_of joined)
    else
        skip "joined was not made in the third phase"
    fi

    grep -q ' /var/mnt/data ' /etc/fstab || abandon "data is not there"
    check "data is mounted" findmnt -rn --mountpoint /var/mnt/data
    # More than the first gigabyte of chunks mkfs gave it, as a pool in use has, and with what is written with a
    # disk missing still leaving the disk that is left room: btrfs replace needs some to set block groups
    # read-only, and 3 GiB then 1.2 GiB on these 8 GiB disks once ran it out (ENOSPC)
    check "${TEST_USER} writes 2 GiB to data" as_user 'head -c 2G /dev/urandom >/var/mnt/data/blob && sync'
    remember BLOB_SUM "$(sha256sum /var/mnt/data/blob | cut -d ' ' -f 1)"
    run_check
    holds "disks.json names both disks of data" "$(cat "${STATE}/disks.json" 2>&1)" \
        "[.devices[] | select(.fsid == \"${DATA_UUID}\") | .serial] | sort == [\"SPARE-1\", \"SPARE-2\"]"
    # raid-data-2's disk vanishes at the next start: its LUKS header zeroed, while its open mapping keeps the
    # pool going until then
    read -r part _ < <(parts_of data | awk '$2 == "raid-data-2"')
    [[ -b "${part}" ]] || abandon "data has no partition raid-data-2 to take away"
    remember MISSING_LUKS "$(cryptsetup luksUUID "${part}")"
    remember MISSING_SERIAL "$(lsblk -ndo SERIAL "/dev/$(lsblk -ndo PKNAME "${part}")")"
    remember REF_BOOT "$(graphical_at)"
    dd if=/dev/zero of="${part}" bs=1M count=16 oflag=direct conv=fsync status=none
    udevadm settle
    check "raid-data-2 (${MISSING_SERIAL}) has no LUKS header now" sh -c "! cryptsetup isLuks '${part}'"
    reboot_and_continue
    ;;

5)
    grep -q ' /var/mnt/data ' /etc/fstab || abandon "data is not there"
    # The boot went on without the disk: nofail on both lines, the wait for it 30 s
    booted="$(graphical_at)"
    waited="$(journalctl -b -o short-monotonic --no-pager | grep -F 'Timed out waiting for device' |
        grep -F -e "${MISSING_LUKS}" -e "${MISSING_LUKS//-/\\x2d}" | head -n 1)"
    echo "# desktop reached ${booted} s after the start, ${REF_BOOT} s with every disk there; ${waited:-no time-out logged for the missing disk}"
    if ((booted - REF_BOOT <= 30)); then
        ok "the missing disk held the boot up 30 s at most ($((booted - REF_BOOT)) s)"
    else
        not_ok "the boot took $((booted - REF_BOOT)) s longer with a disk of data missing" "$(systemd-analyze critical-chain graphical.target 2>&1 | head -n 20)"
    fi

    # data did not open, and everything that reads it says so
    check "data is not mounted" sh -c '! findmnt -rn --mountpoint /var/mnt/data'
    holds "the status says data is not mounted" "$(pool_json data)" '(.mounted | not) and .problems == ["unmounted"]'
    says "ame system raid status says how to open it" "$(runuser -u "${TEST_USER}" -- "${RAID}" status)" \
        'Not mounted, perhaps because one of its disks is missing\. Open it with what is there: ame system raid mount data'
    holds "Control says data is not open, and Open it with a disk missing runs ame system raid mount data" "$(row_of "$(control_json)" data)" \
        '(.text | contains("Not open, perhaps because one of its disks is missing."))
         and any(.actions[]; .id == "mount" and .terminal and .command == ["ame", "system", "raid", "mount", "data"])'
    run_check
    holds "the check writes it into last.json" "$(cat "${STATE}/last.json" 2>&1)" ".[\"${DATA_UUID}/unmounted\"] == 1"
    check "the check tells ${TEST_USER}: The pool data did not open" wait_for 60 notified "The pool data did not open"

    # Open it with a disk missing: degraded, never in fstab, and writable
    says "ame system raid mount data opens it with a disk missing" "$(as_user 'ame system raid mount data' 2>&1)" \
        'data is open at /var/mnt/data with a disk missing'
    findmnt -rn --mountpoint /var/mnt/data >/dev/null || abandon "data could not be opened with a disk missing"
    check "it is mounted degraded" sh -c 'findmnt -rno OPTIONS --mountpoint /var/mnt/data | grep -qw degraded'
    check "degraded is not written into /etc/fstab" has_line /etc/fstab "UUID=${DATA_UUID} /var/mnt/data btrfs ${OPTIONS} 0 0"
    check "the file written at the start is there" sh -c '[ "$(cat /var/mnt/data/marker)" = data ]'
    check "writing works: ${TEST_USER} writes 512 MiB" as_user 'head -c 512M /dev/urandom >/var/mnt/data/degraded && sync'
    degraded_sum="$(sha256sum /var/mnt/data/degraded | cut -d ' ' -f 1)"
    # What was written went into new chunks of one copy only where the two-copy ones had no room left: the status
    # says so exactly when there are some
    echo "# chunks of one copy after the write: $(pool_json data | jq -c .degraded)"
    holds "the status: a disk missing, still two copies, and says so of what was written with one" "$(pool_json data)" \
        '.mounted and .missing == 1 and .layout == "raid1" and any(.problems[]; . == "missing")
         and (.degraded == any(.problems[]; . == "degraded"))'
    run_check
    check "the check tells ${TEST_USER}: A disk of the pool data is missing, naming ${MISSING_SERIAL}" \
        wait_for 60 notified "A disk of the pool data is missing" "${MISSING_SERIAL}"

    # A pool too full to be rebuilt is refused before anything is erased: the disk that is left filled to under a
    # gigabyte unallocated, which btrfs replace needs to set block groups read-only (ENOSPC without), then emptied
    unallocated() {
        btrfs device usage -b /var/mnt/data 2>/dev/null | awk '/^\/.*, ID: / { there = 1; next } /^[^[:space:]]/ { there = 0 }
            there && $1 == "Unallocated:" { print $2 }' | sort -n | head -n 1
    }
    room="$(unallocated)" before="$(lsblk -nro NAME,PARTLABEL "$(spare 5)" | paste -sd ' ')"
    for ((i = 0; i < 40 && $(unallocated) >= 900 * 1048576; i++)); do
        as_user "fallocate -l 256M /var/mnt/data/filler-${i}" || break
    done
    echo "# unallocated on the disk that is left: $((${room:-0} / 1048576)) MiB, filled to $(($(unallocated) / 1048576)) MiB"
    out="$(in_terminal 300 -- "${AS_TESTER[@]}" 'ame system raid replace data')"
    if grep -qF 'data is too full to be rebuilt' <<<"${out}" && ! grep -qF 'Type ERASE' <<<"${out}" &&
        [[ "$(lsblk -nro NAME,PARTLABEL "$(spare 5)" | paste -sd ' ')" == "${before}" ]]; then
        ok "a pool too full to be rebuilt is refused before any disk is erased"
    else
        not_ok "replace on a pool too full to be rebuilt" "$(tail -n 15 <<<"${out}")"
    fi
    rm -f /var/mnt/data/filler-* && sync

    # Replace: once only to see which disks are offered (Ctrl+C at that question), then with SPARE-5
    out="$(in_terminal 300 'Which disk of data goes\?' $'\r' 'Which disk takes its place\?' $'\x03' \
        -- "${AS_TESTER[@]}" 'ame system raid replace data')"
    offered="$(sed -n '/Which disk takes its place?/,$p' <<<"${out}" | grep -oE 'SPARE-[0-9]' | awk '!seen[$0]++')"
    echo "# offered in place of ${MISSING_SERIAL}: $(paste -sd ' ' <<<"${offered}")"
    if grep -qE 'SPARE-[34]' <<<"${offered}"; then
        ok "a spare disk as large as the missing one is offered in its place"
    else
        not_ok "no 8 GiB spare disk is offered in place of the 8 GiB ${MISSING_SERIAL}" "offered: $(paste -sd ' ' <<<"${offered}")" \
            "spare: $(status_json | jq -c '[.spare[] | {serial, size}]')" \
            "missing: $(pool_json data | jq -c '[.devices[] | select(.missing) | {devid, size}]')"
    fi
    place="$(grep -nx SPARE-5 <<<"${offered}" | cut -d : -f 1)"
    replaced=false
    if [[ -z "${place}" ]]; then
        not_ok "SPARE-5 (10 GiB) is not offered either" "$(tail -n 20 <<<"${out}")"
    else
        downs=""
        for ((i = 1; i < place; i++)); do downs+=$'\e[B'; done
        # A replace that stops part way leaves the new disk blank again: cancelled as soon as it runs
        in_terminal 1200 'Which disk of data goes\?' $'\r' 'Which disk takes its place\?' "${downs}"$'\r' 'Type ERASE' $'ERASE\r' \
            'The recovery key of data' "${DATA_KEY}"$'\r' -- "${AS_TESTER[@]}" 'ame system raid replace data' >"${TESTS}/cancelled.out" 2>&1 &
        job=$! cancelled=false
        while kill -0 "${job}" 2>/dev/null; do
            if [[ "${cancelled}" == false ]] && btrfs replace status -1 /var/mnt/data 2>/dev/null | grep -q '% done' &&
                btrfs replace cancel /var/mnt/data >/dev/null 2>&1; then
                cancelled=true
            fi
            sleep 0.2
        done
        wait "${job}"
        out="$(cat "${TESTS}/cancelled.out")"
        if [[ "${cancelled}" != true ]]; then
            skip "a replace that stops part way: it could not be caught running"
        elif grep -qF 'is blank again' <<<"${out}" && ! grep -q '^raid-data-3[[:space:]]' /etc/crypttab &&
            [[ ! -e /dev/mapper/raid-data-3 && -z "$(lsblk -nro FSTYPE,PARTLABEL "$(spare 5)" | tr -d ' \n')" ]] &&
            status_json | jq -e '[.spare[].serial] | index("SPARE-5")' >/dev/null; then
            ok "a replace that stops part way leaves the new disk blank, out of /etc/crypttab, and spare again"
        else
            not_ok "a replace that stopped part way left the new disk behind" "$(tail -n 6 <<<"${out}")" \
                "$(lsblk -o NAME,FSTYPE,PARTLABEL "$(spare 5)")" "$(grep '^raid-data' /etc/crypttab)"
        fi
        out="$(in_terminal 1200 'Which disk of data goes\?' $'\r' 'Which disk takes its place\?' "${downs}"$'\r' 'Type ERASE' $'ERASE\r' \
            'The recovery key of data' "${DATA_KEY}"$'\r' -- "${AS_TESTER[@]}" 'ame system raid replace data')"
        says "the recovery key typed in is put on the new disk" "${out}" 'The recovery key opens the new disk too'
        says "ame system raid replace data replaces the missing disk with SPARE-5" "${out}" 'Disk [0-9]+ of data is replaced' &&
            replaced=true
    fi
    if [[ "${replaced}" == true ]]; then
        holds "data is whole: nothing missing, nothing on one copy, SPARE-5 in it" "$(pool_json data)" \
            '.missing == 0 and (.degraded | not) and .problems == [] and .layout == "raid1" and any(.devices[]; .disk.serial == "SPARE-5")'
        single="$(cat "/sys/fs/btrfs/${DATA_UUID}/allocation/"{data,metadata}/single/total_bytes 2>/dev/null | awk '{ n += $1 } END { print n + 0 }')"
        ((single == 0)) && ok "after the soft balance no single profile holds bytes" || not_ok "${single} bytes are still in single profiles"
        lines="$(awk '$1 ~ /^raid-data-/ { print $1 }' /etc/crypttab | sort | paste -sd ' ')"
        [[ "${lines}" == "raid-data-1 raid-data-3" ]] && ok "the missing disk's line is out of /etc/crypttab, the new disk's in" ||
            not_ok "/etc/crypttab has ${lines}"
        [[ "$(sha256sum /var/mnt/data/blob | cut -d ' ' -f 1)" == "${BLOB_SUM}" && "$(sha256sum /var/mnt/data/degraded | cut -d ' ' -f 1)" == "${degraded_sum}" ]] &&
            ok "every file is as it was written, before and while the disk was missing" || not_ok "a file changed through the replace"
        read -r part _ < <(parts_of data | awk '$2 == "raid-data-3"')
        check "the recovery key, typed, opens the new disk" opens_typed "${part}" "${DATA_KEY}"
        check "the recovery key, as --key-file -, opens the new disk" opens_as_file "${part}" "${DATA_KEY}"
    fi
    remember REPLACED "${replaced}"
    reboot_and_continue
    ;;

6)
    # The rest on data as the restart left it: every copy checked, a disk added, then the pool forgotten. A
    # phase of its own, so that a step that runs long costs only this part's lines.
    grep -q ' /var/mnt/data ' /etc/fstab || abandon "data is not there"
    if [[ "${REPLACED}" == true ]]; then
        holds "data is mounted after the restart, with nothing missing" "$(pool_json data)" '.mounted and .missing == 0'
        # Every copy checked now, and clean
        started="$(date +%s)"
        check "ame system raid scrub now starts the check" as_user 'ame system raid scrub now'
        wait_for 1200 scrub_now_done
        holds "the scrub is clean" "$(cat "${STATE}/scrub-${DATA_UUID}.json" 2>&1)" \
            ".time >= ${started} and .finished and .uncorrectable == 0 and .corrected == 0"

        # Add: one more disk. It asks whether to spread what is there (no, its default), and with three disks
        # whether to keep three copies of the metadata (yes).
        holds "Control offers Add a disk… for data, in a terminal" "$(row_of "$(control_json)" data)" \
            'any(.actions[]; .id == "add" and .terminal and .command == ["ame", "system", "raid", "add", "data"])'
        out="$(in_terminal 900 'Type ERASE' $'ERASE\r' 'The recovery key of data' "${DATA_KEY}"$'\r' \
            'Keep three copies of it\?' y 'Spread it now\?' $'\r' -- "${AS_TESTER[@]}" "ame system raid add data $(spare 4)")"
        says "ame system raid add data adds SPARE-4, asking whether to spread what is there" "${out}" \
            "$(spare 4) is in data" 'stays where it is unless it is spread over every disk now'
        holds "data is on three disks, its metadata in three copies" "$(pool_json data)" \
            '(.devices | length) == 3 and .missing == 0 and .metadata == "raid1c3" and any(.devices[]; .disk.serial == "SPARE-4")'
    fi
    [[ "${REPLACED}" == true ]] || abandon "data was not replaced and added to, so the damaged disks cannot follow"

    # A disk pulled out of an encrypted pool while it is mounted: its LUKS mapping stays, over nothing
    run_check
    pulled="$(spare 4)"
    devid="$(pool_json data | jq -r '.devices[] | select(.disk.serial == "SPARE-4") | .devid')"
    remember PULLED_DEVID "${devid}"
    echo 1 >"/sys/block/${pulled##*/}/device/delete"
    check "data takes a write with SPARE-4 pulled out" as_user 'head -c 32M /dev/urandom >/var/mnt/data/pulled && sync'
    sleep 5
    echo "# the kernel: $(journalctl -k --since -1min --no-pager -o cat | grep -iE 'btrfs|I/O error' | tail -n 4 | paste -sd '|')"
    holds "the status calls SPARE-4 missing, though its LUKS mapping is still open" "$(pool_json data)" \
        "any(.devices[]; .devid == ${devid} and .missing and .disk.serial == \"SPARE-4\") and (.problems | index(\"missing\")) != null"
    run_check
    check "the check tells ${TEST_USER}: A disk of the pool data is missing, naming SPARE-4" \
        wait_for 60 notified "A disk of the pool data is missing" "SPARE-4"
    remember PULLED_SUM "$(sha256sum /var/mnt/data/pulled | cut -d ' ' -f 1)"
    reboot_and_continue
    ;;

7)
    # SPARE-4 is back after the restart, with what it missed: back, not failing, and put right
    check "SPARE-4 is back" test -b "$(spare 4)"
    holds "the status calls it back, not counting errors" "$(pool_json data)" \
        "any(.devices[]; .devid == ${PULLED_DEVID} and .returned) and (.problems | index(\"returned\")) != null and (.problems | index(\"errors\")) == null"
    holds "Control offers Put the copies back… for data" "$(row_of "$(control_json)" data)" \
        '(.text | contains("A disk that was missing is back")) and any(.actions[]; .id == "replace" and .title == "Put the copies back…")'
    out="$(in_terminal 1200 'Put the copies back now\?' y -- "${AS_TESTER[@]}" 'ame system raid replace data')"
    says "ame system raid replace data puts the copies back" "${out}" 'is back, with old copies' 'Every file of data has its copies again'
    holds "data is whole: nothing back, counting or with one copy" "$(pool_json data)" \
        '.state == "ok" and all(.devices[]; (.returned | not) and .error_count == 0)'
    [[ "$(sha256sum /var/mnt/data/pulled | cut -d ' ' -f 1)" == "${PULLED_SUM}" ]] && ok "what was written with SPARE-4 out reads the same" ||
        not_ok "the file written with SPARE-4 out changed"

    # A disk with bad sectors: one copy of a file on a disk whose sectors there fail every read, as a disk
    # that is going does, from the next start (QEMU's blkdebug)
    check "${TEST_USER} writes a file to damage" as_user 'head -c 8M /dev/urandom >/var/mnt/data/damaged && sync'
    remember DAMAGED_SUM "$(sha256sum /var/mnt/data/damaged | cut -d ' ' -f 1)"
    # Never on SPARE-3, which takes the damaged disk's place, nor on SPARE-5, larger (10 GiB) than SPARE-3 can replace
    where="$(SKIP="SPARE-3 SPARE-5" bad_place /var/mnt/data/damaged)"
    if [[ -z "${where}" ]]; then
        not_ok "the damaged file's copies could not be found on the disks" "$(filefrag -v /var/mnt/data/damaged 2>&1 | tail -n 3)"
    else
        read -r serial sectors <<<"${where}"
        remember DAMAGED_SERIAL "${serial}"
        echo "# the file's copy on ${serial} is at sectors ${sectors}"
        next_boot "{\"faulty\": {\"${serial#SPARE-}\": [${sectors// /, }]}}"
        # One command at a time to that disk from its next start: QEMU's AHCI answers a failed read as a disk does,
        # but with several commands queued (NCQ) the guest's I/O to it stalled in the lab (2026-10-19)
        printf 'ACTION=="add|change", SUBSYSTEM=="block", ENV{DEVTYPE}=="disk", ENV{ID_SERIAL_SHORT}=="%s", ATTR{device/queue_depth}="1"\n' \
            "${serial}" >/etc/udev/rules.d/99-vmlab-damaged.rules
    fi
    # GNOME Shell's widgets on the accessibility bus from the next login, for the notification's button
    as_user 'gsettings set org.gnome.desktop.interface toolkit-accessibility true'
    reboot_and_continue
    ;;

8)
    [[ -n "${DAMAGED_SERIAL:-}" ]] || abandon "no disk was damaged"
    # Read again and again, from new processes, so that btrfs reads each copy in turn. Each read in the
    # background: one the kernel never finishes would otherwise hold this suite, and the lab's helper with it.
    sums=""
    for _ in 1 2 3 4 5 6; do
        echo 3 >/proc/sys/vm/drop_caches
        sum="$(bounded_sum /var/mnt/data/damaged 300)" || {
            not_ok "a read of the damaged file did not finish within 5 minutes" \
                "$(timeout 30 journalctl -k -b --no-pager -o short-monotonic | grep -iE 'ata[0-9]|I/O error|btrfs|blocked for more' | tail -n 30)"
            finish
        }
        sums+="${sum} "
    done
    [[ "$(tr ' ' '\n' <<<"${sums}" | grep -v "^$" | sort -u)" == "${DAMAGED_SUM}" ]] && ok "the damaged file reads right every time" ||
        not_ok "the damaged file read wrong" "${sums}"
    kernel="$(journalctl -k -b --no-pager -o cat | grep -iE 'I/O error|read error corrected|btrfs.*errs' | tail -n 6)"
    echo "# the kernel: $(paste -sd '|' <<<"${kernel}")"
    grep -qi 'read error corrected' <<<"${kernel}" && ok "btrfs read the good copy and rewrote the bad one" ||
        not_ok "btrfs did not say it corrected a read error" "${kernel:-(nothing)}"
    # btrfs counts no error against a disk for a read ahead of time, which those were; the check of every copy,
    # which reads each one, does (128 in the lab's reproduction, all repaired from the good copy)
    started="$(date +%s)"
    check "ame system raid scrub now" as_user 'ame system raid scrub now'
    wait_for 1200 scrub_now_done
    holds "the check of every copy met the bad sectors and repaired them from the good copy" "$(cat "${STATE}/scrub-${DATA_UUID}.json" 2>&1)" \
        ".time >= ${started} and .finished and .corrected > 0 and .uncorrectable == 0"
    holds "the status: ${DAMAGED_SERIAL} is counting errors" "$(pool_json data)" \
        "any(.devices[]; .disk.serial == \"${DAMAGED_SERIAL}\" and .error_count > 0) and (.problems | index(\"errors\")) != null"
    echo "# SMART of the damaged disk: $(smartctl -H -j "$(readlink -f "/dev/disk/by-id/ata-QEMU_HARDDISK_${DAMAGED_SERIAL}")" | jq -c .smart_status)"
    run_check
    check "the check tells ${TEST_USER}: A disk of the pool data is counting errors, naming ${DAMAGED_SERIAL}" \
        wait_for 60 notified "A disk of the pool data is counting errors" "${DAMAGED_SERIAL}"
    # The notification's button, clicked as a person would, through GNOME Shell's accessibility tree
    if shell click "Open Control" && wait_for 120 shell_app amethystora-control has "Disks"; then
        ok "Open Control opens Control on its System page, at the Disks group"
        holds "Control says a disk of data counts errors, and offers Replace a disk…" "$(row_of "$(control_json)" data)" \
            '(.text | contains("A disk has counted read or write errors.")) and any(.actions[]; .id == "replace" and .title == "Replace a disk…")'
        pkill -u "${TEST_USER}" -f amethystora-control
    else
        not_ok "Open Control did not open Control" "$(shell text 2>&1 | grep -i -A2 -B2 'disk' | head -n 20)"
    fi
    as_user 'gsettings reset org.gnome.desktop.interface toolkit-accessibility'
    # Replaced while it still reads, from the good copies: the disk counting errors is the one offered first
    out="$(in_terminal 1200 'Which disk of data goes\?' $'\r' 'Which disk takes its place\?' $'\r' 'Type ERASE' $'ERASE\r' \
        'The recovery key of data' "${DATA_KEY}"$'\r' -- "${AS_TESTER[@]}" 'ame system raid replace data')"
    says "ame system raid replace data puts SPARE-3 in place of the damaged ${DAMAGED_SERIAL}" "${out}" 'errors' 'Disk [0-9]+ of data is replaced'
    # Read again for up to two minutes: just after a replace, the kernel's names for the new disk may not be there yet
    whole=".state == \"ok\" and any(.devices[]; .disk.serial == \"SPARE-3\") and (any(.devices[]; .disk.serial == \"${DAMAGED_SERIAL}\") | not)"
    for _ in $(seq 30); do
        jq -e "${whole}" <<<"$(pool_json data)" >/dev/null 2>&1 && break
        sleep 4
    done
    holds "data is whole on SPARE-3, without ${DAMAGED_SERIAL}" "$(pool_json data)" "${whole}"
    [[ "$(sha256sum /var/mnt/data/damaged | cut -d ' ' -f 1)" == "${DAMAGED_SUM}" ]] && ok "the damaged file is whole on the new disk" ||
        not_ok "the damaged file changed through the replace"
    # The rest in phases of their own in this same boot: together they outlast the lab's 30 minutes a phase
    continue_phase
    ;;

9)
    # A copy damaged without a word from the disk: what btrfs's checksums are for
    check "${TEST_USER} writes a file to corrupt" as_user 'head -c 8M /dev/urandom >/var/mnt/data/corrupted && sync'
    sum="$(sha256sum /var/mnt/data/corrupted | cut -d ' ' -f 1)"
    where="$(bad_place /var/mnt/data/corrupted bytes)"
    read -r serial offset <<<"${where}"
    if [[ -z "${serial}" ]]; then
        not_ok "the file's copies could not be found on the disks" "$(filefrag -v /var/mnt/data/corrupted 2>&1 | tail -n 3)"
        continue_phase
    fi
    dd if=/dev/urandom of="$(readlink -f "/dev/disk/by-id/ata-QEMU_HARDDISK_${serial}")" bs=4096 count=16 \
        seek="$((offset / 4096))" oflag=direct conv=notrunc status=none
    echo 3 >/proc/sys/vm/drop_caches
    # Checked before anything reads the file, which would repair the copy itself
    started="$(date +%s)"
    check "ame system raid scrub now" as_user 'ame system raid scrub now'
    wait_for 1200 scrub_now_done
    holds "the scrub found the copy overwritten on ${serial} behind btrfs's back and repaired it" "$(cat "${STATE}/scrub-${DATA_UUID}.json" 2>&1)" \
        ".time >= ${started} and .finished and .corrected > 0 and .uncorrectable == 0"
    [[ "$(sha256sum /var/mnt/data/corrupted | cut -d ' ' -f 1)" == "${sum}" ]] && ok "the overwritten file reads right" ||
        not_ok "the overwritten file read wrong"
    continue_phase
    ;;

10)
    # A suspend while every copy is read: the next run resumes it. systemd's suspend and the kernel's, every task
    # frozen and every device suspended, then woken by the kernel itself after 5 s (pm_test): the lab's VM has no
    # S3 (QEMU's disable_s3), and the RTC's alarm did not wake it from s2idle (2026-10-19)
    if ! grep -qw devices /sys/power/pm_test 2>/dev/null; then
        skip "a suspend during a scrub: this kernel has no test of its suspend (pm_test)"
    else
        # Slow enough for the suspend to come while it reads: a scrub of data's few gigabytes ends in seconds
        for limit in /sys/fs/btrfs/"${DATA_UUID}"/devinfo/*/scrub_speed_max; do echo $((16 * 1048576)) >"${limit}"; done
        systemctl start --no-block amethystora-raid-scrub.service
        wait_for 60 pgrep -f 'btrfs scrub start -B -R /var/mnt/data'
        sleep 3
        echo "# the scrub before the suspend: $(btrfs scrub status /var/mnt/data | grep -E 'Status|scrubbed' | paste -sd ' ')"
        echo devices >/sys/power/pm_test
        before="$(date +%s)"
        systemctl suspend
        wait_for 180 sh -c "journalctl -b --since @${before} --no-pager -o cat | grep -q 'PM: suspend exit'"
        echo none >/sys/power/pm_test
        wait_for 600 sh -c '! systemctl is-active --quiet amethystora-raid-scrub.service'
        slept="$(journalctl -b --since "@${before}" --no-pager -o cat | grep -cE 'PM: suspend (entry|exit)|System returned from sleep')"
        echo "# suspend lines in the journal: ${slept}; the scrub: $(cat "${STATE}/scrub-${DATA_UUID}.json")"
        if ((slept > 0)) && jq -e '.finished == false' "${STATE}/scrub-${DATA_UUID}.json" >/dev/null; then
            ok "a suspend cut the scrub short, and it is written down as not finished"
            systemctl start amethystora-raid-scrub.service &
            job=$! seen=""
            while kill -0 "${job}" 2>/dev/null; do
                seen+="$(pgrep -af 'btrfs scrub (start|resume) -B -R /var/mnt/data' | grep -oE 'scrub (start|resume)')"$'\n'
                sleep 0.2
            done
            wait "${job}"
            grep -qx 'scrub resume' <<<"${seen}" && ok "the next run resumes the scrub the suspend cut short" ||
                not_ok "the next run did not resume it" "seen: $(sort -u <<<"${seen}" | paste -sd ' ')"
        else
            not_ok "the suspend did not cut the scrub short" "suspend lines: ${slept}" "$(cat "${STATE}/scrub-${DATA_UUID}.json")" \
                "$(journalctl -b --since "@${before}" --no-pager -o cat | grep -iE 'scrub|suspend|sleep' | tail -n 10)"
        fi
        for limit in /sys/fs/btrfs/"${DATA_UUID}"/devinfo/*/scrub_speed_max; do echo 0 >"${limit}"; done
    fi

    # A disk truly gone at the next start: not there at all
    absent="$(pool_json data | jq -r '[.devices[] | select(.disk.serial != "SPARE-3") | .disk.serial] | first')"
    remember ABSENT_SERIAL "${absent}"
    remember REF_BOOT "$(graphical_at)"
    # SPARE-6 stays unplugged, as it is for every suite but 85-raid-mirror.sh
    next_boot "{\"absent\": [${absent#SPARE-}, 6]}"
    reboot_and_continue
    ;;

11)
    check "${ABSENT_SERIAL} is not there at all" test ! -e "/dev/disk/by-id/ata-QEMU_HARDDISK_${ABSENT_SERIAL}"
    booted="$(graphical_at)"
    echo "# desktop at ${booted} s, ${REF_BOOT} s with every disk there"
    ((booted - REF_BOOT <= 30)) && ok "the missing disk held the boot up 30 s at most ($((booted - REF_BOOT)) s)" ||
        not_ok "the boot took $((booted - REF_BOOT)) s longer with a disk of data gone"
    check "data did not open" sh -c '! findmnt -rn --mountpoint /var/mnt/data'
    run_check
    check "the check tells ${TEST_USER}: The pool data did not open" wait_for 60 notified "The pool data did not open"
    says "ame system raid mount data opens it with ${ABSENT_SERIAL} gone" "$(as_user 'ame system raid mount data' 2>&1)" \
        'data is open at /var/mnt/data with a disk missing'
    check "writing works" as_user 'head -c 64M /dev/urandom >/var/mnt/data/absent && sync'
    remember ABSENT_SUM "$(sha256sum /var/mnt/data/absent | cut -d ' ' -f 1)"
    run_check
    next_boot '{"absent": [6]}'
    reboot_and_continue
    ;;

12)
    # The disk back: data opens by itself again, and has its copies put back
    check "data opens by itself with ${ABSENT_SERIAL} back" findmnt -rn --mountpoint /var/mnt/data
    holds "the status calls ${ABSENT_SERIAL} back" "$(pool_json data)" \
        "any(.devices[]; .disk.serial == \"${ABSENT_SERIAL}\" and .returned)"
    out="$(in_terminal 1200 'Put the copies back now\?' y -- "${AS_TESTER[@]}" 'ame system raid replace data')"
    says "ame system raid replace data puts the copies back" "${out}" 'Every file of data has its copies again'
    holds "data is whole" "$(pool_json data)" '.state == "ok" and all(.devices[]; (.returned | not) and .error_count == 0) and (.degraded | not)'
    [[ "$(sha256sum /var/mnt/data/absent | cut -d ' ' -f 1)" == "${ABSENT_SUM}" && "$(sha256sum /var/mnt/data/damaged | cut -d ' ' -f 1)" == "${DAMAGED_SUM}" ]] &&
        ok "every file reads as it was written" || not_ok "a file changed"

    # The end: data forgotten, and nothing of any pool left behind
    out="$(in_terminal 180 'Forget data\?' y -- "${AS_TESTER[@]}" 'ame system raid forget data')"
    says "ame system raid forget data" "${out}" 'data is forgotten'
    check "nothing of data is left: fstab and crypttab lines, key, folder, open disks" forgotten data
    check "no pool's line, key or open disk is left anywhere" nothing_left
    holds "no pool is left, and the disks are spare again (the damaged one too)" "$(status_json)" '.pools == [] and (.spare | length) == 5'
    run_check
    check_skipped && ok "with every pool forgotten, the check stops at its ExecCondition again" ||
        not_ok "with every pool forgotten, the check still ran" "$(journalctl -b -u amethystora-raid-check.service --no-pager -o cat | tail -n 10)"
    pkill -u "${TEST_USER}" -f 'action=open=Open Control'
    rm -f "${SUDOERS}" /etc/udev/rules.d/99-vmlab-damaged.rules
    check "${TEST_USER}'s sudo asks for a password again" sh -c "! runuser -u ${TEST_USER} -- sudo -n true 2>/dev/null"
    finish
    ;;

*) finish ;;
esac
