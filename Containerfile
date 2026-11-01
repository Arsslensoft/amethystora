ARG BASE_IMAGE_NAME="silverblue"
ARG FEDORA_MAJOR_VERSION="42"
ARG SOURCE_IMAGE="${BASE_IMAGE_NAME}-main"
ARG BASE_IMAGE="ghcr.io/ublue-os/${SOURCE_IMAGE}"
ARG BASE_IMAGE_SHA=""
ARG BREW_IMAGE="ghcr.io/ublue-os/brew:latest"
ARG BREW_IMAGE_SHA=""

FROM ${BREW_IMAGE}@${BREW_IMAGE_SHA} AS brew

FROM scratch AS ctx
COPY /build_files /build_files
COPY --from=brew /system_files /system_files/shared
# amethystora-owned files overlay last so they take precedence over brew
COPY /system_files /system_files
# Licence and upstream attribution ship in the image (Apache-2.0)
COPY LICENSE NOTICE /system_files/shared/usr/share/licenses/amethystora/
# Public half of the key CI signs the images with: updates are only accepted with a valid signature
COPY cosign.pub /system_files/shared/etc/pki/containers/amethystora.pub
# The wordmark's font, for the windows that draw it (13-apps.sh)
COPY /branding/fonts /branding/fonts
# The shell programs' translations, compiled into /usr/share/locale (14-security.sh)
COPY /po /po
# The packages Amethystora rebuilds, built in their own stage below
COPY /packaging /packaging
# The Amethystora apps, in Rust, built in their own stage below
COPY /apps /apps

## fprintd with the duress finger's patches, built on the base it is installed into. Its build tools stay
## in this stage: only the packages in /out reach the image (05-override-install.sh)
FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA} AS fprintd
# What the stage builds from, copied in only so that a change to it builds the stage again: the layer cache
# does not look at what a bind mount holds, and took a stage built from older sources
COPY --from=ctx /packaging/fprintd /usr/src/amethystora-build/fprintd
COPY --from=ctx /build_files/shared/build-fprintd.sh /usr/src/amethystora-build/
RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /ctx/build_files/shared/build-fprintd.sh

## The Amethystora apps (apps/), built on the base they run on. Rust and the crates stay in this stage:
## only the programs, the agent's model and the crates' licences in /out reach the image (13-apps.sh). Its Node runs the
## checks written in JavaScript (20-tests.sh), so that the image needs none of its own
FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA} AS apps
# As fprintd's: in the lab the cache took an apps stage built before a crate was added
COPY --from=ctx /apps /usr/src/amethystora-build/apps
COPY --from=ctx /build_files/shared/build-apps.sh /usr/src/amethystora-build/
RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /ctx/build_files/shared/build-apps.sh

## amethystora image section
FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA} AS base

ARG AKMODS_FLAVOR="coreos-stable"
# The kernel and module images, by the digests the Justfile checked
ARG AKMODS_DIGEST=""
ARG AKMODS_ZFS_DIGEST=""
ARG AKMODS_NVIDIA_DIGEST=""
# When the base image was built, in seconds since the epoch (00-image-info.sh)
ARG BASE_IMAGE_CREATED=""
ARG BASE_IMAGE_NAME="silverblue"
ARG FEDORA_MAJOR_VERSION="40"
ARG IMAGE_NAME="amethystora"
ARG IMAGE_VENDOR="arsslensoft"
ARG KERNEL="6.10.10-200.fc40.x86_64"
ARG SHA_HEAD_SHORT="dedbeef"
ARG UBLUE_IMAGE_TAG="stable"
ARG VERSION=""
ARG IMAGE_FLAVOR=""

# Build, cleanup, lint. In three steps, so that the kernel module signing key is mounted only for the one
# that builds and signs the modules, before any of the build's own third-party repositories is added
RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /ctx/build_files/shared/prepare.sh

RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=secret,id=AKMODS_PRIVKEY \
    --mount=type=secret,id=AKMODS_CERT \
    /ctx/build_files/base/03-install-kernel-akmods.sh

RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=bind,from=fprintd,source=/out,target=/fprintd \
    --mount=type=bind,from=apps,source=/,target=/apps-stage \
    /ctx/build_files/shared/build.sh

# Makes `/opt` writeable by default
# Needs to be here to make the main image build strict (no /opt there)
# This is for downstream images/stuff like k0s
RUN rm -rf /opt && ln -s /var/opt /opt

CMD ["/sbin/init"]

RUN bootc container lint
