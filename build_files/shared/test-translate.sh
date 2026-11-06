#!/usr/bin/bash
# Any language (R7-47): what /usr/libexec/amethystora-translate does against stand-ins: a route through English, a download
# kept only when its pinned SHA-256 holds, nothing of what is read ever written, and the shipped table within the limit.
# Run by 20-tests.sh; HELPER, TABLE and PART name other copies.
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-translate}"
TABLE="${TABLE:-/usr/share/amethystora/translate/models.json}"
PART="${PART:-/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/screenshot/translate.js}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT

fail() {
    echo "test-translate: $*"
    exit 1
}

# The table the image ships: every direction measured within the limit, every file pinned, addresses only over https
jq -e '(.hosts | length) > 0 and (.models_url | startswith("https://")) and (.languages | length) >= 5' "${TABLE}" >/dev/null ||
    fail "the table's addresses"
jq -e '[.directions[] | .peak_mb > 300] | any | not' "${TABLE}" >/dev/null || fail "a direction over the 300 MB limit"
jq -e '[.directions[].files[], .languages[].reading] | all(.sha256 | test("^[0-9a-f]{64}$"))' "${TABLE}" >/dev/null ||
    fail "a file without its SHA-256"
jq -e '[.languages[].reading.url] | all(startswith("https://raw.githubusercontent.com/"))' "${TABLE}" >/dev/null ||
    fail "reading data not from the one host the table names"
jq -e '.hosts == ["firefox-settings-attachments.cdn.mozilla.net", "raw.githubusercontent.com"]' "${TABLE}" >/dev/null ||
    fail "the table's hosts are not the whole list the recipe and the sends row say"
# The translator is built on the machine, in a container of this release pinned by digest, from a source pinned by a hash
jq -e '.engine.tree_sha256 | test("^[0-9a-f]{64}$")' "${TABLE}" >/dev/null || fail "the engine's source is not pinned"
jq -e '[.engine.images | to_entries[] | .value | test("^registry[.]fedoraproject[.]org/fedora:[0-9]+@sha256:[0-9a-f]{64}$")] | all' \
    "${TABLE}" >/dev/null || fail "an engine image is not pinned by digest"
# Each language offered has both directions, or a person could fetch half
jq -e '. as $t | [$t.languages | keys[] | . as $c | ($t.directions["\($c)-en"] and $t.directions["en-\($c)"])] | all' \
    "${TABLE}" >/dev/null || fail "a language without both its directions"

# --- Stand-ins -----------------------------------------------------------------------------------------------------------
ROOT="${T}/root"
export AMETHYSTORA_TRANSLATE_ROOT="${ROOT}"
export HOME="${T}/home" XDG_DATA_HOME="${T}/home/.local/share"
mkdir -p "${ROOT}/usr/bin" "${ROOT}/etc" "${ROOT}/usr/share/amethystora/translate" "${HOME}" "${T}/remote"
ENGINE="${XDG_DATA_HOME}/amethystora/translate/engine/amethystora-bergamot"
echo 'VERSION_ID=45' >"${ROOT}/etc/os-release"
ln -s "$(command -v curl)" "${ROOT}/usr/bin/curl"
ln -s "$(command -v zstd)" "${ROOT}/usr/bin/zstd"
# The translator says which direction it was, by its config's folder
ENGINE_STANDIN='#!/usr/bin/bash
[[ "$1" == --version ]] && exit 0
config="$2"
dir="$(basename "$(dirname "${config}")")"
grep -q "^models:" "${config}" || exit 3
printf "[%s] %s\n" "${dir}" "$(cat)"'
install_engine() {
    mkdir -p "${ENGINE%/*}"
    printf '%s\n' "${ENGINE_STANDIN}" >"${ENGINE}"
    chmod +x "${ENGINE}"
}
cat >"${ROOT}/usr/bin/tesseract" <<'EOF'
#!/usr/bin/bash
cat >/dev/null
[[ "${*}" == *"-l xyz"* || "${*}" == *"-l eng"* ]] || exit 4
[[ -e "$(dirname "$0")/empty" ]] || printf 'canary words\non two lines\n\nand a second paragraph\n\f'
EOF
chmod +x "${ROOT}/usr/bin/tesseract"

