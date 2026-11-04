#!/usr/bin/bash
# What amethystora-update-check finds, for 20-tests.sh, against stand-ins for rpm-ostree, bootc, skopeo and systemctl:
# the driver uupd would pick, a layered package with a newer version while the image is the same, a new image (also
# behind a list of architectures), a look that fails and keeps what the last one found, an update already staged,
# and the notice said once, only while automatic updates are off.

set -euo pipefail

T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
fail() { echo "test-update-check: $*" >&2; exit 1; }
HELPER="${AMETHYSTORA_UPDATE_CHECK:-/usr/libexec/amethystora-update-check}"
mkdir -p "${T}/bin" "${T}/state"

# Each stand-in answers from a file of the test's, and exits with the code in the file beside it
for tool in rpm-ostree bootc skopeo; do
    cat >"${T}/bin/${tool}" <<STANDIN
#!/usr/bin/bash
case "\$*" in
    "status --json") cat "${T}/${tool}.status" ;;
    "status --format=json") cat "${T}/${tool}.status"; exit "\$(cat "${T}/${tool}.status.code" 2>/dev/null || echo 0)" ;;
    "upgrade --check") cat "${T}/${tool}.check"; exit "\$(cat "${T}/${tool}.check.code")" ;;
    *--raw*) cat "${T}/${tool}.raw" ;;
    inspect*) cat "${T}/${tool}.inspect" ;;
    *) echo "\${0##*/}: not a stand-in for \$*" >&2; exit 64 ;;
esac
STANDIN
    chmod +x "${T}/bin/${tool}"
done
# systemctl: automatic updates on while ${T}/auto exists; what is started is written down
cat >"${T}/bin/systemctl" <<STANDIN
#!/usr/bin/bash
case "\$*" in
    *is-enabled*uupd.timer*) [[ -e "${T}/auto" ]] ;;
    start*) printf '%s\n' "\$*" >>"${T}/started" ;;
    *) exit 0 ;;
esac
STANDIN
cat >"${T}/notify" <<STANDIN
#!/usr/bin/bash
printf '%s\n' "\$*" >>"${T}/said"
STANDIN
chmod +x "${T}/bin/systemctl" "${T}/notify"
export PATH="${T}/bin:${PATH}" AMETHYSTORA_UPDATES_DIR="${T}/state" AMETHYSTORA_UPDATES_NOTIFY="${T}/notify"
unset TRIGGER_UNIT
FOUND="${T}/state/available.json"

deployments() {
    # $1: whether one is staged
    cat >"${T}/rpm-ostree.status" <<JSON
{"deployments": [
  {"booted": true, "staged": $1, "checksum": "booted1", "packages": ["htop"],
   "container-image-reference": "ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:stable",
   "container-image-reference-digest": "sha256:old"}],
 "cached-update": {"version": "x", "rpm-diff": {"upgraded": [[1, "htop", ["3.3-1.fc44", "x86_64"], ["3.4-1.fc44", "x86_64"]]],
   "downgraded": [], "removed": [], "added": []}}}
JSON
}
same_image() { echo '{"Digest": "sha256:old", "Labels": {"org.opencontainers.image.version": "44.20261103"}}' >"${T}/skopeo.inspect"; }

# --- Which updates this machine: bootc, unless it has packages layered onto it, or bootc cannot say --------------

echo '{"status": {"booted": {"incompatible": true}, "staged": null}}' >"${T}/bootc.status"
[[ "$("${HELPER}" driver)" == rpm-ostree ]] || fail "a layered machine is not updated with rpm-ostree"
echo '{"status": {"booted": {"incompatible": false}, "staged": null}}' >"${T}/bootc.status"
[[ "$("${HELPER}" driver)" == bootc ]] || fail "a machine without layered packages is not updated with bootc"
echo 1 >"${T}/bootc.status.code"
[[ "$("${HELPER}" driver)" == rpm-ostree ]] || fail "a machine bootc cannot read is not updated with rpm-ostree"
rm "${T}/bootc.status.code"
echo '{"status": {"booted": {"incompatible": true}, "staged": null}}' >"${T}/bootc.status"

# --- A layered package with a newer version, the image the same ------------------------------------------------

deployments false
echo 'AvailableUpdate:' >"${T}/rpm-ostree.check"
echo 0 >"${T}/rpm-ostree.check.code"
same_image
"${HELPER}" check || fail "the look failed: $(cat "${FOUND}" 2>/dev/null)"
jq -e '.booted == "booted1" and .driver == "rpm-ostree" and .system == null and .error == null
    and .packages == [{"name": "htop", "from": "3.3-1.fc44", "to": "3.4-1.fc44"}]' "${FOUND}" >/dev/null ||
    fail "the newer htop is not found: $(cat "${FOUND}")"
