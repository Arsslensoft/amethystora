#!/usr/bin/bash
# amethystora-boot-guard (R7-52) against stand-ins for ostree, grub2-editenv, bootc, rpm-ostree, findmnt, mount, fsfreeze,
# systemctl and loginctl, run by 20-tests.sh. What it checks is what a mistake would cost: nothing is written while it is off,
# it arms only when the entry behind the default is the system running after a good start (and the new system has the guard
# in it, and ostree would flip its boot version for a swap), a good start leaves nothing behind that could send the computer
# back, a false count never rolls back, the GRUB script it writes is valid, counts as promised and takes turns at the end, and
# a power cut at ANY step it takes (between two of its tools, or half way through writing custom.cfg) leaves a computer that
# starts a working system by itself.
#
# That last one runs against a small model of the machine: two deployments, ostree's boot version (a write of the
# deployments flips it and deletes the boot loader's other /loader.N and the /ostree/boot.N link, which a real ostree 2026.4
# does, shown on a throwaway sysroot, unless both deployments have the same kernel, arguments and version, when it only swaps
# the links' serials: the model does the same), and the script the guard wrote, run as GRUB would run it (the ladder
# translated to bash) to see which system starts. Each stand-in that changes something counts itself, and kills the guard
# after the Nth change. What the model does not have: a journal that loses writes (only a real boot shows that), and GRUB
# itself.
#   bash test-boot-guard.sh [path/to/amethystora-boot-guard]
set -uo pipefail

GUARD="${1:-/usr/libexec/amethystora-boot-guard}"
ROOT="$(mktemp -d)"
BIN="${ROOT}/bin"
S="${ROOT}/state"
failures=0
fail() { echo "test-boot-guard: FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { [[ "$2" == "$3" ]] || fail "$1: expected [$3], got [$2]"; }

NEW="$(printf 'a%.0s' {1..64})"
OLD="$(printf 'b%.0s' {1..64})"
LATER="$(printf 'c%.0s' {1..64})"
SHARED="$(printf 's%.0s' {1..64})"

mkdir -p "${BIN}" "${S}" "${ROOT}/boot/grub2" "${ROOT}/usr/share/amethystora"

# The model: state/order is the deployments newest default first (new old, or old new after a rollback), state/bootver
# ostree's boot version, state/running the system that is running, state/sameboot present when both deployments have the
# same kernel, initramfs, arguments and version. world_build writes what ostree and the boot loader would show for them.
cat >"${S}/world.sh" <<'EOF'
csum_of() { if [[ "$1" == new ]]; then printf 'a%.0s' {1..64}; else printf 'b%.0s' {1..64}; fi; }
world_build() {
    local order i name csum bootcsum serial bootver running flag deploy kernel
    read -r -a order <<<"$(cat "${S}/order")"
    bootver="$(cat "${S}/bootver")"
    running="$(cat "${S}/running")"
    /usr/bin/rm -rf "${ROOT}"/boot/loader.* "${ROOT}/boot/loader" "${ROOT}"/ostree/boot.*
    mkdir -p "${ROOT}/boot/loader.${bootver}/entries"
    ln -s "loader.${bootver}" "${ROOT}/boot/loader"
    : >"${S}/ostree-status"
    for i in 0 1; do
        name="${order[$i]}"
        csum="$(csum_of "${name}")"
        # The links ostree= names: boot.N/default/BOOTCSUM/SERIAL, the serial telling apart two deployments of one kernel
        if [[ -e "${S}/sameboot" ]]; then
            bootcsum="$(printf 's%.0s' {1..64})"
            serial="${i}"
            kernel=shared
        else
            bootcsum="${csum}"
            serial=0
            kernel="${name}"
        fi
        mkdir -p "${ROOT}/ostree/boot.${bootver}/default/${bootcsum}/${serial}" "${ROOT}/boot/ostree/default-${kernel}"
        : >"${ROOT}/boot/ostree/default-${kernel}/vmlinuz"
        printf 'title Amethystora (%s) (ostree:%s)\nversion %s\nlinux /ostree/default-%s/vmlinuz\ninitrd /ostree/default-%s/initramfs.img\noptions rw ostree=/ostree/boot.%s/default/%s/%s\n' \
            "${kernel}" "${i}" "$((2 - i))" "${kernel}" "${kernel}" "${bootver}" "${bootcsum}" "${serial}" >"${ROOT}/boot/loader/entries/ostree-$((2 - i)).conf"
        flag=""
        [[ "${i}" == 0 && "${running}" != "${name}" ]] && flag=" (pending)"
        if [[ "${running}" == "${name}" ]]; then
            printf '* default %s.0\n    Version: %s\n' "${csum}" "${name}" >>"${S}/ostree-status"
        else
            printf '  default %s.0%s\n    Version: %s\n' "${csum}" "${flag}" "${name}" >>"${S}/ostree-status"
        fi
        # The deployment's own tree: the guard in it, and enabled, unless the test took them away
        if [[ ! -e "${S}/noguard-${name}" ]]; then
            deploy="${ROOT}/ostree/deploy/default/deploy/${csum}.0"
            mkdir -p "${deploy}/usr/libexec" "${deploy}/etc/systemd/system/graphical.target.wants"
            : >"${deploy}/usr/libexec/amethystora-boot-guard"
            chmod +x "${deploy}/usr/libexec/amethystora-boot-guard"
            ln -sf /usr/lib/systemd/system/amethystora-boot-guard-good.service "${deploy}/etc/systemd/system/graphical.target.wants/amethystora-boot-guard-good.service"
        fi
    done
}
# What bootc and rpm-ostree do for a rollback, as ostree does it: the deployments swap, and the boot version flips (and the
# other loader.N and boot.N go) unless both have the same boot configuration
world_swap() {
    local a b
    read -r a b <<<"$(cat "${S}/order")"
    echo "${b} ${a}" >"${S}/order"
    [[ -e "${S}/sameboot" ]] || echo $((1 - $(cat "${S}/bootver"))) >"${S}/bootver"
    world_build
}
# Whether the next change is the one to cut at
tick_peek() {
    [[ -e "${S}/count-on" ]] && [[ "$(cat "${S}/die-at" 2>/dev/null)" == "$(($(cat "${S}/count" 2>/dev/null || echo 0) + 1))" ]]
}
# A change a stand-in has just made: counted, and the guard is killed after the one the test asked for
tick() {
    [[ -e "${S}/count-on" ]] || return 0
    local n p
    n=$(($(cat "${S}/count" 2>/dev/null || echo 0) + 1))
    echo "${n}" >"${S}/count"
    if [[ "$(cat "${S}/die-at" 2>/dev/null)" == "${n}" ]]; then
        echo "${n}:$1" >"${S}/died"
        p=$PPID
        while [[ -n "${p}" && "${p}" -gt 1 ]]; do
            if tr '\0' ' ' <"/proc/${p}/cmdline" 2>/dev/null | grep -q "^bash ${AMETHYSTORA_TEST_GUARD} "; then kill -9 "${p}" 2>/dev/null; fi
            p="$(awk '{ print $4 }' "/proc/${p}/stat" 2>/dev/null)"
        done
    fi
}
EOF
cat >"${BIN}/ostree" <<'EOF'
#!/usr/bin/bash
cat "${AMETHYSTORA_BOOT_GUARD_ROOT}/state/ostree-status"
EOF
cat >"${BIN}/findmnt" <<'EOF'
#!/usr/bin/bash
cat "${AMETHYSTORA_BOOT_GUARD_ROOT}/state/boot-mount"
EOF
cat >"${BIN}/mount" <<'EOF'
#!/usr/bin/bash
S="${AMETHYSTORA_BOOT_GUARD_ROOT}/state"
echo "mount $*" >>"${S}/log"
[[ ! -e "${S}/mount-fails" ]] || exit 1
. "${S}/world.sh"
tick mount
EOF
# fsfreeze: logged only (a cut between the freeze and the thaw is not modelled, nothing is written in between)
cat >"${BIN}/fsfreeze" <<'EOF'
#!/usr/bin/bash
echo "fsfreeze $*" >>"${AMETHYSTORA_BOOT_GUARD_ROOT}/state/log"
EOF
cat >"${BIN}/systemctl" <<'EOF'
#!/usr/bin/bash
echo "systemctl $*" >>"${AMETHYSTORA_BOOT_GUARD_ROOT}/state/log"
[[ "$1" != is-active ]] || [[ -e "${AMETHYSTORA_BOOT_GUARD_ROOT}/state/dm-up" ]]
EOF
cat >"${BIN}/loginctl" <<'EOF'
#!/usr/bin/bash
[[ -e "${AMETHYSTORA_BOOT_GUARD_ROOT}/state/session" ]] && echo 2
exit 0
EOF
cat >"${BIN}/bootc" <<'EOF'
#!/usr/bin/bash
S="${AMETHYSTORA_BOOT_GUARD_ROOT}/state"
ROOT="${AMETHYSTORA_BOOT_GUARD_ROOT}"
echo "bootc $*" >>"${S}/log"
case "$1 ${2:-}" in
    "status --json") echo '{"status":{"rollback":{"image":{"imageDigest":"sha256:bad","version":"new-1"}},"booted":{"image":{"version":"old-1"}}}}' ;;
    "upgrade --check")
        cat "${S}/upgrade-check" 2>/dev/null
        [[ ! -e "${S}/upgrade-check-late" ]] || { /usr/bin/sleep 0.3; echo "more lines after the digest"; }
        ;;
    "rollback "*)
        [[ ! -e "${S}/rollback-fails" ]] || exit 1
        # Exits 0 and changes nothing
        [[ ! -e "${S}/rollback-noop" ]] || exit 0
        . "${S}/world.sh"
        world_swap
        tick "bootc-rollback"
        # Fails after its swap (ostree's final cleanup, or bootc's own bookkeeping, can)
        [[ ! -e "${S}/rollback-fails-after" ]] || exit 1
        ;;
