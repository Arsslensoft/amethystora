#!/usr/bin/bash
# The Containerfile's apps stage: the Amethystora apps (apps/, a Cargo workspace), built with Fedora's Rust
# on the same base image they run on, so that they link against the GTK and libadwaita the image has. The
# tests run first: the shared crate's, which hold every date, number and list to what the Electron windows
# wrote, and every app's, Notes' among them reading and writing the Electron window's own files. Only the
# program and the licences of the crates built into it leave the stage, in /out (13-apps.sh installs them):
# the toolchain and the crates' sources stay here, and the build itself is cleared away once done. The
# stage's Node runs Notes' tests of the Electron side, and the checks written in JavaScript in 20-tests.sh.
#
# Cargo.lock pins every crate by version and checksum (--locked), and the crates come from crates.io, which
# serves them by that checksum.

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# The toolchain and the libraries' headers from Fedora's own repositories alone (build-fprintd.sh says why).
# clang, bpftool, libbpf-devel and kernel-devel (for vmlinux.h) build amethystora-sensor's BPF object; the
# sensor links Fedora's libbpf, which the image itself carries, so nothing of libbpf is vendored.
dnf5 -y install --repo=fedora --repo=updates --setopt=install_weak_deps=False \
    rust cargo gcc gtk4-devel libadwaita-devel jq nodejs clang llvm bpftool libbpf-devel kernel-devel libseccomp-devel
# The Vault: the openssl crate (keys and certificates, in the one short-lived child that loads libcrypto) builds against
# openssl-devel, and its tests hold the work to what other programs make of it: openssl, osslsigncode, pdfsig and pdftotext
# (poppler-utils), certutil (nss-tools), ssh-keygen, oathtool, qrencode, a JDK for jarsigner and a private dbus-daemon
# for the service's desktop tests
dnf5 -y install --repo=fedora --repo=updates --setopt=install_weak_deps=False \
    openssl-devel pkgconf-pkg-config openssl osslsigncode poppler-utils nss-tools openssh-clients oathtool qrencode \
    java-latest-openjdk-devel dbus-daemon
# Node's whole ICU data, for 20-tests.sh's checks of numbers and dates in other languages (Fedora's node has only
# English without it); the package is named after the node version, so ask the one that came with nodejs.
# 20-tests.sh points NODE_ICU_DATA at /apps-stage/usr/share/node-*/icudata, since it runs this node from there.
dnf5 -y install --repo=fedora --repo=updates --setopt=install_weak_deps=False \
    "$(rpm -qf --qf '%{NAME}' "$(command -v node)" | sed 's/-bin$//')-full-i18n"

# The workspace is copied out of the read-only context, and built outside it
cp -a /ctx/apps /tmp/apps
cd /tmp/apps
export CARGO_TARGET_DIR=/tmp/target CARGO_HOME=/tmp/cargo CARGO_TERM_COLOR=never
cargo fetch --locked

# The AI agent's search ranks passages by meaning with potion-base-8M (MIT): its files at one revision of
# the model's repository, each checked against its SHA-256 (usr/share/licenses/potion-base-8M/SOURCE).
# The agent tools' tests compare their tokenizer and vectors with it, and their golden questions are asked
# of an index of the source tree's pages (AMETHYSTORA_ROOT).
MODEL_REVISION=bf8b056651a2c21b8d2565580b8569da283cab23
declare -A MODEL=(
    [config.json]=2a6ac0e9aaa356a68a5688070db78fc3a464fefe85d2f06a1905ce3718687553
    [model.safetensors]=f65d0f325faadc1e121c319e2faa41170d3fa07d8c89abd48ca5358d9a223de2
    [tokenizer.json]=e67e803f624fb4d67dea1c730d06e1067e1b14d830e2c2202569e3ef0f70bb50
)
mkdir -p /out/model
for file in "${!MODEL[@]}"; do
    curl -fsSL --retry 3 -o "/out/model/${file}" "https://huggingface.co/minishlab/potion-base-8M/resolve/${MODEL_REVISION}/${file}"
    echo "${MODEL[${file}]}  /out/model/${file}" | sha256sum -c -
done
export AMETHYSTORA_MODEL=/out/model AMETHYSTORA_ROOT=/ctx/system_files/shared

# A tool a Vault test needs and the stage lacks fails the build, where it would otherwise skip with a line and pass
export VAULT_VERIFIERS_REQUIRED=1

# The Vault's PDF test signs through the real PKCS#11 module, which `cargo test` does not build: build it first. (The test backend's
# run below writes the same file; the last build of the module, further down, makes the real one again.)
cargo build --release --locked -p amethystora-vault-pkcs11
# The golden tests of the shared crate compare with what Chromium wrote in UTC. One thread: Notes' tests
# run its timers on GLib's default main context, which only one thread may own
TZ=UTC cargo test --release --locked --workspace -- --test-threads=1
# The PKCS#11 module signs a PDF through pdfsig only with its test backend, which is never built into the image
TZ=UTC cargo test --release --locked -p amethystora-vault-pkcs11 --features amethystora-vault-pkcs11/test-backend -- --test-threads=1
cargo build --release --locked -p amethystora-apps
# The sensor is a binary of its own (no GTK): it loads a BPF object and writes audit records for the
# security watcher (plans/done/PLAN-audit-cost.md).
cargo build --release --locked -p amethystora-sensor
# The agent's tools are two more (no GTK): its MCP server and the helper that runs its root commands
cargo build --release --locked -p amethystora-agent-tools
# And one more for Amethystora Clipboard, the GNOME Shell extension, which seals its history with it
cargo build --release --locked -p amethystora-clipboard-seal
# The phone link's program (no GTK): it speaks the KDE Connect protocol, so that the phone's app works with it
cargo build --release --locked -p amethystora-connect
# Move to this computer: the two ends of a move over the home network (no GTK)
cargo build --release --locked -p amethystora-move
# The plugin system (plans/PLAN-plugins.md): the host that runs plugins in a sandbox, the root helper that is the only writer of
# their store, and the sandbox's seccomp filter, written by a small C program with libseccomp (the version the image has)
cargo build --release --locked -p amethystora-plugin-host
gcc -Wall -O2 -o /tmp/seccomp-gen /tmp/apps/plugin-host/seccomp/gen.c -lseccomp
/tmp/seccomp-gen /out/seccomp.bpf
# The Vault (plans/PLAN-vault.md): the service, its command line and the child that does its libcrypto work,
# the PKCS#11 module pdfsig loads, and the network service that sync reaches a server through (no key, no state)
cargo build --release --locked -p amethystora-vault-service -p amethystora-vault-pkcs11 -p amethystora-vault-net