[[ "$(stat -c %a "${FOUND}")" == 644 ]] || fail "what was found is not readable by everyone"
[[ ! -e "${T}/started" ]] || fail "a look nobody's timer started raised a notice"

# --- A new image, alone and behind a list of architectures ---------------------------------------------------------

echo 'No updates available.' >"${T}/rpm-ostree.check"
echo 77 >"${T}/rpm-ostree.check.code"
echo '{"Digest": "sha256:new", "Created": "2026-11-03T01:00:00Z", "Labels": {"org.opencontainers.image.version": "44.20261103"}}' >"${T}/skopeo.inspect"
echo '{"schemaVersion": 2, "config": {}}' >"${T}/skopeo.raw"
"${HELPER}" check || fail "the look for a new image failed"
jq -e '.system.version == "44.20261103" and .system.digest == "sha256:new" and .system.built == 1793667600 and .packages == []' \
    "${FOUND}" >/dev/null || fail "the new image is not found: $(cat "${FOUND}")"
echo '{"manifests": [{"digest": "sha256:arm", "platform": {"os": "linux", "architecture": "arm64"}},
    {"digest": "sha256:old", "platform": {"os": "linux", "architecture": "amd64"}}]}' >"${T}/skopeo.raw"
"${HELPER}" check || fail "the look behind a list failed"
[[ "$(uname -m)" != x86_64 ]] || jq -e '.system == null' "${FOUND}" >/dev/null ||
    fail "a list whose image for this machine is the one installed counts as new: $(cat "${FOUND}")"

# --- A look that fails keeps what the last one found ---------------------------------------------------------------

echo 0 >"${T}/rpm-ostree.check.code"
same_image
"${HELPER}" check
echo 'error: Transaction in progress' >"${T}/rpm-ostree.check"
echo 1 >"${T}/rpm-ostree.check.code"
"${HELPER}" check 2>/dev/null && fail "a look that failed says it worked"
jq -e '.error == "error: Transaction in progress" and (.packages | length) == 1 and .failed > 0' "${FOUND}" >/dev/null ||
    fail "a failed look lost what was found, or its error: $(cat "${FOUND}")"
grep -q "Transaction in progress" <<<"$("${HELPER}" status)" || fail "ame update check does not say why the look failed"

# --- Nothing is looked for while an update waits for the restart ---------------------------------------------------

before="$(cat "${FOUND}")"
deployments true
echo 0 >"${T}/rpm-ostree.check.code"
"${HELPER}" check || fail "a look with an update staged failed"
[[ "$(cat "${FOUND}")" == "${before}" ]] || fail "a look compared against the update already staged"
deployments false

# --- bootc, on a machine without layered packages --------------------------------------------------------------------

echo '{"status": {"booted": {"incompatible": false, "cachedUpdate": {"version": "44.20261104",
    "timestamp": "2026-11-04T01:00:00Z", "imageDigest": "sha256:b"}}, "staged": null}}' >"${T}/bootc.status"
echo 'Update available for: docker://ghcr.io/arsslensoft/amethystora:stable' >"${T}/bootc.check"
echo 0 >"${T}/bootc.check.code"
"${HELPER}" check || fail "the look with bootc failed"
jq -e '.driver == "bootc" and .system.version == "44.20261104" and .system.digest == "sha256:b" and .packages == []' \
    "${FOUND}" >/dev/null || fail "bootc's update is not found: $(cat "${FOUND}")"
echo 'No changes in docker://ghcr.io/arsslensoft/amethystora:stable => sha256:a' >"${T}/bootc.check"
"${HELPER}" check || fail "the look with bootc failed"
jq -e '.system == null' "${FOUND}" >/dev/null || fail "no change from bootc counts as an update"
grep -q "Nothing is waiting" <<<"$("${HELPER}" status)" || fail "ame update check does not say nothing is waiting"

# --- The notice: from the timer only, once, and only while automatic updates are off -----------------------------

echo '{"status": {"booted": {"incompatible": true}, "staged": null}}' >"${T}/bootc.status"
echo 0 >"${T}/rpm-ostree.check.code"
same_image
TRIGGER_UNIT=amethystora-update-check.timer "${HELPER}" check || fail "the timer's look failed"
grep -qx 'start --no-block amethystora-update-check-notice.service' "${T}/started" || fail "the timer's look raised no notice"
touch "${T}/auto"
"${HELPER}" notify
[[ ! -e "${T}/said" ]] || fail "a notice was raised with automatic updates on"
rm "${T}/auto"
"${HELPER}" notify
grep -q -- '--msgid An update is waiting Packages you added to the system have newer versions' "${T}/said" ||
    fail "the notice does not say which packages: $(cat "${T}/said" 2>/dev/null)"
"${HELPER}" notify
[[ "$(wc -l <"${T}/said")" == 1 ]] || fail "the same update was said twice"
exit 0
