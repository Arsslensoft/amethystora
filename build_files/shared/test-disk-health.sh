#!/usr/bin/bash
# amethystora-disk-health (R7-53), against a stand-in machine: which disks are the system's, a disk asleep keeping what it
# said, told once per change and only when somebody is there to tell, and what the state file holds.

set -euo pipefail

HEALTH="${1:-/usr/libexec/amethystora-disk-health}"
TEXT="${2:-/usr/libexec/amethystora-security-text}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
export AMETHYSTORA_DISK_ROOT="${T}/root" STAND_IN="${T}" AMETHYSTORA_SECURITY_TEXT="${TEXT}" AMETHYSTORA_DISK_RAID="${T}/bin/amethystora-raid"
export PATH="${T}/bin:${PATH}"
mkdir -p "${T}/bin" "${AMETHYSTORA_DISK_ROOT}/var/lib/amethystora"
STATE="${AMETHYSTORA_DISK_ROOT}/var/lib/amethystora/disk-health.json"
UUID=11111111-0000-4000-8000-000000000000

fail() { echo "test-disk-health: $*" >&2; exit 1; }

cat >"${T}/bin/findmnt" <<EOF
#!/usr/bin/bash
echo "/dev/mapper/luks-1[/root] btrfs ${UUID}"
EOF
# lsblk -s: the tree upward from the mapping, through its partition, to the disk
cat >"${T}/bin/lsblk" <<'EOF'
#!/usr/bin/bash
device="${*: -1}"
case "${device}" in
    /dev/sdb1) echo '{"blockdevices":[{"path":"/dev/sdb1","type":"part","model":null,"serial":null,"children":[{"path":"/dev/sdb","type":"disk","model":"Second","serial":"S2"}]}]}' ;;
    *)
        disk="$(cat "${STAND_IN}/diskpath" 2>/dev/null || echo /dev/sda)"
        model="$(cat "${STAND_IN}/model" 2>/dev/null || printf 'First\tDisk')"
        jq -nc --arg disk "${disk}" --arg model "${model}" '{blockdevices: [{path: "/dev/mapper/luks-1", type: "crypt", model: null, serial: null,
            children: [{path: "/dev/sda3", type: "part", model: null, serial: null,
                children: [{path: $disk, type: "disk", model: $model, serial: "S1"}]}]}]}'
        ;;
esac
EOF
# The pools' check: it runs only when there is something for it to watch (the file), and tells what the file says
cat >"${T}/bin/amethystora-raid" <<'EOF'
#!/usr/bin/bash
case "$1" in
    present) [[ -e "${STAND_IN}/pools" ]] ;;
    status) cat "${STAND_IN}/pools" ;;
esac
EOF
cat >"${T}/bin/smartctl" <<'EOF'
#!/usr/bin/bash
[[ " $* " == *" -n standby "* ]] || { echo "woke a disk" >>"${STAND_IN}/woke"; exit 1; }
echo "$*" >>"${STAND_IN}/smartctl-args"
cat "${STAND_IN}/smart" 2>/dev/null
EOF
chmod +x "${T}/bin/"*

passed='{"smart_status":{"passed":true},"ata_smart_attributes":{"table":[{"id":5,"raw":{"value":0}},{"id":9,"raw":{"value":7000}},{"id":197,"raw":{"value":0}}]}}'
sectors() { printf '{"smart_status":{"passed":true},"ata_smart_attributes":{"table":[{"id":5,"raw":{"value":%s}},{"id":9,"raw":{"value":7000}},{"id":197,"raw":{"value":1}}]}}' "$1"; }
failed='{"smart_status":{"passed":false}}'
nvme_ok='{"smart_status":{"passed":true},"nvme_smart_health_information_log":{"critical_warning":0,"media_errors":0}}'
nvme_worn='{"smart_status":{"passed":true},"nvme_smart_health_information_log":{"critical_warning":4,"media_errors":0}}'
nvme_errors='{"smart_status":{"passed":true},"nvme_smart_health_information_log":{"critical_warning":0,"media_errors":3}}'

check() { bash "${HEALTH}" check; }
says() { # what, expected output of a check
    local got
    got="$(check)"
    [[ "${got}" == "$2" ]] || fail "$1: told [${got}], wanted [$2]"
}
field() { jq -r ".disks[0].$1" "${STATE}"; }