remote_file() {
    local name="$1" content="$2"
    printf '%s' "${content}" >"${T}/remote/${name}"
    zstd -q -f "${T}/remote/${name}" -o "${T}/remote/${name}.zst"
    jq -nc --arg type "$3" --arg name "${name}" --arg location "x/${name}.zst" \
        --arg sha256 "$(sha256sum "${T}/remote/${name}.zst" | cut -d' ' -f1)" --argjson size "$(stat -c %s "${T}/remote/${name}.zst")" \
        --arg unpacked "$(sha256sum "${T}/remote/${name}" | cut -d' ' -f1)" \
        '{$type, $name, $location, $sha256, $size, unpacked_sha256: $unpacked}'
}
direction() {
    local d="$1"
    jq -nc --argjson a "$(remote_file "model.${d}" "model" model)" --argjson b "$(remote_file "lex.${d}" "lex" lex)" \
        --argjson c "$(remote_file "vocab.${d}" "vocab" vocab)" '{peak_mb: 100, files: [$a, $b, $c]}'
}
printf 'reading' >"${T}/remote/xyz.traineddata"
READING="$(jq -nc --arg sha256 "$(sha256sum "${T}/remote/xyz.traineddata" | cut -d' ' -f1)" \
    '{name: "xyz.traineddata", url: "https://example.org/xyz.traineddata", $sha256, size: 7}')"
IMAGE="registry.fedoraproject.org/fedora:45@sha256:$(printf '5%.0s' {1..64})"
jq -n --arg image "${IMAGE}" --argjson r "${READING}" --argjson a "$(direction xy-en)" --argjson b "$(direction en-xy)" \
    --argjson c "$(direction zz-en)" --argjson d "$(direction en-zz)" \
    '{hosts: ["example.org"], engine: {images: {"45": $image}, tree_sha256: "abc", download_mb: 5, minutes: 1, hosts: ["example.org"]},
      models_url: "https://example.org/",
      languages: {xy: {name: "Xy", reading: $r}, zz: {name: "Zz", reading: $r}},
      directions: {"xy-en": $a, "en-xy": $b, "zz-en": $c, "en-zz": $d}}' >"${ROOT}/usr/share/amethystora/translate/models.json"
export AMETHYSTORA_TRANSLATE_URL="file://${T}/remote"

# --- What it does ---------------------------------------------------------------------------------------------------------
# The translator is built on the machine: without podman it says it cannot, with a stand-in podman it is built in the pinned
# image (pulled by digest, the script mounted read-only, the table's source hash handed in), kept only when it runs, and the
# image taken away again unless it was there before
jq -e '.here == false and .buildable == false' <<<"$(python3 "${HELPER}" engine status)" >/dev/null || fail "buildable without podman"
python3 "${HELPER}" engine build 2>/dev/null && fail "built without podman"
cat >"${ROOT}/usr/bin/podman" <<'PODMAN'
#!/usr/bin/bash
echo "podman $*" >>"${PODMAN_LOG}"
case "$1" in
    image) [[ -n "${PODMAN_HAD:-}" ]]; exit $? ;;
    run)
        [[ -z "${PODMAN_FAIL:-}" ]] || exit 1
        out=""
        for arg in "$@"; do [[ "${arg}" == *:/out ]] && out="${arg%:/out}"; done
        printf '%s\n' "${ENGINE_STANDIN}" >"${out}/amethystora-bergamot"
        chmod +x "${out}/amethystora-bergamot"
        mkdir -p "${out}/licenses"
        ;;
esac
exit 0
PODMAN
chmod +x "${ROOT}/usr/bin/podman"
printf '#!/usr/bin/bash\n' >"${ROOT}/usr/share/amethystora/translate/build-engine.sh"
export PODMAN_LOG="${T}/podman.log" ENGINE_STANDIN
: >"${PODMAN_LOG}"
jq -e '.buildable == true and .here == false and (.hosts | length) > 0' <<<"$(python3 "${HELPER}" engine status)" >/dev/null ||
    fail "not buildable with podman: $(python3 "${HELPER}" engine status)"
PODMAN_FAIL=1 python3 "${HELPER}" engine build 2>/dev/null && fail "a failed build counted"
[[ ! -e "${ENGINE}" && ! -e "${XDG_DATA_HOME}/amethystora/translate/.building" ]] || fail "a failed build kept something"
grep -q "^podman rmi --force ${IMAGE}$" "${PODMAN_LOG}" || fail "the image was left after a failed build: $(cat "${PODMAN_LOG}")"
: >"${PODMAN_LOG}"
python3 "${HELPER}" engine build >/dev/null 2>&1 || fail "the build failed"
[[ -x "${ENGINE}" ]] || fail "no translator after a build"
grep -q "^podman pull --quiet ${IMAGE}$" "${PODMAN_LOG}" || fail "the image was not pulled by its digest"
grep -qE "^podman run --rm --security-opt label=disable -v .*/build-engine.sh:/build.sh:ro -v .*:/out -e EXPECT_TREE=abc -e JOBS=[1-4] ${IMAGE} bash /build.sh$" "${PODMAN_LOG}" ||
    fail "the build's command: $(cat "${PODMAN_LOG}")"
