# The image, on its layers (plans/done/PLAN-build-layers.md), which the Justfile finds or builds first (layers/lib.sh):
# the packages layer of the flavor (packages-dx's for a dx image), the apps and the assets
ARG PACKAGES_IMAGE=""
ARG APPS_IMAGE=""
ARG ASSETS_IMAGE=""
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
# The apps' sources, which 20-tests.sh reads (their golden questions, their messages)
COPY /apps /apps

FROM ${APPS_IMAGE} AS apps

FROM ${ASSETS_IMAGE} AS assets

## amethystora image section
FROM ${PACKAGES_IMAGE} AS base

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

# The build, cleanup and tests, which end with the /opt link (build.sh). The base image's preparation, the kernel
# and the packages are the layers'
RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=bind,from=apps,source=/,target=/apps-stage \
    --mount=type=bind,from=assets,source=/,target=/assets \
    /ctx/build_files/shared/build.sh

CMD ["/sbin/init"]

RUN bootc container lint