esac
EOF
# What rpm-ostree documents: it swaps the two, whatever the order is
cat >"${BIN}/rpm-ostree" <<'EOF'
#!/usr/bin/bash
S="${AMETHYSTORA_BOOT_GUARD_ROOT}/state"
echo "rpm-ostree $*" >>"${S}/log"
[[ ! -e "${S}/rpm-fails" ]] || exit 1
[[ ! -e "${S}/rpm-noop" ]] || exit 0
. "${S}/world.sh"
world_swap
tick "rpm-ostree-rollback"
EOF
# A grubenv of KEY=VALUE lines; the argument is the file, then the action, as grub2-editenv takes them
cat >"${BIN}/grub2-editenv" <<'EOF'
#!/usr/bin/bash
S="${AMETHYSTORA_BOOT_GUARD_ROOT}/state"
file="$1"
[[ ! -e "${S}/editenv-fails" || "$2" == list ]] || exit 1
. "${S}/world.sh"
case "$2" in
    create) echo '# GRUB Environment Block' >"${file}"; tick "editenv-create" ;;
    list) grep -v '^#' "${file}" ;;
    set) grep -v "^${3%%=*}=" "${file}" >"${file}.n"; echo "$3" >>"${file}.n"; /usr/bin/mv "${file}.n" "${file}"; tick "editenv-set" ;;
    unset) grep -v "^$3=" "${file}" >"${file}.n"; /usr/bin/mv "${file}.n" "${file}"; tick "editenv-unset" ;;
esac
EOF
# The write of custom.cfg: tee, which a cut can leave half written (the file it is given, whichever that is)
cat >"${BIN}/tee" <<'EOF'
#!/usr/bin/bash
. "${AMETHYSTORA_BOOT_GUARD_ROOT}/state/world.sh"
data="$(/usr/bin/cat; echo x)"
data="${data%x}"
if tick_peek; then
    printf '%s' "${data:0:$((${#data} / 2))}" >"$1"
else
    printf '%s' "${data}" >"$1"
fi
tick tee
EOF
# The changes the guard makes with the system's own tools, each counted
for tool in mv rm sync; do
    cat >"${BIN}/${tool}" <<EOF
#!/usr/bin/bash
. "\${AMETHYSTORA_BOOT_GUARD_ROOT}/state/world.sh"
/usr/bin/${tool} "\$@"
code=\$?
tick ${tool}
exit \${code}
EOF
done
cat >"${BIN}/sleep" <<'EOF'
#!/usr/bin/bash
# A session that comes up on the Nth wait, when the test says so
S="${AMETHYSTORA_BOOT_GUARD_ROOT}/state"
if [[ -e "${S}/session-after" ]]; then
    n=$(($(cat "${S}/slept" 2>/dev/null || echo 0) + 1))
    echo "${n}" >"${S}/slept"
    [[ "${n}" -lt "$(cat "${S}/session-after")" ]] || : >"${S}/session"
