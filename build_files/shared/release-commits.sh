#!/usr/bin/bash
# What's new (R4-15): the notes of this build and of the stream's last few releases, by `just build` on the host, which
# has git, before the container build. Written where the build context takes it (build_files/whats-new.json,
# gitignored): build.sh makes the Manual's page and the pill's file of it, and `just build` puts its releases in the
# image's label org.amethystora.whats-new, which the next build reads back here.
#
#   release-commits.sh IMAGE TAG VERSION > build_files/whats-new.json
#
# The published image of IMAGE:TAG says which commit it was built from (org.opencontainers.image.revision, which
# `just build` sets) or, for one from before, when it was built (org.opencontainers.image.created), and what its notes
# were. The subjects of the commits since go to build-whats-new.py, never an author. Without git, jq, skopeo, python3
# or the registry (offline, a pull request with no published image), it writes nothing and exits 0: the build then has
# no notes. It never fails a build.
set -uo pipefail

image="${1:?image}" tag="${2:?tag}" version="${3:?version}"
org="${REPO_ORGANIZATION:-arsslensoft}"
here="$(dirname "$0")"

for tool in git jq skopeo python3 timeout; do
    command -v "${tool}" >/dev/null || exit 0
done
git rev-parse --verify -q HEAD >/dev/null 2>&1 || exit 0

labels="$(timeout 60 skopeo inspect --retry-times 2 --no-tags "docker://ghcr.io/${org}/${image}:${tag}" 2>/dev/null |
    jq -c '.Labels // {}' 2>/dev/null)" || exit 0
[[ -n "${labels}" ]] || exit 0
from="$(jq -r '.["org.opencontainers.image.version"] // empty' <<<"${labels}")"
revision="$(jq -r '.["org.opencontainers.image.revision"] // empty' <<<"${labels}")"
created="$(jq -r '.["org.opencontainers.image.created"] // empty' <<<"${labels}")"
# The notes the published build carried, a JSON list in a label (build-whats-new.py checks every title again)
history="$(jq -c '(.["org.amethystora.whats-new"] // "[]") | (try fromjson catch []) | if type == "array" then . else [] end' \
    <<<"${labels}" 2>/dev/null)" || history="[]"
[[ -n "${history}" ]] || history="[]"
[[ "${revision}" =~ ^[0-9a-f]{40}$ ]] || revision=""
[[ "${created}" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:]{8}Z$ ]] || created=""

# CI checks out one commit: a few hundred more reach back past any published build
if [[ "$(git rev-parse --is-shallow-repository 2>/dev/null)" == true ]]; then
    timeout 120 git fetch --quiet --deepen=400 origin >/dev/null 2>&1 || true
fi
range=()
if [[ -n "${revision}" ]] && git cat-file -e "${revision}^{commit}" 2>/dev/null; then
    range=("${revision}..HEAD")
elif [[ -n "${created}" ]]; then
    range=("--since=${created}" HEAD)
fi

work="$(mktemp)" || exit 0
trap 'rm -f "${work}"' EXIT
{ [[ ${#range[@]} -eq 0 ]] || git log --no-merges --format='%s' "${range[@]}" 2>/dev/null; } | jq -R . |
    jq -s --arg version "${version}" --arg from "${from}" --argjson history "${history}" \
        '{version: $version, from: (if $from == "" then null else $from end), commits: ., history: $history}' \
        >"${work}" || exit 0
python3 "${here}/build-whats-new.py" notes "${work}" || exit 0
