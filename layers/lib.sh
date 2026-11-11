#!/usr/bin/bash
# shellcheck disable=SC2154,SC2034 # the build recipe's variables, and the references it reads back
# The build's layers (plans/done/PLAN-build-layers.md), sourced by the Justfile's build recipe once it has resolved and
# checked every digest. Each layer is an image of its own, tagged by a key over what it is made of: its
# Containerfile in this folder, every file its COPY lines name, the values given beside it below and the key of the
# layer under it. A layer is used from localhost if it is there, else pulled from GHCR by digest once its signature
# checks out against cosign.pub, else built here. AMETHYSTORA_LAYERS picks what runs:
#   (unset)  every layer the image needs, found or built, then the image
#   fprintd  fprintd, then stop (CI's first layers job)
#   shared   the crates, the apps and the assets, then stop (CI's shared layers job)
#   flavor   fprintd, the kernel and both packages layers of the flavor, then stop (CI's flavor jobs)
#   fetch    find the image's layers and build none, then stop (CI's image jobs, with a login to GHCR)
# Every layer built is added to /tmp/amethystora-layers-built, "NAME TAG" a line, for CI to push and sign.
#
# Reads the build recipe's variables: image, tag, flavor, fedora_version, base_image_name, base_image_sha,
# akmods_flavor, akmods_sha, akmods_zfs_sha, akmods_nvidia_sha, kernel_release, LAYER_REGISTRY, REPO_ORGANIZATION,
# PODMAN, and the module key's KERNEL_LAYER_ARGS and MODULE_KEY (release or throwaway).

LAYERS_BUILT=/tmp/amethystora-layers-built
# The layers this build used ("NAME TAG"), whose older tags are untagged once the image is built
LAYERS_USED=()
# Set by each layers_* function: the local reference of the layer it found or built
LAYER_REF=""

layer_log() {
    echo "layers: $*" >&2
}

# The files a layer's Containerfile copies from the repository (its COPY lines without --from), one per line
layer_files() {
    awk '$1 == "COPY" && $2 !~ /^--from/ { for (i = 2; i < NF; i++) { sub(/^\//, "", $i); print $i } }' "$1"
}

# layer_key FILE VALUE... - the first 16 hex digits of a sha256 over the Containerfile and the files it copies
# (names, modes and contents) and the values, which include the key of the layer under it
layer_key() {
    local file="$1" files tree
    shift
    readarray -t files < <(layer_files "${file}")
    # Every function here fails by its status, never by set -e, which is off in a function called from a list
    # What .containerignore keeps out of the build is kept out of the key (a local cargo build of apps/)
    tree="$(tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --mode='go-w' \
        --exclude-from=<(grep -v '^#' .containerignore) -cf - "${file}" "${files[@]}" |
        sha256sum)" || return 1
    printf '%s\n' "${tree}" "$@" | sha256sum | cut -c1-16
}

# layer_find NAME TAG - is localhost/NAME:TAG here, or on GHCR with Amethystora's signature (then pulled and tagged).
# The signature names the layer (the annotation org.amethystora.layer=NAME:TAG, which push-layers signs with): a digest
# Amethystora signed for another layer or tag, or one signed before the annotation, is not this layer's, so that a
# tag someone points at an older signed layer is built here instead
layer_find() {
    local name="$1" tag="$2" digest
    if ${PODMAN} image exists "localhost/${name}:${tag}"; then
        layer_log "${name}:${tag} is here"
        return 0
    fi
    digest="$(skopeo inspect --retry-times 2 --no-tags "docker://${LAYER_REGISTRY}/${name}:${tag}" 2>/dev/null | jq -r .Digest)" || true
    if [[ "${digest}" != sha256:* ]]; then
        layer_log "${name}:${tag} is not on ${LAYER_REGISTRY}"
        return 1
    fi
    if ! cosign verify --key cosign.pub -a "org.amethystora.layer=${name}:${tag}" "${LAYER_REGISTRY}/${name}@${digest}" >/dev/null 2>&1; then
        echo "::warning::${LAYER_REGISTRY}/${name}:${tag} does not carry Amethystora's signature for that layer, so it is built here" >&2
        return 1
    fi
    ${PODMAN} pull --retry 3 "${LAYER_REGISTRY}/${name}@${digest}" >&2 || return 1
    ${PODMAN} tag "${LAYER_REGISTRY}/${name}@${digest}" "localhost/${name}:${tag}" || return 1
    layer_log "${name}:${tag} pulled from ${LAYER_REGISTRY} (${digest})"
}

