#!/usr/bin/bash
# shellcheck disable=SC2016 # the ${...} in the patterns below is the text of the scripts it reads, not ours
# The release chain's static invariants (audit 2026-11-10, area ci), from the checkout, for the Checks workflow's Lint
# job: the module signing key reaches only the signing step, a layer's signature names the layer, a build that publishes
# nothing holds a read-only token, and what's new is read from a digest cosign checked. Run it from the repository root.

set -euo pipefail

fail() {
    echo "test-ci-chain: $*" >&2
    exit 1
}

kernel=layers/kernel.Containerfile
# The key is mounted in one RUN, which has no network, runs only the signing step, and is in a stage of Fedora's own
# image pinned by digest that takes nothing from the kernel stage but evdi's module as a file: nothing of the third-party
# filesystem may execute while the key is mounted
[[ "$(grep -c 'AKMODS_PRIVKEY' "${kernel}")" == 1 ]] || fail "${kernel} must mention AKMODS_PRIVKEY exactly once"
run="$(awk '/^RUN /{blk=""; inrun=1} inrun{blk=blk $0 "\n"; if ($0 !~ /\\$/) {inrun=0; if (blk ~ /AKMODS_PRIVKEY/) printf "%s", blk}}' "${kernel}")"
grep -q -- '--network=none' <<<"${run}" || fail "the RUN that mounts AKMODS_PRIVKEY must have --network=none"
grep -q '03-sign-evdi.sh sign ' <<<"${run}" || fail "the RUN that mounts AKMODS_PRIVKEY must run only the signing step of 03-sign-evdi.sh"
! grep -q 'from=kernel' <<<"${run}" || fail "the RUN that mounts AKMODS_PRIVKEY must not mount the kernel stage"
stage="$(awk '/^FROM /{from=$0} /AKMODS_PRIVKEY/{print from}' "${kernel}")"
grep -qE '^FROM registry\.fedoraproject\.org/fedora:[0-9]+@sha256:[0-9a-f]{64} AS signer$' <<<"${stage}" ||
    fail "the stage that mounts AKMODS_PRIVKEY must be Fedora's own image pinned by digest, as the sealed signer is"
[[ "$(grep -c -- '--from=kernel' "${kernel}")" == 1 ]] || fail "the signer stage may take one thing from the kernel stage"
grep -q 'COPY --from=kernel .*/evdi.ko.xz ' "${kernel}" || fail "the one thing the signer stage takes from the kernel stage is evdi's module"
! grep -nE 'AKMODS_PRIVKEY|private_key|/run/secrets' build_files/base/03-install-kernel-akmods.sh ||
    fail "03-install-kernel-akmods.sh runs third-party code with the network on and must not see the key"
grep -q '03-sign-evdi.sh' "${kernel}" || fail "${kernel} does not copy 03-sign-evdi.sh"

# A layer's signature carries its name and tag, and is verified with them
grep -q 'sign .*-a "org.amethystora.layer=\${name}:\${tag}"' .github/actions/push-layers/action.yml ||
    fail "push-layers does not sign with the layer's annotation"
grep -q 'verify .*-a "org.amethystora.layer=\${name}:\${tag}"' layers/lib.sh ||
    fail "layer_find does not verify the layer's annotation"

# What's new comes from a digest cosign accepted
grep -q 'cosign verify --key' build_files/shared/release-commits.sh ||
    fail "release-commits.sh does not verify the image it reads labels from"

# A run that publishes nothing: no write token anywhere in what it can reach. The reusable workflow asks for none (a
# called workflow may only lower its caller's), each caller has the two jobs, and only the publishing one writes
! grep -nE '^\s*(packages|id-token|attestations|artifact-metadata): write' .github/workflows/reusable-build.yml ||
    fail "reusable-build.yml must not ask for write permissions: its callers grant them"
for f in latest-main stable beta; do
    w=.github/workflows/build-image-${f}.yml
    grep -q 'unpublished:' "${w}" || fail "${w} has no -unpublished job"
    awk '/-unpublished:/{u=1} u && /packages: write/{bad=1} /^  [a-z-]+:$/ && !/-unpublished:/{u=0} END{exit bad}' "${w}" ||
        fail "the unpublished job of ${w} must not have write permissions"
done

echo "test-ci-chain: ok"
