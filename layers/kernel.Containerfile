# Layer 5 (plans/done/PLAN-build-layers.md): the base image as the build starts it (prepare.sh: the upstream packages
# system_files replaces, Fedora's logos and release packages), then the kernel and its modules
# (03-install-kernel-akmods.sh, which runs third-party packages and scripts with the network on and so has no key),
# then the signing of evdi (03-sign-evdi.sh), in a stage of Fedora's own pinned image that holds nothing of the kernel
# stage's filesystem but the module, the only place the module signing key is mounted. One per flavor and day, shared by
# its base and dx images
ARG BASE_IMAGE="ghcr.io/ublue-os/silverblue-main"
ARG FEDORA_MAJOR_VERSION="44"
ARG BASE_IMAGE_SHA=""

FROM scratch AS ctx
COPY /build_files/shared/prepare.sh /build_files/shared/prepare.sh
COPY /build_files/base/03-install-kernel-akmods.sh /build_files/base/03-install-kernel-akmods.sh
COPY /build_files/base/03-sign-evdi.sh /build_files/base/03-sign-evdi.sh
COPY /build_files/shared/copr-helpers.sh /build_files/shared/copr-helpers.sh
COPY /system_files/shared/etc/pki/akmods/certs/amethystora-modules.der /system_files/shared/etc/pki/akmods/certs/amethystora-modules.der
# The image's container policy, which 03's skopeo copies the module images under, as it did when system_files came
# first: the base image's asks ghcr.io/ublue-os for a signature skopeo cannot find, where the Justfile has checked
# each digest with cosign already
COPY /system_files/shared/etc/containers/policy.json /system_files/shared/etc/containers/policy.json
# The Vault's two system users, made before any package's, as they were when system_files came first: the packages'
# own users then get the numbers they have on every machine installed before the layers (dx's dumpcap is 0:941, the
# wireshark group, which one number lower would hand to another group on those machines)
COPY /system_files/shared/usr/lib/sysusers.d/amethystora-vault.conf /system_files/shared/usr/lib/sysusers.d/amethystora-vault.conf
COPY /system_files/shared/usr/lib/sysusers.d/amethystora-vault-net.conf /system_files/shared/usr/lib/sysusers.d/amethystora-vault-net.conf

FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA} AS kernel

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
    install -Dpm0644 -t /usr/lib/sysusers.d /ctx/system_files/shared/usr/lib/sysusers.d/amethystora-vault*.conf && \
    systemd-sysusers && \
    /ctx/build_files/shared/prepare.sh

RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    install -Dpm0644 -t /etc/pki/akmods/certs /ctx/system_files/shared/etc/pki/akmods/certs/amethystora-modules.der && \
    install -Dpm0644 -t /etc/containers /ctx/system_files/shared/etc/containers/policy.json && \
    /ctx/build_files/base/03-install-kernel-akmods.sh

# What signs evdi: Fedora's own image, pinned by digest (the Justfile's fedora_image, and the sealed images' signer),
# with kernel-devel, which carries sign-file, and the tools that check the result, from Fedora's repositories and
# nothing else. Nothing of the kernel stage's filesystem is here but evdi's module as a file, and nothing in this stage
# runs it: the module is data to sign-file. sign-file does not have to match the kernel (it appends a PKCS#7 signature
# the kernel checks)
FROM registry.fedoraproject.org/fedora:45@sha256:fd31d2f9dfd835cf7398af9cb6cbbbffbd33ffac2d5204042ea14157455b63a2 AS signer
RUN dnf -y install --setopt=install_weak_deps=False kernel-devel xz kmod openssl && dnf clean all
ARG KERNEL=""
COPY --from=kernel /usr/lib/modules/${KERNEL}/extra/evdi/evdi.ko.xz /in/evdi.ko.xz
# Three steps, with no network, and the key only in the middle one, as a file that is never copied (the secrets are the
# Justfile's: the release key, or a throwaway one with its own certificate). The steps before and after it are the ones
# that parse the module
RUN --network=none \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /usr/bin/bash /ctx/build_files/base/03-sign-evdi.sh unpack "${KERNEL}"
RUN --network=none \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=secret,id=AKMODS_PRIVKEY \
    --mount=type=secret,id=AKMODS_CERT \
    /usr/bin/bash /ctx/build_files/base/03-sign-evdi.sh sign "${KERNEL}"
RUN --network=none \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /usr/bin/bash /ctx/build_files/base/03-sign-evdi.sh pack "${KERNEL}"

# The kernel layer: the kernel stage, with the signed module put where the unsigned one was
FROM kernel
COPY --from=signer /out/ /