# layer_build NAME TAG CONTAINERFILE TARGET [podman build options...] - build a layer, and list it as built
layer_build() {
    local name="$1" tag="$2" file="$3" target="$4" start="${SECONDS}"
    shift 4
    if [[ "${AMETHYSTORA_LAYERS:-}" == fetch ]]; then
        layer_log "${name}:${tag} is not published: the image's build makes it"
        return 1
    fi
    local options=(--file "${file}" --tag "localhost/${name}:${tag}"
        --build-arg "BASE_IMAGE=ghcr.io/ublue-os/${base_image_name}-main"
        --build-arg "FEDORA_MAJOR_VERSION=${fedora_version}"
        --build-arg "BASE_IMAGE_SHA=${base_image_sha}"
        --label "org.opencontainers.image.source=https://github.com/${REPO_ORGANIZATION}/amethystora"
        --label "org.opencontainers.image.description=A build layer of Amethystora, not an image to run"
        --label "org.amethystora.layer=${name}:${tag}")
    [[ -z "${target}" ]] || options+=(--target "${target}")
    layer_log "building ${name}:${tag}"
    ${PODMAN} build "${options[@]}" "$@" . >&2 || { echo "::error::layer ${name}:${tag} did not build" >&2; return 1; }
    echo "${name} ${tag}" >>"${LAYERS_BUILT}"
    echo "::notice::layer ${name}:${tag} built in $((SECONDS - start)) s" >&2
}

layer_use() {
    LAYERS_USED+=("$1 $2")
    LAYER_REF="localhost/$1:$2"
}

# --- The shared layers, built once for all six images ----------------------------------------------------------

FPRINTD_REF=""
layers_fprintd() {
    local tag
    tag="${fedora_version}-$(layer_key layers/fprintd.Containerfile "base=${base_image_sha}")" || return 1
    layer_find amethystora-fprintd "${tag}" ||
        layer_build amethystora-fprintd "${tag}" layers/fprintd.Containerfile "" --layers=false || return 1
    layer_use amethystora-fprintd "${tag}"
    FPRINTD_REF="${LAYER_REF}"
}

APPS_REF=""
layers_apps() {
    local tag cargo_tag cargo_image=scratch
    tag="${fedora_version}-$(layer_key layers/apps.Containerfile "base=${base_image_sha}")" || return 1
    if ! layer_find amethystora-apps "${tag}"; then
        [[ "${AMETHYSTORA_LAYERS:-}" == fetch ]] && return 1
        # The crates, compiled: they change with the lock and the manifests, and Fedora's Rust moves on (a new
        # compiler rebuilds everything), so the cargo layer starts again each ISO week
        cargo_tag="${fedora_version}-$(
            { find apps -name Cargo.toml -not -path '*/target*' -print0 | sort -z | xargs -0 cat; cat apps/Cargo.lock; } |
                sha256sum | cut -c1-16
        )-$(date -u +%GW%V)" || return 1
        if layer_find amethystora-cargo "${cargo_tag}"; then
            cargo_image="localhost/amethystora-cargo:${cargo_tag}"
            layer_use amethystora-cargo "${cargo_tag}"
        fi
        # With the layer cache, so that the cargo target below takes the same build stage instead of a second one
        layer_build amethystora-apps "${tag}" layers/apps.Containerfile apps --build-arg "CARGO_IMAGE=${cargo_image}" ||
            return 1
        if [[ "${cargo_image}" == scratch ]]; then
            layer_build amethystora-cargo "${cargo_tag}" layers/apps.Containerfile cargo-out \
                --build-arg "CARGO_IMAGE=${cargo_image}" || return 1
            layer_use amethystora-cargo "${cargo_tag}"
        fi
        # The build stage, which only the cache held
        ${PODMAN} image prune -f >&2
    fi
    layer_use amethystora-apps "${tag}"
    APPS_REF="${LAYER_REF}"
}

ASSETS_REF=""
layers_assets() {
    local tag
    tag="${fedora_version}-$(layer_key layers/assets.Containerfile "base=${base_image_sha}")" || return 1
    layer_find amethystora-assets "${tag}" ||
        layer_build amethystora-assets "${tag}" layers/assets.Containerfile "" --layers=false || return 1
    layer_use amethystora-assets "${tag}"
    ASSETS_REF="${LAYER_REF}"
}

# --- The flavor's layers, shared by its base and dx images -----------------------------------------------------

# The packages layers install from repositories, so their key carries an epoch: the UTC date on latest and beta,
# the workflow run on stable (every release installs the packages of its moment), or AMETHYSTORA_PACKAGES_EPOCH
layers_epoch() {
    if [[ -n "${AMETHYSTORA_PACKAGES_EPOCH:-}" ]]; then
        echo "${AMETHYSTORA_PACKAGES_EPOCH}"
    elif [[ "${tag}" =~ stable && -n "${GITHUB_RUN_ID:-}" ]]; then
        echo "run${GITHUB_RUN_ID}"
    else
        date -u +%Y%m%d
    fi
}