fi
exit 0
EOF
chmod +x "${BIN}"/*

export AMETHYSTORA_BOOT_GUARD_ROOT="${ROOT}" AMETHYSTORA_BOOT_GUARD_WAIT=2 AMETHYSTORA_BOOT_GUARD_LOCK_WAIT=1 \
    AMETHYSTORA_TEST_GUARD="${GUARD}" ROOT S PATH="${BIN}:${PATH}"
# shellcheck disable=SC1091
. "${S}/world.sh"
guard() { bash "${GUARD}" "$@"; }
ENV="${ROOT}/boot/grub2/grubenv"
CFG="${ROOT}/boot/grub2/custom.cfg"
STATE="${ROOT}/var/lib/amethystora/boot-guard"
ON="${STATE}/on"
GOOD_NOW="${ROOT}/run/amethystora/boot-guard/good"
RAN="${S}/log"

# The machine as it is just before an update's restart: the old system running, the new one the default
reset() {
    /usr/bin/rm -rf "${ROOT:?}/var" "${ROOT:?}/run" "${ROOT:?}/ostree" "${ROOT:?}/boot/ostree" "${CFG}" "${CFG}".new* "${ENV}" "${ENV}.n" \
        "${S}"/{rollback-fails,rollback-noop,rollback-fails-after,rpm-fails,rpm-noop,editenv-fails,mount-fails,count,count-on,die-at,died,upgrade-check,upgrade-check-late,noguard-new,noguard-old,sameboot,session-after,slept} "${RAN}"
    mkdir -p "${STATE}" "${ROOT}/run/amethystora/boot-guard"
    : >"${RAN}"
    echo '# GRUB Environment Block' >"${ENV}"
    echo '/dev/nvme0n1p2 ext4 rw,relatime' >"${S}/boot-mount"
    /usr/bin/rm -f "${ROOT}/usr/share/amethystora/sealed"
    echo "new old" >"${S}/order"
    echo 1 >"${S}/bootver"
    echo old >"${S}/running"
    world_build
    : >"${S}/dm-up"
    : >"${S}/session"
    NEW_BROKEN=0
}
tries() { grep "^ame_boot_tries=" "${ENV}" 2>/dev/null | cut -d= -f2; }
rollbacks() { grep -c '^bootc rollback' "${RAN}"; }
on() { : >"${ON}"; }
# This start was good, and an update was finalized: the shutdown that arms
good_start() { : >"${GOOD_NOW}"; }
armed_now() { grep -qxF '### BEGIN amethystora-boot-guard ###' "${CFG}" 2>/dev/null; }
run_as() {
    echo "$1" >"${S}/running"
    world_build
}
# The default (the first of the order), and whether custom.cfg is a script GRUB would take whole
default_now() { head -c 3 "${S}/order"; }
cfg_whole() {
    [[ ! -e "${CFG}" ]] || ! command -v grub2-script-check >/dev/null || grub2-script-check "${CFG}" >/dev/null 2>&1
}
# Every freeze has its thaw, in that order
frozen_clean() {
    grep '^fsfreeze' "${RAN}" | awk 'NR % 2 == 1 && $2 != "-f" { bad = 1 } NR % 2 == 0 && $2 != "-u" { bad = 1 } END { exit bad + (NR % 2) }'
}

# ---- GRUB, as the script runs: which system starts. new, old, or FAIL when the kernel or the root cannot be found ----
grub_start() {
    local BOOTFS="${ROOT}/boot" ame_boot_tries="" default="" ladder opts linux path csum serial entry order
    # ame_boot_tries is read by the ladder, which is eval-ed below
    # shellcheck disable=SC2034
    [[ -s "${ENV}" ]] && ame_boot_tries="$(grep '^ame_boot_tries=' "${ENV}" | cut -d= -f2)"
    save_env() {
        local v="${!1}"
        grep -v "^$1=" "${ENV}" >"${ENV}.n" 2>/dev/null
        echo "$1=${v}" >>"${ENV}.n"
        /usr/bin/mv "${ENV}.n" "${ENV}"
    }
    if armed_now; then
        ladder="$(awk '/^### BEGIN/ { on = 1; next } /^menuentry/ { exit } on' "${CFG}" | grep -v '^#' |
            sed -e 's/^\( *\)set /\1/' -e 's| -d /| -d "${BOOTFS}"/|' -e 's| -f /| -f "${BOOTFS}"/|')"
        eval "${ladder}"
    fi
    if [[ "${default}" == amethystora-previous ]]; then
        linux="$(sed -n 's/^    linux \([^ ]*\) .*/\1/p' "${CFG}")"
        opts="$(sed -n 's/^    linux [^ ]* //p' "${CFG}")"
    else
        entry="$(grep -lF '(ostree:0)' "${BOOTFS}"/loader/entries/*.conf 2>/dev/null | head -n 1)"
        linux="$(awk '$1 == "linux" { print $2 }' "${entry}" 2>/dev/null)"
        opts="$(awk '$1 == "options" { $1 = ""; print }' "${entry}" 2>/dev/null)"
    fi
    [[ -f "${BOOTFS}${linux}" ]] || {
        echo FAIL
        return
    }
    path="$(sed -n 's|.*ostree=\([^ ]*\).*|\1|p' <<<"${opts}")"
    [[ -d "${ROOT}${path}" ]] || {
        echo FAIL
        return
    }
    # The link is BOOTCSUM/SERIAL: a kernel of its own means one deployment, a shared one means the serial'th of the order
    csum="$(basename "$(dirname "${path}")")"
    serial="$(basename "${path}")"
    if [[ "${csum}" == "${SHARED}" ]]; then
        read -r -a order <<<"$(cat "${S}/order")"
        echo "${order[${serial}]}"
    else
        [[ "${csum}" == "${NEW}" ]] && echo new || echo old
    fi
}

# One start of the machine: GRUB picks a system, it runs begin, and good once its desktop is up (the new system's never is
# when NEW_BROKEN is 1). CUT is the Nth change good makes, after which the power goes.
STARTED=""
start_machine() {
    local cut="${1:-0}"
    STARTED="$(grub_start)"
    [[ "${STARTED}" != FAIL ]] || return 1
    run_as "${STARTED}"
    guard begin
    if [[ "${STARTED}" == old || "${NEW_BROKEN}" != 1 ]]; then
        ((cut)) && echo "${cut}" >"${S}/die-at"
        /usr/bin/rm -f "${S}/count" "${S}/died"
        : >"${S}/count-on"
        { guard good; } 2>/dev/null
        /usr/bin/rm -f "${S}/count-on" "${S}/die-at"
    fi
    return 0
}

