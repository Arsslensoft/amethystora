# Layers 2 and 3 (plans/PLAN-build-layers.md): the Amethystora apps (apps/), built and tested on the base they run
# on. The default target is the apps layer: /out (the programs, the agent's model, the crates' licences) and the
# Node that 20-tests.sh runs its checks written in JavaScript with, which the image mounts at /apps-stage. The
# cargo target is the crates compiled from crates.io, which the next build of a changed apps/ starts from
ARG BASE_IMAGE="ghcr.io/ublue-os/silverblue-main"
ARG FEDORA_MAJOR_VERSION="44"
ARG BASE_IMAGE_SHA=""
# The cargo layer of the same key, or nothing
ARG CARGO_IMAGE="scratch"

FROM scratch AS ctx
COPY /apps /apps
COPY /build_files/shared/build-apps.sh /build_files/shared/build-apps.sh
# What the tests compare the apps with (AMETHYSTORA_ROOT): the palette's checks and the themes (Control's contrast),
# the watcher's list of credential files and vault.just (the Vault's)
COPY /system_files/shared/usr/lib/amethystora/theme/palette.awk /system_files/shared/usr/lib/amethystora/theme/palette.awk
COPY /system_files/shared/usr/share/amethystora/themes /system_files/shared/usr/share/amethystora/themes
COPY /system_files/shared/usr/libexec/amethystora-audit-home-rules /system_files/shared/usr/libexec/amethystora-audit-home-rules
COPY /system_files/shared/usr/share/amethystora/just/vault.just /system_files/shared/usr/share/amethystora/just/vault.just

FROM ${CARGO_IMAGE} AS cargo

FROM ${BASE_IMAGE}:${FEDORA_MAJOR_VERSION}@${BASE_IMAGE_SHA} AS build
RUN --mount=type=cache,dst=/var/cache/libdnf5 \
    --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=bind,from=cargo,source=/,target=/cargo-cache \
    CARGO_SAVE=/cargo /ctx/build_files/shared/build-apps.sh

FROM scratch AS cargo-out
COPY --from=build /cargo /cargo

FROM scratch AS apps
COPY --from=build /stage/ /