# The tags of the kernel, packages and packages-dx layers built with the module key $1 (release or throwaway)
layers_tags() {
    local kind="$1" kernel_key packages_key prefix="${flavor}-${tag}-${fedora_version}"
    kernel_key="$(layer_key layers/kernel.Containerfile "base=${base_image_sha}" "akmods=${akmods_sha}" \
        "zfs=${akmods_zfs_sha}" "nvidia=${akmods_nvidia_sha}" "akmods-flavor=${akmods_flavor}" "kernel=${kernel_release}" \
        "stream=${tag}" "image=${FLAVOR_NAME}" "module-key=${kind}" "epoch=${EPOCH}")" || return 1
    packages_key="$(layer_key layers/packages.Containerfile "kernel=${kernel_key}" "fprintd=${FPRINTD_REF##*:}")" || return 1
    KERNEL_TAG="${prefix}-${kernel_key}"
    PACKAGES_TAG="${prefix}-${packages_key}"
    PACKAGES_DX_TAG="${prefix}-$(layer_key layers/packages-dx.Containerfile "packages=${packages_key}")" || return 1
}

# The packages layer for a base image, packages-dx for a dx one, and both for the flavor job. A build without the
# release module key takes the release key's layers first: they are as good for a build that publishes nothing
PACKAGES_REF=""
layers_packages() {
    local want=packages
    [[ "${image}" =~ dx ]] && want=packages-dx
    FLAVOR_NAME=amethystora
    [[ "${flavor}" == main ]] || FLAVOR_NAME="amethystora-${flavor}"
    EPOCH="$(layers_epoch)"
    if [[ "${MODULE_KEY}" != release && "${AMETHYSTORA_LAYERS:-}" != flavor ]]; then
        layers_tags release || return 1
        local release_tag="${PACKAGES_TAG}"
        [[ "${want}" == packages-dx ]] && release_tag="${PACKAGES_DX_TAG}"
        if layer_find "amethystora-${want}" "${release_tag}"; then
            layer_use "amethystora-${want}" "${release_tag}"
            PACKAGES_REF="${LAYER_REF}"
            return
        fi
    fi
    layers_tags "${MODULE_KEY}" || return 1
    if ! layer_find amethystora-packages "${PACKAGES_TAG}"; then
        if ! layer_find amethystora-kernel "${KERNEL_TAG}"; then
            layer_build amethystora-kernel "${KERNEL_TAG}" layers/kernel.Containerfile "" --layers=false \
                "${KERNEL_LAYER_ARGS[@]}" --build-arg "IMAGE_NAME=${FLAVOR_NAME}" || return 1
        fi
        layer_use amethystora-kernel "${KERNEL_TAG}"
        layer_build amethystora-packages "${PACKAGES_TAG}" layers/packages.Containerfile "" --layers=false \
            --build-arg "KERNEL_IMAGE=localhost/amethystora-kernel:${KERNEL_TAG}" --build-arg "FPRINTD_IMAGE=${FPRINTD_REF}" \
            --build-arg "IMAGE_NAME=${FLAVOR_NAME}" --build-arg "UBLUE_IMAGE_TAG=${tag}" || return 1
    fi
    layer_use amethystora-packages "${PACKAGES_TAG}"
    if [[ "${want}" == packages-dx || "${AMETHYSTORA_LAYERS:-}" == flavor ]]; then
        if ! layer_find amethystora-packages-dx "${PACKAGES_DX_TAG}"; then
            layer_build amethystora-packages-dx "${PACKAGES_DX_TAG}" layers/packages-dx.Containerfile "" --layers=false \
                --build-arg "PACKAGES_IMAGE=localhost/amethystora-packages:${PACKAGES_TAG}" \
                --build-arg "IMAGE_NAME=${FLAVOR_NAME}" --build-arg "UBLUE_IMAGE_TAG=${tag}" || return 1
        fi
        [[ "${want}" == packages-dx ]] && layer_use amethystora-packages-dx "${PACKAGES_DX_TAG}"
    fi
    PACKAGES_REF="${LAYER_REF}"
}

# Every layer AMETHYSTORA_LAYERS asks for. Sets FPRINTD_REF, APPS_REF, ASSETS_REF and PACKAGES_REF
layers_run() {
    : >>"${LAYERS_BUILT}"
    case "${AMETHYSTORA_LAYERS:-}" in
        fprintd)
            layers_fprintd || return 1
            ;;
        shared)
            layers_apps || return 1
            layers_assets || return 1
            ;;
        flavor)
            layers_fprintd || return 1
            layers_packages || return 1
            ;;
        fetch)
            # Whatever is published is pulled; the image's build makes the rest
            layers_fprintd || true
            layers_apps || true
            layers_assets || true
            [[ -z "${FPRINTD_REF}" ]] || layers_packages || true
            ;;
        "")
            layers_fprintd || return 1
            layers_apps || return 1
            layers_assets || return 1
            layers_packages || return 1
            ;;
        *)
            echo "AMETHYSTORA_LAYERS is fprintd, shared, flavor, fetch or empty, not ${AMETHYSTORA_LAYERS}" >&2
            return 1
            ;;
    esac
}

# Untag the older tags of the layers this build used (the same layer, flavor, stream and Fedora), so that a prune
# reclaims them
layers_untag_old() {
    local entry name used prefix old
    for entry in "${LAYERS_USED[@]}"; do
        read -r name used <<<"${entry}"
        prefix="${used%-*}"
        [[ "${name}" == amethystora-cargo ]] && prefix="${fedora_version}"
        while read -r old; do
            [[ "${old}" == "${used}" || "${old}" != "${prefix}-"* ]] && continue
            ${PODMAN} untag "localhost/${name}:${old}" >&2 || true
        done < <(${PODMAN} images --format '{{.Tag}}' "localhost/${name}" 2>/dev/null)
    done
}
