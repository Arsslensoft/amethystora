#!/usr/bin/bash
#
# The index `ame desktop missing-command` answers from (/usr/libexec/amethystora-missing-command): every /usr/bin and
# /usr/sbin name in the packages of the image's own Fedora release, and the package it comes in, one "name<TAB>package"
# a line, sorted. It is what a container made from the fedora template can install; read on the machine, never fetched
# there. Fedora's repositories only, as 04-packages.sh installs from them (and updates-testing on beta).

echo "::group:: ===$(basename "$0")==="

set -euo pipefail

OUT=/usr/share/amethystora/missing-command/fedora.tsv
REPOS=(--repo=fedora --repo=updates)
if [[ "${UBLUE_IMAGE_TAG:-}" == "beta" ]]; then
    REPOS+=(--repo=updates-testing)
fi

mkdir -p "$(dirname "${OUT}")"
# A line "@@NAME" before each package's files; the files of /usr/bin and /usr/sbin kept, by their base name. A mirror
# that stalls makes dnf give up with a short list, so it is asked up to three times, and says why when it gave up.
for attempt in 1 2 3; do
    dnf5 -q repoquery "${REPOS[@]}" --latest-limit=1 --arch="$(uname -m),noarch" --qf '@@%{name}\n%{files}\n' 2>/tmp/repoquery.err |
        awk '/^@@/ { package = substr($0, 3); next }
            /^\/usr\/s?bin\/[^\/]+$/ { name = $0; sub(/.*\//, "", name); if (name ~ /^[A-Za-z0-9_+][A-Za-z0-9._+-]*$/) print name "\t" package }' |
        LC_ALL=C sort -u -t "$(printf '\t')" -k1,1 >"${OUT}.new" || true
    # Fedora's release has tens of thousands of commands: far fewer means the query did not work
    [[ "$(wc -l <"${OUT}.new")" -gt 10000 ]] && break
    echo "repoquery attempt ${attempt} listed $(wc -l <"${OUT}.new") commands:" >&2
    tail -n 5 /tmp/repoquery.err >&2
    ((attempt < 3)) || exit 1
    sleep 30
done
rm -f /tmp/repoquery.err
mv "${OUT}.new" "${OUT}"
chmod 0644 "${OUT}"
echo "$(wc -l <"${OUT}") commands in ${OUT}"

echo "::endgroup::"