grep -q '^podman rmi' "${PODMAN_LOG}" || fail "the image it pulled was kept"
: >"${PODMAN_LOG}"
PODMAN_HAD=1 python3 "${HELPER}" engine build >/dev/null 2>&1 || fail "a second build failed"
grep -q '^podman rmi' "${PODMAN_LOG}" && fail "an image that was here before was removed"
python3 "${HELPER}" engine remove
[[ ! -e "${ENGINE}" ]] || fail "engine remove left it"
install_engine

[[ "$(python3 "${HELPER}" status zz | jq -c .from)" == '[]' ]] || fail "something is here before a fetch"
jq -e '.error == "no-model"' <<<"$(python3 "${HELPER}" text xy zz </dev/null || true)" >/dev/null || fail "it answered without its models"
python3 "${HELPER}" fetch no-such zz 2>/dev/null && fail "a language not offered was fetched"
python3 "${HELPER}" read xy xy </dev/null 2>/dev/null && fail "a language into itself"
[[ "$(python3 "${HELPER}" size xy zz | jq -r '.bytes')" -gt 0 ]] || fail "size says nothing to fetch"

# A file that is not what was pinned: nothing is kept, not even the part
cp "${T}/remote/model.en-zz.zst" "${T}/good.zst"
echo tampered >"${T}/remote/model.en-zz.zst"
python3 "${HELPER}" fetch xy zz >/dev/null 2>&1 && fail "a tampered model was taken"
[[ -z "$(find "${XDG_DATA_HOME}/amethystora/translate" -name '*en-zz*' 2>/dev/null)" ]] || fail "a tampered model stayed"
cp "${T}/good.zst" "${T}/remote/model.en-zz.zst"

python3 "${HELPER}" fetch xy zz >/dev/null 2>&1 || fail "the fetch failed"
[[ "$(stat -c %a "${XDG_DATA_HOME}/amethystora/translate")" == 700 ]] || fail "the folder is not the account's alone"
[[ "$(python3 "${HELPER}" size xy zz | jq -r '.bytes')" == 0 ]] || fail "a second fetch would download again"
[[ "$(python3 "${HELPER}" status zz | jq -c .from)" == '["en","xy"]' ]] || fail "status: $(python3 "${HELPER}" status zz)"

# Through English, one direction after another, and the lines of a paragraph joined
PNG="${T}/p.png"
base64 -d >"${PNG}" <<<'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAEBgIApD5fRAAAAABJRU5ErkJggg=='
answer="$(python3 "${HELPER}" read xy zz <"${PNG}")"
jq -e '.source == "canary words on two lines\n\nand a second paragraph"' <<<"${answer}" >/dev/null || fail "the reading: ${answer}"
jq -e '.text | startswith("[en-zz] [xy-en] canary")' <<<"${answer}" >/dev/null || fail "the route: ${answer}"
jq -e '.cut == false' <<<"${answer}" >/dev/null || fail "cut"
touch "${ROOT}/usr/bin/empty"
jq -e '.error == "no-text"' <<<"$(python3 "${HELPER}" read xy zz <"${PNG}" || true)" >/dev/null || fail "an empty box"
rm "${ROOT}/usr/bin/empty"

# Too much is cut and says so; a program that fails names no word of the text
python3 - <<'EOF' | python3 "${HELPER}" text xy zz | jq -e '.cut == true and (.source | length) == 4000' >/dev/null || fail "the cut"
print('word ' * 2000)
EOF
printf '#!/usr/bin/bash\nexit 9\n' >"${ENGINE}"
answer="$(echo canary words | python3 "${HELPER}" text xy zz || true)"
jq -e '.error == "unreadable"' <<<"${answer}" >/dev/null || fail "a failing engine: ${answer}"
[[ "${answer}" != *canary* ]] || fail "a failure names the words"

# Nothing of what was read stays anywhere
grep -rl canary "${HOME}" "${XDG_RUNTIME_DIR:-/nonexistent}" 2>/dev/null && fail "the words were written"

python3 "${HELPER}" remove
[[ -x "${ENGINE}" && "$(ls -A "${XDG_DATA_HOME}/amethystora/translate")" == engine ]] || fail "remove left something, or took the translator"
python3 "${HELPER}" remove all
[[ ! -e "${XDG_DATA_HOME}/amethystora/translate" ]] || fail "remove all left something"

# --- What the code may not do ---------------------------------------------------------------------------------------------
# No shell, and the pipes and the fetch are all it opens; the shell's part never blocks or logs what it was given
grep -q 'shell=True' "${HELPER}" && fail "a shell"
for word in tempfile urllib requests socket; do
    grep -qw "${word}" "${HELPER}" && fail "the helper names ${word}"
done
grep -qE '_sync\(|file_get_contents|console\.(log|warn|error)\(.*(answer|source|text)\b' "${PART}" && fail "the shell's part blocks or logs the words"
grep -q 'use_markup = false' "${PART}" || fail "words off a screen could be markup"
echo "test-translate: ok"