# ---- Off: nothing at all ----
reset
guard arm
guard good
[[ ! -e "${CFG}" ]] || fail "off, arm wrote custom.cfg"
expect "off, no count" "$(tries)" ""
[[ ! -s "${RAN}" ]] || fail "off, it ran something: $(cat "${RAN}")"
echo '{"digest":"sha256:bad"}' >"${STATE}/went-back.json"
echo 'Update available: sha256:bad' >"${S}/upgrade-check"
guard update-ok || fail "off, the update waited"

# ---- Arming ----
reset
on
guard arm
[[ ! -e "${CFG}" ]] || fail "armed without a good start this time"
good_start
run_as new
guard arm
[[ ! -e "${CFG}" ]] || fail "armed with nothing new"
reset
on
good_start
guard arm
armed_now || fail "not armed after a new system was put in place"
expect "the count" "$(tries)" 3
grep -qF "ostree=/ostree/boot.1/default/${OLD}/0" "${CFG}" || fail "the entry is not the system that was running"
grep -qF "ostree=/ostree/boot.1/default/${NEW}/0" "${CFG}" && fail "the entry is the new system"
grep -qF 'set default=amethystora-previous' "${CFG}" || fail "no way back in the entry"
grep -q 'default=1\|default="1"' "${CFG}" && fail "the entry goes back by position: the second entry may be firmware settings"
grep -qxF "# amethystora-boot-guard for ${NEW}.0 ${OLD}.0" "${CFG}" || fail "the block does not say which deployments it is for"
grep -qF -- '-d /loader.1' "${CFG}" || fail "the way back does not check that the entry is still real"
# With the disk's passphrase to type, more tries
reset
on
good_start
sed -i 's/^options rw/options rw rd.luks.uuid=luks-1/' "${ROOT}/boot/loader/entries/ostree-1.conf"
guard arm
expect "the count under LUKS" "$(tries)" 5

# The rest of custom.cfg (the pools of disks' entry) is left as it is, both ways
reset
on
good_start
printf '%s\n' '### BEGIN amethystora-raid ###' 'menuentry x {' '}' '### END amethystora-raid ###' >"${CFG}"
guard arm
grep -qF 'amethystora-raid' "${CFG}" || fail "arm lost another block of custom.cfg"
run_as new
guard good
armed_now && fail "a good start left the entry"
grep -qF 'amethystora-raid' "${CFG}" || fail "good lost another block of custom.cfg"

# Where GRUB cannot count, nothing is armed
for mount in '/dev/md127 ext4 rw' '/dev/sda2 btrfs rw' '/dev/sda2[/boot] ext4 rw' '/dev/sda2 xfs rw'; do
    reset
    on
    good_start
    echo "${mount}" >"${S}/boot-mount"
    guard arm
    [[ ! -e "${CFG}" ]] || fail "armed on ${mount}"
    expect "no count on ${mount}" "$(tries)" ""
done
reset
on
good_start
: >"${ROOT}/usr/share/amethystora/sealed"
guard arm
guard good
[[ ! -e "${CFG}" ]] || fail "armed on a sealed image"
[[ ! -s "${RAN}" ]] || fail "it ran something on a sealed image: $(cat "${RAN}")"
guard set on 2>/dev/null
expect "set on is refused on a sealed image" "$?" 1
/usr/bin/rm -f "${ON}"

# A /boot that is read-only is made writable and read-only again
reset
on
good_start
echo '/dev/sda2 ext4 ro,relatime' >"${S}/boot-mount"
guard arm
grep -q 'mount -o remount,rw /boot' "${RAN}" || fail "/boot was not made writable"
grep -q 'mount -o remount,ro /boot' "${RAN}" || fail "/boot was not made read-only again"

# A new system without the guard is never armed for: nothing would say its start was good, and a fine start would be sent back
reset
on
good_start
: >"${S}/noguard-new"
/usr/bin/rm -rf "${ROOT}/ostree/deploy"
world_build
guard arm
[[ ! -e "${CFG}" ]] || fail "armed for a new system that has no guard"
expect "no count for a system without the guard" "$(tries)" ""
reset
on
good_start
/usr/bin/rm -f "${ROOT}/ostree/deploy/default/deploy/${NEW}.0/etc/systemd/system/graphical.target.wants/amethystora-boot-guard-good.service"
guard arm
[[ ! -e "${CFG}" ]] || fail "armed for a new system whose good unit is not enabled"
reset
on
good_start
/usr/bin/rm -rf "${ROOT}/ostree/deploy"
guard arm
[[ ! -e "${CFG}" ]] || fail "armed when the new system's tree could not be looked at"

# ---- A good start leaves nothing ----
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}"
run_as new
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=2/' "${ENV}"
guard good
armed_now && fail "a good start left custom.cfg"
expect "a good start left the count" "$(tries)" ""
[[ -e "${GOOD_NOW}" ]] || fail "a good start was not noted"
expect "the good system is on record" "$(cat "${STATE}/good")" "${NEW}.0"
[[ "$(rollbacks)" == 0 ]] || fail "a good start rolled something back"

# A deployment staged for the next restart, ahead of the one running, is not the default: the new system that started fine on
# its last try (nothing left to count) is not mistaken for the old one
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}"
run_as new
{ printf '  default %s.0 (staged)\n' "${LATER}"; cat "${S}/ostree-status"; } >"${S}/ostree-status.n" && /usr/bin/mv "${S}/ostree-status.n" "${S}/ostree-status"
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
guard good
[[ "$(rollbacks)" == 0 ]] || fail "a staged deployment ahead of the new system made it look like the old one: it was rolled back"
armed_now && fail "a good start of the new system, with an update staged, left the entry"
expect "the staged deployment did not make the new system any less good" "$(cat "${STATE}/good")" "${NEW}.0"

# No session on the seat: not good, and nothing taken away
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}" "${S}/session"
run_as new
guard good 2>/dev/null
armed_now || fail "good with no session took the entry away"
[[ ! -e "${GOOD_NOW}" ]] || fail "good with no session counted"