mkdir -p /out/licenses
install -Dpm0755 "${CARGO_TARGET_DIR}/release/amethystora-apps" /out/amethystora-apps
install -Dpm0755 "${CARGO_TARGET_DIR}/release/amethystora-sensor" /out/amethystora-sensor
for program in amethystora-agent-tools amethystora-agent-root amethystora-clipboard-seal amethystora-connect amethystora-move amethystora-plugin-host amethystora-plugin-root \
    amethystora-vault-service amethystora-vault-cli amethystora-vault-work amethystora-vault-net \
    git-credential-amethystora docker-credential-amethystora; do
    install -Dpm0755 "${CARGO_TARGET_DIR}/release/${program}" "/out/${program}"
done
install -Dpm0755 "${CARGO_TARGET_DIR}/release/libamethystora_vault_pkcs11.so" /out/libamethystora_vault_pkcs11.so
# The service holds every open vault's keys and must never map libcrypto (its memory is 8 MB, and libcrypto parses what it is
# given): the openssl crate is the work child's and the command line's alone
for program in amethystora-vault-service; do
    ! ldd "${CARGO_TARGET_DIR}/release/${program}" | grep -q libcrypto ||
        { echo "::error::${program} is linked against libcrypto"; exit 1; }
done
# It must link the image's own libbpf, not a vendored copy, or the version it was built against travels with
# it: the image carries libbpf and the apps stage built against the same Fedora release.
ldd "${CARGO_TARGET_DIR}/release/amethystora-sensor" | grep -q 'libbpf\.so\.1' ||
    { echo "::error::amethystora-sensor is not linked against the system libbpf"; exit 1; }

# Every crate built into the program (its normal dependencies: build tools and test-only crates are not in
# it), with its licence, and the licence and notice files it ships: most are MIT or Apache-2.0, whose
# terms ask for their notices to travel with the program
cargo tree --locked -p amethystora-apps -p amethystora-sensor -p amethystora-agent-tools -p amethystora-clipboard-seal -p amethystora-connect -p amethystora-move -p amethystora-plugin-host \
    -p amethystora-vault-service -p amethystora-vault-pkcs11 -p amethystora-vault-net -e normal --prefix none --format '{p}' |
    sed -n 's/^\([^ ]*\) v\([^ ]*\)\( (\*)\)\{0,1\}$/\1\t\2/p' | sort -u >/tmp/built.tsv
cargo metadata --locked --format-version 1 >/tmp/metadata.json
jq -r '.packages[] | select(.source != null)
    | [.name, .version, (.license // "see its licence file"), (.repository // ""), (.manifest_path | sub("/Cargo.toml$"; ""))]
    | @tsv' /tmp/metadata.json | sort -u >/tmp/packages.tsv
printf '# The Rust crates built into the Amethystora apps, amethystora-sensor, the agent tools, the clipboard seal, the phone link, the move, the plugin host and the Vault: name, version, licence, source\n' \
    >/out/licenses/CRATES
missing=()
while IFS=$'\t' read -r name version licence repository folder; do
    grep -qxF "${name}"$'\t'"${version}" /tmp/built.tsv || continue
    printf '%s\t%s\t%s\t%s\n' "${name}" "${version}" "${licence}" "${repository}" >>/out/licenses/CRATES
    mkdir -p "/out/licenses/${name}-${version}"
    find "${folder}" -maxdepth 1 -type f \( -iname 'LICEN[CS]E*' -o -iname 'COPYING*' -o -iname 'NOTICE*' -o -iname 'COPYRIGHT*' -o -iname 'UNLICENSE*' \) \
        -exec install -pm0644 -t "/out/licenses/${name}-${version}" {} +
    rmdir "/out/licenses/${name}-${version}" 2>/dev/null && missing+=("${name}-${version}")
done </tmp/packages.tsv
# Every crate in the program is listed, and one that ships no licence file of its own stops the build, so
# that its licence is looked at before it reaches the image
[[ "$(($(wc -l </out/licenses/CRATES) - 1))" == "$(wc -l </tmp/built.tsv)" ]]
if ((${#missing[@]})); then
    echo "::error::crates with no licence file of their own: ${missing[*]}"
    exit 1
fi
ls -l /out
echo "$(($(wc -l </out/licenses/CRATES) - 1)) crates, each with its licence"

cd /
rm -rf /tmp/apps /tmp/target /tmp/cargo /tmp/built.tsv /tmp/packages.tsv /tmp/metadata.json /tmp/amethystora-notes-test-*

echo "::endgroup::"