mkdir -p "${AMETHYSTORA_DISK_ROOT}/run/user/1000"
: >"${AMETHYSTORA_DISK_ROOT}/run/user/1000/bus"

echo "${passed}" >"${T}/smart"
says "a healthy disk" ""
[[ "$(field state)" == passed ]] || fail "it says it passed"
[[ "$(field path)" == /dev/sda ]] || fail "the disk under the LUKS mapping is the one read"
[[ "$(stat -c %a "${STATE}")" == 644 ]] || fail "readable by everyone, as disks.json is"
if jq -e '.. | strings | select(test("Documents|home"))' "${STATE}" >/dev/null; then fail "a path of a person's"; fi

sectors 2 >"${T}/smart"
says "bad sectors" "told /dev/sda errors"
[[ "$(field errors)" == 3 ]] || fail "5 and 197 and 198 add up"
says "said once" ""
sectors 5 >"${T}/smart"
says "the count rose" "told /dev/sda errors"
says "said once more" ""
sectors 1 >"${T}/smart"
says "the count fell: nothing" ""

# Asleep: no reading, and what it said last stands
: >"${T}/smart"
says "asleep" ""
[[ "$(field state)" == errors ]] || fail "a disk asleep keeps what it said"
grep -q -- "-n standby" "${T}/smartctl-args" || fail "asked to stay asleep"
[[ ! -e "${T}/woke" ]] || fail "a disk was woken"

echo "${failed}" >"${T}/smart"
says "failing" "told /dev/sda failed"
says "failing, said once" ""
sectors 50 >"${T}/smart"
says "errors after failing add nothing" ""

# Nobody there: it waits, so that the first person to log in is told
rm "${STATE}"
rm "${AMETHYSTORA_DISK_ROOT}/run/user/1000/bus"
echo "${failed}" >"${T}/smart"
says "nobody to tell" ""
: >"${AMETHYSTORA_DISK_ROOT}/run/user/1000/bus"
says "somebody logged in" "told /dev/sda failed"

# NVMe
rm "${STATE}"
echo "${nvme_ok}" >"${T}/smart"
says "NVMe, healthy" ""
echo "${nvme_errors}" >"${T}/smart"
says "NVMe's media errors" "told /dev/sda errors"
echo "${nvme_worn}" >"${T}/smart"
says "NVMe's critical warning" "told /dev/sda failed"

# A disk that cannot say anything has no state, and nothing is told
rm "${STATE}"
echo '{"device":{"name":"/dev/vda"}}' >"${T}/smart"
says "no SMART at all" ""
[[ "$(field state)" == null ]] || fail "a disk that cannot say has no state"

# The system over two disks: both read, and a failing one is the pools' check's to tell
rm "${STATE}"
mkdir -p "${AMETHYSTORA_DISK_ROOT}/sys/fs/btrfs/${UUID}/devices"
: >"${AMETHYSTORA_DISK_ROOT}/sys/fs/btrfs/${UUID}/devices/sdb1"
: >"${AMETHYSTORA_DISK_ROOT}/sys/fs/btrfs/${UUID}/devices/dm-0"
echo '{"pools":[{"mounted":true,"devices":[{"disk":{"path":"/dev/sda"},"error_count":0},{"disk":{"path":"/dev/sdb"},"error_count":0}]}]}' >"${T}/pools"
echo "${failed}" >"${T}/smart"
says "spread over disks, failing" ""
[[ "$(jq '.disks | length' "${STATE}")" == 2 ]] || fail "both of the system's disks are read"
sectors 3 >"${T}/smart"
says "spread over disks, errors" "told /dev/sda errors"
# ...but errors btrfs counts for that disk are the pools' to tell
echo '{"pools":[{"mounted":true,"devices":[{"disk":{"path":"/dev/sda"},"error_count":4},{"disk":{"path":"/dev/sdb"},"error_count":4}]}]}' >"${T}/pools"
sectors 9 >"${T}/smart"
says "errors the pools count are told by the pools" ""

