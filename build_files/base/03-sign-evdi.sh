#!/usr/bin/bash
# Signs the evdi module 03-install-kernel-akmods.sh built, the only use of the kernel module signing key. It runs in the
# `signer` stage of layers/kernel.Containerfile: Fedora's own image, pinned by digest, which holds nothing of the kernel
# layer's filesystem but evdi's module as a file (/in/evdi.ko.xz). Three steps, each its own RUN with no network, and
# only the middle one with the key:
#   unpack  xz decompresses the module (a parser, run without the key)
#   sign    sign-file appends the signature, which reads the module's bytes and parses nothing, with the key read from
#           the secret mount and never copied
#   pack    xz compresses it as the kernel's own build does, and modinfo checks the signature names the certificate
#           (parsers again, run without the key)
# The release key's certificate is the one the image ships; a build without that key (a pull request, a local build)
# gets a throwaway key from the Justfile with a certificate of its own, so that the signing is still tested, and evdi is
# then rejected by any machine with Secure Boot on. The result is /out/usr/lib/modules/KERNEL/extra/evdi/evdi.ko.xz,
# which the kernel layer copies over the unsigned module.

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

STEP="${1:?step}"
KERNEL="${2:?kernel release}"
WORK=/work
MARK="~Module signature appended~"
CERT=/ctx/system_files/shared/etc/pki/akmods/certs/amethystora-modules.der
if [[ -s /run/secrets/AKMODS_CERT ]]; then
    CERT=/run/secrets/AKMODS_CERT
fi
OUT="/out/usr/lib/modules/${KERNEL}/extra/evdi"

case "${STEP}" in
unpack)
    mkdir -p "${WORK}"
    xz -dc /in/evdi.ko.xz >"${WORK}/evdi.ko"
    ;;
sign)
    if [[ ! -s /run/secrets/AKMODS_PRIVKEY ]]; then
        echo "WARNING: no AKMODS_PRIVKEY secret, evdi will not load with Secure Boot on"
        touch "${WORK}/unsigned"
    else
        # An unsigned module: the build left it so, and a second signature on a signed one would never be the one checked
        [[ "$(tail -c 28 "${WORK}/evdi.ko")" != "${MARK}" ]]
        sign_file=(/usr/src/kernels/*/scripts/sign-file)
        [[ ${#sign_file[@]} == 1 && -x "${sign_file[0]}" ]]
        "${sign_file[0]}" sha256 /run/secrets/AKMODS_PRIVKEY "${CERT}" "${WORK}/evdi.ko"
        [[ "$(tail -c 28 "${WORK}/evdi.ko")" == "${MARK}" ]]
        # sign-file names the signing certificate by its serial number, which modinfo reports as sig_key
        openssl x509 -inform DER -in "${CERT}" -noout -serial | cut -d= -f2 >"${WORK}/serial"
    fi
    ;;
pack)
    mkdir -p "${OUT}"
    if [[ -e "${WORK}/unsigned" ]]; then
        install -m0644 /in/evdi.ko.xz "${OUT}/evdi.ko.xz"
    else
        # As the kernel's own build compresses its modules
        xz --check=crc32 --lzma2=dict=1MiB -c "${WORK}/evdi.ko" >"${OUT}/evdi.ko.xz"
        chmod 0644 "${OUT}/evdi.ko.xz"
        # (colon separated, where openssl prints plain hex; both may differ by leading zeros)
        [[ "$(modinfo -F sig_key "${OUT}/evdi.ko.xz" | tr -d ':' | sed 's/^0*//')" == \
            "$(sed 's/^0*//' "${WORK}/serial")" ]]
    fi
    ;;
*)
    echo "unknown step ${STEP}" >&2
    exit 1
    ;;
esac

echo "::endgroup::"
