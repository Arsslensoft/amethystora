# Layer 5 (plans/PLAN-build-layers.md): the base image as the build starts it (prepare.sh: the upstream packages
# system_files replaces, Fedora's logos and release packages), then the kernel and its modules
# (03-install-kernel-akmods.sh), the only step the module signing key is mounted for. One per flavor and day,
# shared by its base and dx images
ARG BASE_IMAGE="ghcr.io/ublue-os/silverblue-main"
ARG FEDORA_MAJOR_VERSION="44"
ARG BASE_IMAGE_SHA=""

FROM scratch AS ctx
COPY /build_files/shared/prepare.sh /build_files/shared/prepare.sh
COPY /build_files/base/03-install-kernel-akmods.sh /build_files/base/03-install-kernel-akmods.sh
COPY /build_files/shared/copr-helpers.sh /build_files/shared/copr-helpers.sh
COPY /system_files/shared/etc/pki/akmods/certs/amethystora-modules.der /system_files/shared/etc/pki/akmods/certs/amethystora-modules.der
# The image's container policy, which 03's skopeo copies the module images under, as it did when system_files came
# first: the base image's asks ghcr.io/ublue-os for a signature skopeo cannot find, where the Justfile has checked
# each digest with cosign already
COPY /system_files/shared/etc/containers/policy.json /system_files/shared/etc/containers/policy.json

FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA}

ARG AKMODS_FLAVOR="coreos-stable"
ARG AKMODS_DIGEST=""
ARG AKMODS_ZFS_DIGEST=""
ARG AKMODS_NVIDIA_DIGEST=""
ARG BASE_IMAGE_NAME="silverblue"
ARG FEDORA_MAJOR_VERSION="44"
ARG IMAGE_NAME="amethystora"
ARG KERNEL=""
ARG UBLUE_IMAGE_TAG="stable"

RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /ctx/build_files/shared/prepare.sh

RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=secret,id=AKMODS_PRIVKEY \
    --mount=type=secret,id=AKMODS_CERT \
    install -Dpm0644 -t /etc/pki/akmods/certs /ctx/system_files/shared/etc/pki/akmods/certs/amethystora-modules.der && \
    install -Dpm0644 -t /etc/containers /ctx/system_files/shared/etc/containers/policy.json && \
    /ctx/build_files/base/03-install-kernel-akmods.sh