# LVM or md over two disks is no pool: the pools' check does not run, and a failing disk is told here
rm -f "${T}/pools"
rm -rf "${STATE}"
echo "${passed}" >"${T}/smart"
says "two disks, healthy" ""
echo "${failed}" >"${T}/smart"
says "two disks that are no pool, failing" "told /dev/sda failed"
# ...and a btrfs on two partitions of one disk is: that disk is the pools' check's to tell
echo '{"pools":[{"mounted":true,"devices":[{"disk":{"path":"/dev/sda"},"error_count":0},{"disk":{"path":"/dev/sda"},"error_count":0}]}]}' >"${T}/pools"
rm -rf "${AMETHYSTORA_DISK_ROOT}/sys/fs/btrfs" "${STATE}"
echo "${passed}" >"${T}/smart"
says "one disk, two partitions in a pool, healthy" ""
echo "${failed}" >"${T}/smart"
says "one disk, two partitions in a pool, failing" ""
rm -f "${T}/pools"

# A disk first seen with bad sectors has them as its baseline: a rise is news, the count itself is not
rm -f "${STATE}"
sectors 7 >"${T}/smart"
says "an old reallocated sector on the first run" ""
[[ "$(field errors)" == 8 ]] || fail "it still counts them: $(field errors)"
says "and on the next" ""
sectors 9 >"${T}/smart"
says "a rise past the baseline" "told /dev/sda errors"
# ...but a disk that says it is failing is not given a baseline
rm -f "${STATE}"
echo "${failed}" >"${T}/smart"
says "failing on the first run" "told /dev/sda failed"
# A raw value that is no plain count says nothing
rm -f "${STATE}"
echo '{"smart_status":{"passed":true},"ata_smart_attributes":{"table":[{"id":5,"raw":{"value":281474976710656}}]}}' >"${T}/smart"
says "a packed raw value" ""
[[ "$(field state)" == passed ]] || fail "a raw value above 65535 is no count: $(field state)"

# A disk is its serial's checksum, whatever the path: two NVMe disks swap names between starts
rm -f "${STATE}"
echo "${passed}" >"${T}/smart"
check
[[ "$(field serial)" == "$(printf S1 | sha256sum | cut -c1-32)" ]] || fail "the file holds a checksum of the serial: $(field serial)"
if grep -q '"S1"' "${STATE}"; then fail "the serial itself is in the file"; fi
echo "${failed}" >"${T}/smart"
says "failing" "told /dev/sda failed"
echo /dev/nvme9n1 >"${T}/diskpath"
says "the same disk under another name is not told again" ""
[[ "$(field path)" == /dev/nvme9n1 ]] || fail "the path is the latest"
rm -f "${T}/diskpath"

# What a disk calls itself is plain text that cannot act or read as a link, in the file and on a terminal
rm -f "${STATE}"
printf 'WD\x1b[31m red www.evil.example/x https://a.b/c Disk' >"${T}/model"
echo "${passed}" >"${T}/smart"
check
model="$(field model)"
[[ "${model}" != *$'\x1b'* && "${model}" != */* && "${model}" != *www.* ]] || fail "the model is not plain: ${model}"
[[ "${model}" == WD* ]] || fail "the model is still its name: ${model}"
jq -c '.disks[0].model = "X[2Jevil/www.x.org"' "${STATE}" >"${STATE}.edit" && mv "${STATE}.edit" "${STATE}"
shown="$(bash "${HEALTH}" status)"
[[ "${shown}" != *$'\x1b'* && "${shown}" != */* && "${shown}" != *www.* ]] || fail "status prints it as written: ${shown}"
rm -f "${T}/model"

# status, for anyone
[[ "$(bash "${HEALTH}" status --json | jq -c '.disks[0] | keys')" == '["errors","model","state"]' ]] || fail "status names state and model only"
rm "${STATE}"
[[ "$(bash "${HEALTH}" status --json)" == '{"time":null,"disks":[]}' ]] || fail "status before any check"

# A disk first read with bad sectors already counted: they are its baseline, and only a later rise is news
echo "${passed}" >"${T}/smart"
rm -f "${STATE}"
sectors 1 >"${T}/smart"
says "an old bad sector is not news" ""
[[ "$(field state)" == errors && "$(field errors)" == 2 ]] || fail "the baseline is kept as errors, 2"
sectors 3 >"${T}/smart"
says "a rise after the baseline" "told /dev/sda errors"

echo "test-disk-health: fine"
