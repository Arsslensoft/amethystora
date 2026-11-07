# Layer 4 (plans/done/PLAN-build-layers.md): what is made only from pinned downloads, each checked by its SHA-256: Sweet
# recoloured (11-gtk-theme.sh), the themes' fonts from Google Fonts (09-fonts.sh) and the emoji picker's lists
# (09-emoji.sh). Each script's make half writes under /out here; its install half copies it into the image
ARG BASE_IMAGE="ghcr.io/ublue-os/silverblue-main"
ARG FEDORA_MAJOR_VERSION="44"
ARG BASE_IMAGE_SHA=""

FROM scratch AS ctx
COPY /build_files/base/09-emoji.sh /build_files/base/09-emoji.sh
COPY /build_files/base/09-fonts.sh /build_files/base/09-fonts.sh
COPY /build_files/base/11-gtk-theme.sh /build_files/base/11-gtk-theme.sh
COPY /build_files/shared/emoji-data.py /build_files/shared/emoji-data.py
COPY /build_files/shared/recolor.py /build_files/shared/recolor.py
COPY /build_files/shared/utils/ghcurl /build_files/shared/utils/ghcurl

FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA} AS build
RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    install -Dm0755 /ctx/build_files/shared/utils/ghcurl /tmp/scripts/helpers/ghcurl && \
    export PATH="/tmp/scripts/helpers:${PATH}" ASSETS=/out && \
    dnf5 -y install --repo=fedora --repo=updates ImageMagick && \
    /ctx/build_files/base/09-fonts.sh make && \
    /ctx/build_files/base/09-emoji.sh make && \
    /ctx/build_files/base/11-gtk-theme.sh make

FROM scratch
COPY --from=build /out /out