# ---- Going back ----
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}"
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
guard good 2>/dev/null
expect "out of tries on the old system: rolled back once" "$(rollbacks)" 1
grep -q '^rpm-ostree' "${RAN}" && fail "rpm-ostree ran on top of a rollback that took, and would swap it straight back"
armed_now && fail "gone back, the entry stayed"
expect "gone back, the count stayed" "$(tries)" ""
expect "what went back" "$(jq -r '.from + " " + .to + " " + .digest' "${STATE}/went-back.json")" "new-1 old-1 sha256:bad"
expect "went-back is readable by the accounts" "$(stat -c %a "${STATE}/went-back.json")" 644
[[ ! -e "${STATE}/rolling-back.json" ]] || fail "the record of the rollback was left"
# The old system, now the default, keeps saying so; another system that started well ends it
guard begin
guard good 2>/dev/null
[[ -e "${STATE}/went-back.json" ]] || fail "the next start of the old system forgot it went back"
echo '{"system":"another"}' >"${STATE}/went-back.json"
guard good 2>/dev/null
[[ ! -e "${STATE}/went-back.json" ]] || fail "a newer system that started well did not end it"

# The rollback failing leaves the way back as it is
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}"
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
: >"${S}/rollback-fails"
: >"${S}/rpm-fails"
guard good 2>/dev/null
armed_now || fail "the rollback failed and the entry went"
expect "the rollback failed and the count went" "$(tries)" 0
[[ ! -e "${STATE}/rolling-back.json" ]] || fail "the rollback failed and left its record"
[[ ! -e "${STATE}/went-back.json" ]] || fail "the rollback failed and it said it went back"
expect "the rollback failed, the machine still starts" "$(grub_start)" old

# The false case: /var knows the new system started fine, so nothing is rolled back, ever
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}"
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
echo "${NEW}.0" >"${STATE}/good"
guard good 2>/dev/null
[[ "$(rollbacks)" == 0 ]] || fail "a system on record as good was rolled back"
armed_now && fail "the false case left the entry"
[[ ! -e "${STATE}/went-back.json" ]] || fail "the false case said it went back"

# Picked by hand from the boot menu, with tries left: nothing happens
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}"
guard good 2>/dev/null
[[ "$(rollbacks)" == 0 ]] || fail "an older system picked by hand was rolled back into"
armed_now || fail "picked by hand, the entry went"
[[ ! -e "${GOOD_NOW}" ]] || fail "picked by hand, it counted as good for arming"

# ---- A block that belongs to deployments that are gone ----
# The new system did not start, the network was up, a newer image was staged and finalized: the old system is dropped
reset
on
good_start
guard arm
armed_now || fail "stale block: not armed to begin with"
/usr/bin/rm -f "${GOOD_NOW}"
run_as new
printf '  default %s.0 (pending)\n    Version: later\n* default %s.0\n    Version: new\n' "${LATER}" "${NEW}" >"${S}/ostree-status"
guard arm
armed_now && fail "arm left a block written for deployments that are gone"
expect "arm left a count for deployments that are gone" "$(tries)" ""
# At a good start of a system the block was not written for, it goes too, and nothing is rolled back
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}"
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
printf '  default %s.0\n    Version: later\n* default %s.0\n    Version: old\n' "${LATER}" "${OLD}" >"${S}/ostree-status"
guard good 2>/dev/null
[[ "$(rollbacks)" == 0 ]] || fail "a block written for others made a rollback"
armed_now && fail "good left a block written for deployments that are gone"
# A block of an older format, naming no deployments, is no way back either
reset
on
good_start
guard arm
sed -i '/^# amethystora-boot-guard for /d' "${CFG}"
/usr/bin/rm -f "${GOOD_NOW}"
guard arm
armed_now && fail "arm kept a block that names no deployments"

# ---- The update waits only for the image that did not start ----
reset
on
echo '{"digest":"sha256:bad"}' >"${STATE}/went-back.json"
echo 'Update available: sha256:bad' >"${S}/upgrade-check"
guard update-ok 2>/dev/null && fail "the same image was updated to again"
# bootc still writing after the digest line must not let it through
: >"${S}/upgrade-check-late"
guard update-ok 2>/dev/null && fail "bootc writing after the digest let the same image through"
/usr/bin/rm -f "${S}/upgrade-check-late"
echo 'Update available: sha256:newer' >"${S}/upgrade-check"
guard update-ok 2>/dev/null || fail "a newer image waited"
: >"${S}/upgrade-check"
guard update-ok 2>/dev/null || fail "a failed check held the update"

# ---- set and status ----
reset
guard set on
[[ -e "${ON}" ]] || fail "set on"
expect "status: on" "$(guard status --json | jq -r '[.on, .can, .armed] | @tsv')" "$(printf 'true\ttrue\tfalse')"
good_start
guard arm
expect "status: armed" "$(guard status --json | jq -r '[.armed, .tries] | @tsv')" "$(printf 'true\t3')"
echo '{"digest":"sha256:bad"}' >"${STATE}/went-back.json"
guard set off
[[ ! -e "${ON}" ]] || fail "set off"
armed_now && fail "set off left the entry"
expect "set off left the count" "$(tries)" ""
[[ ! -e "${STATE}/went-back.json" ]] || fail "set off kept the notice of a rollback"
[[ ! -e "${STATE}/cleanup" ]] || fail "set off left its mark though it cleaned up"
guard set maybe 2>/dev/null
expect "set refuses a word" "$?" 64

# Off while /boot refuses: the mark is what lets the next start take the entry away, with the guard off
reset
on
good_start
guard arm
armed_now || fail "off/refusing: not armed to begin with"
: >"${S}/mount-fails"
echo '/dev/sda2 ext4 ro,relatime' >"${S}/boot-mount"
guard set off 2>/dev/null
armed_now || fail "off/refusing: the test cannot refuse"
[[ -e "${STATE}/cleanup" ]] || fail "set off could not clean up and left no mark"
[[ ! -e "${ON}" ]] || fail "set off with /boot refusing left it on"
/usr/bin/rm -f "${S}/mount-fails"
guard good 2>/dev/null
armed_now && fail "the next start did not take away what set off could not"
expect "the next start left the count" "$(tries)" ""
[[ ! -e "${STATE}/cleanup" ]] || fail "the mark outlived the clean-up"
[[ "$(rollbacks)" == 0 ]] || fail "cleaning up rolled something back"
# and with no mark, off does nothing at all
: >"${RAN}"
guard good
guard arm
[[ ! -s "${RAN}" ]] || fail "off with nothing to clean ran something: $(cat "${RAN}")"

