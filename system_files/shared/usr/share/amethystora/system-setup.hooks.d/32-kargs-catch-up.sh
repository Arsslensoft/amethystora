#!/usr/bin/env bash

source /usr/lib/amethystora/setup-services/libsetup.sh

version-script kargs-catch-up system 1 || exit 0

set -x

# amethystora-kargs gives each update rpm-ostree stages the changes of the image's kargs.d, from now on. A machine that
# rpm-ostree updated before (one with layered packages) never got the arguments that were already there when it moved
# onto them: this adds them once, for the next restart, as 27 and 28 did one argument each. An argument whose name
# the command line has with another value is the owner's choice, one turned off with `ame security kernel-args` is
# kept off (amethystora-kargs missing reads hardening-off), and lockdown stays `ame security lockdown`'s. A machine bootc
# updates already has them, and one it lacks is the owner's. A sealed image has them in its signed UKI.
# AMETHYSTORA_KARGS_DRIVER is for 20-tests.sh, which runs this against stand-ins (with the helper's own variables).
DRIVER="${AMETHYSTORA_KARGS_DRIVER:-$(/usr/libexec/amethystora-update-check driver)}"
[[ "${DRIVER}" == rpm-ostree ]] || exit 0
mapfile -t MISSING < <(/usr/libexec/amethystora-kargs missing)
[[ "${#MISSING[@]}" -gt 0 ]] || exit 0
echo "Adding ${MISSING[*]} to the kernel arguments, for the next restart"
rpm-ostree kargs "${MISSING[@]/#/--append-if-missing=}"
