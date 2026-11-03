#!/usr/bin/env bash
#
# Take away the file the Vault's browser switch of an earlier image put in the account's browsers' native-messaging folders.
#
# The Vault had a browser extension of its own, and this one small file let the browser start the program behind it. The extension
# is gone (Bitwarden's works with the same account that the Vault's sync uses: the manual's Vault page says how), and so is the
# program, so the file now names nothing, and the security watcher no longer knows it as the image's own.
#
# Only that file goes, and only a plain file, never through a link, whose `path` is exactly the program that is gone: a manifest
# of the same name that says anything else is somebody's, and stays, for the watcher to say. A folder the file leaves empty goes
# too (the image made them for it), never `.config` or the home itself.

source /usr/lib/amethystora/setup-services/libsetup.sh

command -v jq >/dev/null || exit 0

version-script retire-vault-browser user 1 || exit 0

FOLDERS=(
    .mozilla/native-messaging-hosts
    .config/google-chrome/NativeMessagingHosts
    .config/chromium/NativeMessagingHosts
    .config/microsoft-edge/NativeMessagingHosts
    .config/BraveSoftware/Brave-Browser/NativeMessagingHosts
)

for folder in "${FOLDERS[@]}"; do
    file="${HOME}/${folder}/org.amethystora.vault.json"
    [[ -f ${file} && ! -L ${file} && "$(realpath -e -- "${file}" 2>/dev/null)" == "$(realpath -e -- "${HOME}" 2>/dev/null)/${folder}/org.amethystora.vault.json" ]] || continue
    [[ "$(stat -c %s -- "${file}" 2>/dev/null)" -le 4096 ]] || continue
    [[ "$(timeout 5 jq -r '.path // empty' "${file}" 2>/dev/null)" == /usr/libexec/amethystora-vault-native ]] || continue
    rm -f -- "${file}"
    place=${folder}
    while [[ ${place} != .config ]]; do
        rmdir -- "${HOME}/${place}" 2>/dev/null || break
        [[ ${place} == */* ]] || break
        place=${place%/*}
    done
done

exit 0