# ---- The GRUB script is valid, and counts as promised ----
reset
on
good_start
guard arm
if command -v grub2-script-check >/dev/null; then
    grub2-script-check "${CFG}" || fail "GRUB refuses the script"
fi
# Each value of the ladder falls by one and is saved; 0 goes to the entry, and no other value does
for n in 5 4 3 2 1; do
    grep -A2 "= \"${n}\" \]; then" "${CFG}" | grep -qxF "    set ame_boot_tries=$((n - 1))" || fail "GRUB does not count from ${n}"
done
# Only 0 takes the entry (saving 0b), and 0b takes the default (saving 0): turns, so that no entry can hold the computer
expect "only 0 goes back" "$(awk '/= "0" \]; then/ { on = 1 } /= "0b" \]/ { on = 0 } on' "${CFG}" | grep -c 'set default=')" 1
expect "0 saves 0b" "$(awk '/= "0" \]; then/ { on = 1 } /= "0b" \]/ { on = 0 } on' "${CFG}" | grep -c 'set ame_boot_tries=0b')" 1
expect "0b takes the default" "$(awk '/= "0b" \]/ { on = 1 } /= "5" \]/ { on = 0 } on' "${CFG}" | grep -c 'set default=')" 0
expect "0b saves 0" "$(awk '/= "0b" \]/ { on = 1 } /= "5" \]/ { on = 0 } on' "${CFG}" | grep -c 'set ame_boot_tries=0$')" 1
expect "one entry" "$(grep -c '^menuentry' "${CFG}")" 1
grep -q -- "--unrestricted --id amethystora-previous" "${CFG}" || fail "a boot password would stop the way back"
# As GRUB runs it: three tries of the new system, then the old
expect "GRUB: first try" "$(grub_start)" new
expect "GRUB: second try" "$(grub_start)" new
expect "GRUB: third try" "$(grub_start)" new
expect "GRUB: none left" "$(grub_start)" old
expect "GRUB: then the default, by turns" "$(grub_start)" new
expect "GRUB: then the entry again" "$(grub_start)" old
expect "GRUB: and the default" "$(grub_start)" new

# ---- Both writers of custom.cfg: one at a time ----
reset
on
good_start
mkdir -p "${ROOT}/run/amethystora"
: >"${ROOT}/run/amethystora/boot-cfg.lock"
flock "${ROOT}/run/amethystora/boot-cfg.lock" /usr/bin/sleep 4 &
holder=$!
/usr/bin/sleep 0.5
guard arm 2>/dev/null
armed_now && fail "arm wrote custom.cfg while another command held the lock"
wait "${holder}"
guard arm
armed_now || fail "arm did not write once the lock was free"
expect "no temporary file left" "$(find "${ROOT}/boot/grub2" -name 'custom.cfg.new*' | wc -l)" 0

# Arming itself cut at every step, over a count of 0 an earlier clean-up could not remove: the update is still tried first,
# never the system before it (a block with a count of 0 and no tries would send the computer back unasked)
arm_cut() {
    local cut="$1"
    reset
    on
    good_start
    echo 'ame_boot_tries=0' >>"${ENV}"
    ((cut)) && echo "${cut}" >"${S}/die-at"
    /usr/bin/rm -f "${S}/count" "${S}/died"
    : >"${S}/count-on"
    { guard arm; } 2>/dev/null
    /usr/bin/rm -f "${S}/count-on" "${S}/die-at"
    ARM_STEPS="$(cat "${S}/count" 2>/dev/null || echo 0)"
    # A cut half way through writing custom.cfg leaves the old file or the whole new one, never half of it
    cfg_whole || fail "arming cut at ${cut} ($(cat "${S}/died" 2>/dev/null)): custom.cfg is not a whole script"
    expect "arming cut at ${cut} ($(cat "${S}/died" 2>/dev/null)): the update is tried first" "$(grub_start)" new
}
arm_cut 0
ARM_TOTAL="${ARM_STEPS}"
((ARM_TOTAL >= 3)) || fail "arming made only ${ARM_TOTAL} changes"
for ((cut = 1; cut <= ARM_TOTAL; cut++)); do
    arm_cut "${cut}"
done

# ---- A rollback that does not take: nothing is taken away unless the old system is the default afterwards ----
roll_setup() {
    reset
    on
    good_start
    guard arm
    /usr/bin/rm -f "${GOOD_NOW}"
    sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
}
# bootc fails after its swap: it took, so rpm-ostree must not run (it would swap back), and the way back goes
roll_setup
: >"${S}/rollback-fails-after"
guard good 2>/dev/null
expect "bootc failing after its swap: the default" "$(default_now)" old
grep -q '^rpm-ostree' "${RAN}" && fail "rpm-ostree ran after a bootc rollback that took"
armed_now && fail "bootc failing after its swap: the entry stayed"
[[ -e "${STATE}/went-back.json" ]] || fail "bootc failing after its swap: nothing said it went back"
# bootc exits 0 and changes nothing, rpm-ostree does swap: the second is the way
roll_setup
: >"${S}/rollback-noop"
guard good 2>/dev/null
expect "bootc doing nothing, rpm-ostree swapping: the default" "$(default_now)" old
[[ "$(grep -c '^rpm-ostree rollback' "${RAN}")" == 1 ]] || fail "rpm-ostree was not tried once after a rollback that did nothing"
[[ -e "${STATE}/went-back.json" ]] || fail "rpm-ostree swapping: nothing said it went back"
# bootc fails before it swaps, rpm-ostree swaps
roll_setup
: >"${S}/rollback-fails"
guard good 2>/dev/null
expect "bootc failing, rpm-ostree swapping: the default" "$(default_now)" old
# Neither changes anything (one of them exits 0): everything stays, nothing is said, the machine still starts the old system
for how in rollback-noop rollback-fails; do
    for rpm in rpm-noop rpm-fails; do
        roll_setup
        : >"${S}/${how}"
        : >"${S}/${rpm}"
        guard good 2>/dev/null
        expect "${how} and ${rpm}: the default is untouched" "$(default_now)" new
        armed_now || fail "${how} and ${rpm}: the way back was taken away though nothing was rolled back"
        expect "${how} and ${rpm}: the count" "$(tries)" 0
        [[ ! -e "${STATE}/went-back.json" && ! -e "${STATE}/rolling-back.json" ]] || fail "${how} and ${rpm}: it said it went back"
        expect "${how} and ${rpm}: the machine still starts" "$(grub_start)" old
    done
