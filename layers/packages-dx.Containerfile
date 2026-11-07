# Layer 7 (plans/done/PLAN-build-layers.md), on the packages layer: dx's packages (00-dx.sh), their source packages and
# debranding's pass again (layers/packages-dx.sh). One per flavor and day, for its dx image
ARG PACKAGES_IMAGE=""

FROM scratch AS ctx
COPY /layers/packages-dx.sh /layers/packages-dx.sh
COPY /build_files/dx/00-dx.sh /build_files/dx/00-dx.sh
COPY /build_files/shared/copr-helpers.sh /build_files/shared/copr-helpers.sh
COPY /build_files/shared/debrand.py /build_files/shared/debrand.py
COPY /build_files/shared/source-manifest.sh /build_files/shared/source-manifest.sh
COPY /system_files/dx/etc/pki/rpm-gpg/RPM-GPG-KEY-VSCodium /system_files/dx/etc/pki/rpm-gpg/RPM-GPG-KEY-VSCodium

FROM ${PACKAGES_IMAGE}

ARG FEDORA_MAJOR_VERSION="44"
ARG IMAGE_NAME="amethystora"
ARG UBLUE_IMAGE_TAG="stable"

RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=cache,dst=/var/cache/rpm-ostree \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /ctx/layers/packages-dx.sh
