# Layer 1 (plans/done/PLAN-build-layers.md): fprintd with the duress finger's patches (packaging/fprintd), built on the
# base it is installed into. Only the three packages in /out leave it (05-override-install.sh installs them)
ARG BASE_IMAGE="ghcr.io/ublue-os/silverblue-main"
ARG FEDORA_MAJOR_VERSION="44"
ARG BASE_IMAGE_SHA=""

FROM scratch AS ctx
COPY /packaging/fprintd /packaging/fprintd
COPY /build_files/shared/build-fprintd.sh /build_files/shared/build-fprintd.sh

FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA} AS build
RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    /ctx/build_files/shared/build-fprintd.sh

FROM scratch
COPY --from=build /out /out