done

# ---- The same kernel, arguments and version in both: ostree swaps them without a new boot version, so the entry would start the other one ----
reset
on
good_start
: >"${S}/sameboot"
world_build
guard arm
armed_now && fail "armed for two deployments of one boot configuration"
expect "no count for two deployments of one boot configuration" "$(tries)" ""
# the model does what ostree does: such a swap keeps the boot version and the links, and swaps what their serials mean
bootc rollback >/dev/null
[[ -d "${ROOT}/boot/loader.1" ]] || fail "the model flipped the boot version for a swap of one boot configuration"
expect "the serial 1 now means the other deployment" "$(awk '{ print $2 }' "${S}/order")" new
# Different versions or kernels, or arguments: armed as before
reset
on
good_start
sed -i 's/^options rw/options rw quiet/' "${ROOT}/boot/loader/entries/ostree-1.conf"
: >"${S}/sameboot"
world_build
sed -i 's/^options rw/options rw quiet/' "${ROOT}/boot/loader/entries/ostree-1.conf"
guard arm
armed_now || fail "not armed for two deployments whose arguments differ"

# ---- No entry can hold the computer: a wrong one is taken every other start at most ----
reset
on
good_start
guard arm
NEW_BROKEN=1
for i in 1 2 3; do start_machine; done
# the old system is the default now, and the entry still names the broken one (as after a swap that kept the links)
echo "old new" >"${S}/order"
echo old >"${S}/running"
world_build
sed -i "s|default/${OLD}/0|default/${NEW}/0|; s|default-old/|default-new/|g" "${CFG}"
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
grep -q "default/${NEW}/0" "${CFG}" || fail "the test could not make the entry name the broken system"
seen=""
for i in 1 2 3 4; do
    start_machine || { fail "a wrong entry: start ${i} found nothing to start"; break; }
    seen="${seen}${STARTED} "
    [[ "${STARTED}" == old ]] && break
done
[[ "${seen}" == "new old " || "${seen}" == "old " ]] || fail "a wrong entry held the computer: ${seen}"
armed_now && fail "the system that started did not take the wrong entry away"
expect "a wrong entry: the settled start" "$(grub_start)" old

# ---- The kernel the entry names is gone: the entry is not followed ----
reset
on
good_start
guard arm
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
/usr/bin/rm -f "${ROOT}/boot/ostree/default-old/vmlinuz"
expect "no kernel, no entry: the default starts" "$(grub_start)" new

# ---- A soft reboot keeps /run: begin forgets that the last start was good ----
reset
on
good_start
guard arm
armed_now || fail "soft reboot: not armed"
[[ -e "${GOOD_NOW}" ]] || fail "soft reboot: the test lost the good start"
run_as new
guard begin
[[ ! -e "${GOOD_NOW}" ]] || fail "begin kept the last start's good mark across a soft reboot"
guard good
armed_now && fail "soft reboot: a good start of the new system left the entry"
[[ -e "${GOOD_NOW}" ]] || fail "soft reboot: the good start was not noted"
[[ "$(rollbacks)" == 0 ]] || fail "soft reboot: something was rolled back"
# a soft reboot into a system that never gets its desktop arms nothing at the shutdown after it
reset
on
good_start
guard begin
guard arm
armed_now && fail "a start that was never good armed"

# ---- A slow greeter, or a stray unit holding graphical.target: a working machine is not sent back ----
reset
on
good_start
guard arm
/usr/bin/rm -f "${GOOD_NOW}" "${S}/session"
run_as new
echo 6 >"${S}/session-after"
guard good 2>/dev/null
armed_now || fail "a bounded wait took the entry away without a session"
AMETHYSTORA_BOOT_GUARD_WAIT=0 guard good 2>/dev/null
armed_now && fail "a greeter that took a while to come up never counted as good"
expect "a slow greeter: the good system is on record" "$(cat "${STATE}/good")" "${NEW}.0"

# ---- Writes are flushed to the disk itself: every freeze has its thaw ----
reset
on
good_start
guard arm
grep -q '^fsfreeze -f /boot' "${RAN}" || fail "arm did not freeze /boot after its write"
frozen_clean || fail "arm left /boot frozen or thawed it twice"
: >"${RAN}"
run_as new
guard good
grep -q '^fsfreeze -f /boot' "${RAN}" || fail "a good start did not flush what it took away"
frozen_clean || fail "a good start left /boot frozen"
: >"${RAN}"
guard good
grep -q '^fsfreeze' "${RAN}" && fail "a good start with nothing to take away froze /boot"
# a failed thaw is tried again
reset
on
good_start
cat >"${BIN}/fsfreeze" <<'XEOF'
#!/usr/bin/bash
echo "fsfreeze $*" >>"${AMETHYSTORA_BOOT_GUARD_ROOT}/state/log"
[[ "$1" != -u ]] || [[ -e "${AMETHYSTORA_BOOT_GUARD_ROOT}/state/thawed-once" ]] || { : >"${AMETHYSTORA_BOOT_GUARD_ROOT}/state/thawed-once"; exit 1; }
XEOF
guard arm
[[ "$(grep -c '^fsfreeze -u' "${RAN}")" -ge 2 ]] || fail "a thaw that failed was not tried again"
/usr/bin/rm -f "${S}/thawed-once"
cat >"${BIN}/fsfreeze" <<'XEOF'
#!/usr/bin/bash
echo "fsfreeze $*" >>"${AMETHYSTORA_BOOT_GUARD_ROOT}/state/log"
XEOF

# What it says is still heard after the lock was taken (the lock's open must not silence the shell), and a count that cannot
# be written leaves no entry that would count against a system that was not tried
reset
on
good_start
: >"${S}/editenv-fails"
out="$(guard arm 2>&1)"
grep -q 'could not write the count' <<<"${out}" || fail "after taking the lock, a failure was not said: [${out}]"
armed_now && fail "a count that could not be written left the entry"

