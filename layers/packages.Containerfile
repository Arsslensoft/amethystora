# Layer 6 (plans/done/PLAN-build-layers.md), on the kernel layer: the packages (04-packages.sh, 05-override-install.sh
# with fprintd from layer 1), the image's build tools, the source packages and debranding's full pass
# (layers/packages.sh). One per flavor and day, shared by its base and dx images
ARG KERNEL_IMAGE=""
ARG FPRINTD_IMAGE=""

FROM scratch AS ctx
COPY /layers/packages.sh /layers/packages.sh
COPY /build_files/base/04-packages.sh /build_files/base/04-packages.sh
COPY /build_files/base/05-override-install.sh /build_files/base/05-override-install.sh
COPY /build_files/shared/build-missing-command-index.sh /build_files/shared/build-missing-command-index.sh
COPY /build_files/shared/copr-helpers.sh /build_files/shared/copr-helpers.sh
COPY /build_files/shared/debrand.py /build_files/shared/debrand.py
COPY /build_files/shared/source-manifest.sh /build_files/shared/source-manifest.sh
COPY /build_files/shared/utils/ghcurl /build_files/shared/utils/ghcurl

FROM ${FPRINTD_IMAGE} AS fprintd

FROM ${KERNEL_IMAGE}

ARG FEDORA_MAJOR_VERSION="44"
ARG IMAGE_NAME="amethystora"
ARG UBLUE_IMAGE_TAG="stable"

RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=bind,from=fprintd,source=/out,target=/fprintd \
    /ctx/layers/packages.sh
