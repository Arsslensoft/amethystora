#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# The Amethystora apps: the Manual (Super+F1), Security, Updates, Notes, Logs, Backups, Control, Phone, the Shelf
# (a window the desktop opens while something is put down on it, round 4's R4-08) and Trim (R6-19), and the Vault (see below). They are
# one program, written in Rust with GTK 4 and libadwaita (apps/), which the Containerfile's apps stage built
# on this same base, and which picks the app by the name it is started as: each app's own path is a hard
# link to it, so that /usr/bin/amethystora-<app>, the process names and the launchers stay as they were.
# What else the apps need is in system_files already: each app's translations in
# /usr/lib/amethystora-<app>/resources/app/locale, where Weblate writes them (CONTRIBUTING.md), the manual's
# pages in /usr/share/amethystora/manual, and the launchers.
#
#   - Quicksand, the face of the Amethystora wordmark, which the windows draw it in. The image has no system
#     copy of it; the Containerfile brings branding/fonts into the build context.
#   - The licences of the Rust crates built into the program, beside the list of them (CRATES).

APPS_DIR=/usr/lib/amethystora-apps
LICENSE_DIR=/usr/share/licenses/amethystora-apps

install -Dpm0755 /apps-stage/out/amethystora-apps "${APPS_DIR}/amethystora-apps"
for app in manual security update notes logs backups control phone shelf trim help vault; do
    install -d "/usr/lib/amethystora-${app}"
    ln -f "${APPS_DIR}/amethystora-apps" "/usr/lib/amethystora-${app}/amethystora-${app}"
done

# The file-watch sensor is a binary of its own (no GTK), not one of the windowed apps: it loads a BPF object
# and writes audit records for the security watcher (plans/done/PLAN-audit-cost.md). Its BPF object is
# Amethystora's own code, dual-licensed so it may use the kernel's GPL-only BPF helpers; the Rust crates
# built into it are listed with the apps' in ${LICENSE_DIR}/crates.
install -Dpm0755 /apps-stage/out/amethystora-sensor /usr/libexec/amethystora-sensor
install -Dpm0644 /ctx/apps/sensor/src/bpf/COPYING /usr/share/licenses/amethystora-sensor/COPYING

# The AI agent's tools (no GTK either): the MCP server the launcher gives each agent, and the model its search
# ranks by meaning with, as downloaded and checked in the apps stage. The index itself is written once every
# name in the image is final (build-agent-index.sh). Off costs nothing: nothing here runs until an agent the
# user started starts it.
install -Dpm0755 /apps-stage/out/amethystora-agent-tools /usr/libexec/amethystora-agent-tools
# The helper that runs the agent's root commands, started with pkexec only (org.amethystora.agent.policy)
install -Dpm0755 /apps-stage/out/amethystora-agent-root /usr/libexec/amethystora-agent-root

# What Amethystora Clipboard, the GNOME Shell extension, seals its history with: a filter from stdin to stdout
# with no GTK, the key and the data never on its command line
install -Dpm0755 /apps-stage/out/amethystora-clipboard-seal /usr/libexec/amethystora-clipboard-seal

# The phone link's program (no GTK either), started by the user unit amethystora-connect.service, which nothing
# enables for anyone: `ame desktop phone on` does. The window it is paired and used from is amethystora-phone.
install -Dpm0755 /apps-stage/out/amethystora-connect /usr/libexec/amethystora-connect

# Amethystora Vault (plans/PLAN-vault.md; the window is the app above). The service holds the keys, started by
# amethystora-vault.socket as its own user; the command line is what `ame vault` runs; the work child is the only one of
# them that loads libcrypto (keys and certificates), one job at a time; the PKCS#11 module is what pdfsig loads to sign through
# the service (the key never leaves it); and the network service is the only thing sync reaches a server through (a user of its own, no key, no state).
for program in service cli work net; do
    install -Dpm0755 "/apps-stage/out/amethystora-vault-${program}" "/usr/libexec/amethystora-vault-${program}"
done
install -Dpm0755 /apps-stage/out/libamethystora_vault_pkcs11.so /usr/lib64/amethystora-vault/libamethystora_vault_pkcs11.so
# What git and docker look for on their PATH, under the names they look for, to have a login from the vault (ame vault helpers)
install -Dpm0755 /apps-stage/out/git-credential-amethystora /usr/bin/git-credential-amethystora
install -Dpm0755 /apps-stage/out/docker-credential-amethystora /usr/bin/docker-credential-amethystora
install -d /usr/share/amethystora/agents/knowledge/model
install -pm0644 -t /usr/share/amethystora/agents/knowledge/model /apps-stage/out/model/*

install -Dpm0644 /ctx/branding/fonts/QuicksandVariable.ttf "${APPS_DIR}/fonts/QuicksandVariable.ttf"
install -Dpm0644 /ctx/branding/fonts/Quicksand-OFL.txt "${LICENSE_DIR}/Quicksand-OFL.txt"
install -d "${LICENSE_DIR}/crates"
cp -a /apps-stage/out/licenses/. "${LICENSE_DIR}/crates/"

# The apps' launchers in the languages the apps are translated into: their names, what they are and what
# to find them by, from the same catalogs as the windows' own words
python3 /ctx/build_files/shared/translate-desktop.py /

echo "::endgroup::"