# ---- A power cut at every step ----
# The old system goes back after three tries of a new one that never reaches its desktop. Whatever step the power goes at,
# every start that follows must find a system, and the end is the old system, default and running, with nothing left.
steps_fail() {  # CUT, then mountfail for a /boot that refuses the last writes
    local cut="$1" variant="${2:-}" i
    reset
    on
    good_start
    guard arm
    NEW_BROKEN=1
    [[ "${variant}" == mountfail || "${variant}" == ro ]] && echo '/dev/sda2 ext4 ro,relatime' >"${S}/boot-mount"
    for i in 1 2 3; do
        start_machine || { fail "cut ${cut} ${variant}: try ${i} of the new system found nothing to start"; return; }
        [[ "${STARTED}" == new ]] || { fail "cut ${cut} ${variant}: try ${i} started ${STARTED}"; return; }
    done
    [[ "${variant}" == mountfail ]] && : >"${S}/mount-fails"
    start_machine "${cut}" || { fail "cut ${cut} ${variant}: the old system could not be started"; return; }
    [[ "${STARTED}" == old ]] || fail "cut ${cut} ${variant}: the fourth start was ${STARTED}"
    TOTAL="$(cat "${S}/count" 2>/dev/null || echo 0)"
    DIED="$(cat "${S}/died" 2>/dev/null)"
    [[ "${variant}" == mountfail ]] && /usr/bin/rm -f "${S}/mount-fails"
    cfg_whole || fail "cut ${cut} ${variant} (${DIED}): custom.cfg is not whole"
    # The computer takes turns at the end of the count, so a cut before the rollback means one more try of the broken
    # system (and never more than one in a row); the starts go on until nothing is left, and each must find a system
    local last=""
    for i in {1..12}; do
        start_machine || { fail "cut ${cut} ${variant} (${DIED}): start ${i} after it found nothing to start"; return; }
        [[ "${last}${STARTED}" != newnew ]] || fail "cut ${cut} ${variant} (${DIED}): the broken system started twice in a row"
        last="${STARTED}"
        if ! armed_now && [[ -z "$(tries)" && "$(default_now)" == old && ! -e "${STATE}/rolling-back.json" ]]; then
            break
        fi
    done
    for i in 1 2 3; do
        start_machine || { fail "cut ${cut} ${variant} (${DIED}): a settled start ${i} found nothing to start"; return; }
        [[ "${STARTED}" == old ]] || fail "cut ${cut} ${variant} (${DIED}): a settled start ${i} was ${STARTED}"
    done
    frozen_clean || fail "cut ${cut} ${variant} (${DIED}): /boot was left frozen or thawed without a freeze"
    expect "cut ${cut} ${variant} (${DIED}): rolled back once" "$(rollbacks)" 1
    armed_now && fail "cut ${cut} ${variant} (${DIED}): the entry is still there"
    expect "cut ${cut} ${variant} (${DIED}): the count" "$(tries)" ""
    expect "cut ${cut} ${variant} (${DIED}): the default" "$(head -c 3 "${S}/order")" old
    expect "cut ${cut} ${variant} (${DIED}): what went back" "$(jq -r '.from + " " + .to' "${STATE}/went-back.json" 2>/dev/null)" "new-1 old-1"
    [[ ! -e "${STATE}/rolling-back.json" ]] || fail "cut ${cut} ${variant} (${DIED}): the rollback's record is still there"
}
steps_fail 0
TOTAL_FAIL="${TOTAL}"
((TOTAL_FAIL >= 5)) || fail "the old system's good start made only ${TOTAL_FAIL} changes; the cuts below would test too little"
for ((cut = 1; cut <= TOTAL_FAIL; cut++)); do
    steps_fail "${cut}"
done
# The same with /boot read-only: the remounts are steps too
steps_fail 0 ro
TOTAL_RW="${TOTAL}"
for ((cut = 1; cut <= TOTAL_RW; cut++)); do
    steps_fail "${cut}" ro
done
# and with the last writes refused
steps_fail 0 mountfail
TOTAL_RO="${TOTAL}"
for ((cut = 1; cut <= TOTAL_RO; cut++)); do
    steps_fail "${cut}" mountfail
done

# A new system that started fine: whatever step the power goes at in its good start, it stays the default and keeps starting,
# nothing is rolled back, and nothing is left
steps_ok() {
    local cut="$1" i
    reset
    on
    good_start
    guard arm
    start_machine "${cut}" || { fail "ok cut ${cut}: nothing to start"; return; }
    TOTAL="$(cat "${S}/count" 2>/dev/null || echo 0)"
    DIED="$(cat "${S}/died" 2>/dev/null)"
    cfg_whole || fail "ok cut ${cut} (${DIED}): custom.cfg is not whole"
    for i in 1 2 3; do
        start_machine || { fail "ok cut ${cut} (${DIED}): start ${i} after it found nothing to start"; return; }
        [[ "${STARTED}" == new ]] || fail "ok cut ${cut} (${DIED}): start ${i} after it was ${STARTED}"
    done
    expect "ok cut ${cut} (${DIED}): nothing rolled back" "$(rollbacks)" 0
    armed_now && fail "ok cut ${cut} (${DIED}): the entry is still there"
    expect "ok cut ${cut} (${DIED}): the count" "$(tries)" ""
    expect "ok cut ${cut} (${DIED}): the default" "$(head -c 3 "${S}/order")" new
}
steps_ok 0
TOTAL_OK="${TOTAL}"
((TOTAL_OK >= 3)) || fail "the new system's good start made only ${TOTAL_OK} changes"
for ((cut = 1; cut <= TOTAL_OK; cut++)); do
    steps_ok "${cut}"
done
echo "test-boot-guard: power cut at each of ${TOTAL_FAIL} steps going back, ${TOTAL_RW} with /boot read-only, ${TOTAL_RO} with it refusing, ${TOTAL_OK} at a good start"

# The finding that made this: the swap done, the entry still there. The entry must not be followed (the link it names is gone)
reset
on
good_start
guard arm
NEW_BROKEN=1
for i in 1 2 3; do start_machine; done
run_as old
bootc rollback
armed_now || fail "the model lost the entry on the swap"
expect "the swap done, the entry left: the machine" "$(grub_start)" old
# Without the check the same entry is followed to a root that is gone
sed -i -E '/^( {4}| {8})if /d; /^( {4}| {8})fi$/d' "${CFG}"
sed -i 's/^ame_boot_tries=.*/ame_boot_tries=0/' "${ENV}"
expect "without the check, the entry is followed to a root that is gone" "$(grub_start)" FAIL

/usr/bin/rm -rf "${ROOT}"
if ((failures)); then
    echo "test-boot-guard: ${failures} failed" >&2
    exit 1
fi
echo "test-boot-guard: ok"
