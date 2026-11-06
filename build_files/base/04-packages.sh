#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -ouex pipefail

# All DNF-related operations should be done here whenever possible

# shellcheck source=build_files/shared/copr-helpers.sh
source /ctx/build_files/shared/copr-helpers.sh

# NOTE:
# Packages are split into FEDORA_PACKAGES and COPR_PACKAGES to prevent
# malicious COPRs from injecting fake versions of Fedora packages.
# Fedora packages are installed first in bulk (safe).
# COPR packages are installed individually with isolated enablement.

# Base packages from Fedora repos - common to all versions
FEDORA_PACKAGES=(
    # The interface and monospace fonts of the themes that Fedora packages (each theme's font.theme and mono.theme,
    # ame desktop font and mono-font): Comfortaa, Rubik, IBM Plex Mono and Sans, Geist and Geist Mono, Fira Code,
    # Nunito and Work Sans here, Inter and JetBrains Mono below. The others come from Google Fonts, pinned, in
    # 09-fonts.sh.
    aajohan-comfortaa-fonts
    adcli
    adw-gtk3-theme
    adwaita-fonts-all
    audit
    autofs
    bash-color-prompt
    bcache-tools
    bootc
    borgbackup
    # What the kernel's BPF runs, for the security watcher (BPF_PROGRAMS)
    bpftool
    # The pointers of the themes (themes/*/cursor.theme): Breeze Light, white, for a dark theme, and Breeze, dark, for a
    # light one. Flatpak applications find them through the host's /usr/share/icons, which it binds into the sandbox.
    breeze-cursor-theme
    # Ransomware protection's snapshots of /var/home (ame security ransomware)
    btrfs-progs
    # Microsoft's firmware for the Xbox wireless adapter, out of the driver package it comes in (ame apps xbox-adapter)
    cabextract
    clamav
    clamav-freshclam
    clamd
    containerd
    cryfs
    davfs2
    ddcutil
    dotnet-sdk-10.0
    evtest
    fail2ban-firewalld
    fail2ban-selinux
    fail2ban-server
    fastfetch
    # fido2-token, fido2-cred and fido2-assert, through which the Notes app speaks to a security key
    # for its passkeys; libfido2 itself is already in the base image
    fido2-tools
    # Catppuccin's monospace font (themes/*/mono.theme)
    fira-code-fonts
    # The browser, in the image so that a machine has one before it is online. It ships as Fedora
    # builds it, but for Fedora's start page, taken out below: Mozilla lets Firefox be redistributed
    # only unaltered, so nothing here or in system_files adds a policy, an extension, a preference
    # or a home page to it
    firefox
    firefox-langpacks
    firewall-config
    fish
    foo2zjs
    fuse-encfs
    # gamemoded, which a game asks through the GameMode portal for the CPU's performance mode while it
    # runs: Steam and the rest from Flathub reach it from their sandbox (ame apps gaming)
    gamemode
    gcc
    gcc-c++
    # Live captions' speech recognizer: GStreamer's vosk element with Vosk (Apache-2.0, libvosk from vosk-api-devel,
    # with openfst and rnnoise), about 64 MiB; /usr/libexec/amethystora-captions runs it, and only once the account
    # turned captions on and fetched the model (plans/beyond/spec-r4-later.md, part B)
    gst-vosk
    # Hide what's private in a screenshot, and the reading behind it (amethystora-cover-private): Tesseract (Apache-2.0)
    # with its English data, about 11 MiB with leptonica; it runs only after the button is pressed
    tesseract
    tesseract-langpack-eng
    # gettext and gettext.sh, through which the security report and the app permissions speak the
    # session's language (po/amethystora)
    gettext-runtime
    git-credential-libsecret
    glow
    # GNOME's Extensions app, the logo menu's "Extensions": it reads each extension's details from the
    # extension itself, where Extension Manager asks extensions.gnome.org, which has none of the image's own
    gnome-extensions-app
    gnome-tweaks
    # The private folder that closes when the screen locks (ame security private-folder)
    gocryptfs
    # One-time codes for password logins over SSH, off until `ame security ssh-codes on`
    google-authenticator
    google-rubik-fonts
    # grub2-mkfont, which 06-branding.sh uses to build the boot menu font
    grub2-tools-extra
    gum
    hplip
    ibm-plex-mono-fonts
    ibm-plex-sans-fonts
    ibus-mozc
    ifuse
    igt-gpu-tools
    input-remapper
    iwd
    jetbrains-mono-fonts-all
    just
    # The terminal (xdg-terminal-exec's first choice, /etc/xdg/kitty/kitty.conf). Ptyxis, the base image's, stays
    # behind it for the container menu it has.
    kitty
    krb5-workstation
    libappindicator-gtk3
    libayatana-appindicator-gtk3
    libgda
    libgda-sqlite
    libimobiledevice
    libratbag-ratbagd
    libxcrypt-compat
    lm_sensors
    lynis
    make
    # Parental Controls' allowed hours at the text consoles and over SSH (08-hardening.sh)
    malcontent-pam
    # Copy without hidden data, in Files' scripts menu (amethystora-clean-copy): a photo's, PDF's or office file's
    # location, camera, author and history taken out of a copy, with ExifTool, through which mat2 reads a picture's
    # metadata (only its weak dependency, so named here)
    mat2
    mesa-libGLU
    mozc
    # The VPNs people are given at work: L2TP/IPsec, IPsec (IKEv2), Microsoft's SSTP and, up to Fedora 44,
    # Fortinet (below). OpenConnect, OpenVPN and up to 44 vpnc come with the base image; PPTP is left out on
    # purpose (MS-CHAPv2 breaks offline). None of them runs a service until a connection comes up (17-cleanup.sh
    # frees the modules L2TP needs).
    NetworkManager-l2tp
    NetworkManager-l2tp-gnome
    NetworkManager-libreswan
    NetworkManager-libreswan-gnome
    NetworkManager-sstp
    NetworkManager-sstp-gnome
    # certutil and modutil, through which `ame vault pdf sign` gives pdfsig a throwaway NSS database holding only the Vault's
    # PKCS#11 module (amethystora-vault-cli)
    nss-tools
    oddjob-mkhomedir
    opendyslexic-fonts
    openssh-askpass
    # Who signed a Windows program, read before it is run (amethystora-windows-program)
    osslsigncode
    pam-u2f
    pamu2fcfg
    perl-Image-ExifTool
    # The camera behind a QR code the Vault's window reads (pipewiresrc, through the camera portal's descriptor, for gst-launch-1.0)
    pipewire-gstreamer
    plymouth-plugin-label
    plymouth-plugin-script
    # pdftotext, which reads backup codes out of a PDF, and pdfsig, which checks a signed one (amethystora-vault-cli); the base
    # image has them, and the Vault depends on them
    poppler-utils
    powerstat
    powertop
    printer-driver-brlaser
    pulseaudio-utils
    python3-pip
    # The two programs that run a small model on the machine, with no GPU: the camera's blur and light (amethystora-camera, R7-68) and
    # the photo index (amethystora-photos, R7-63). Both are offered and off, and their models are downloaded only when asked
    python3-numpy
    python3-onnxruntime
    python3-pygit2
    # QR codes of a copied link and of the Wi-Fi in use (Amethystora UI's tools module), the text on its stdin
    qrencode
    rclone
    restic
    rsms-inter-fonts
    samba
    samba-dcerpc
    samba-ldb-ldap-modules
    samba-winbind-clients
    samba-winbind-modules
    setools-console
    # SSSD's back ends: Active Directory's, which a join through realmd needs (Settings' Enterprise Login,
    # `realm join`), FreeIPA's, and LDAP and Kerberos for directories set up by hand. /etc/realmd.conf keeps
    # the image's authselect features.
    sssd
    sssd-nfs-idmap
    # Network protection (amethystora-ips.service), off until `ame security settings network`
    suricata
    switcheroo-control
    tmux
    tpm2-tools
    usbguard
    # USBGuard on D-Bus, through which GNOME blocks new USB devices while the screen is locked (ame
    # security usb locked); masked otherwise (17-cleanup.sh)
    usbguard-dbus
    usbip
    usbmuxd
    vercel-geist-fonts
    # Matte Black's monospace font (themes/*/mono.theme); the static files, not the variable ones, which carry the
    # same family name
    vercel-geist-mono-fonts
    vernnobile-nunito-fonts
    # Android apps in a container (ame apps android): off until set up, as its container service ships
    # disabled (17-cleanup.sh). Binder is built into Fedora's kernel, and the package brings its own
    # SELinux policy. The Android images are downloaded on the machine that asks for them.
    waydroid
    waypipe
    weiweihuanghuang-work-sans-fonts
    wireguard-tools
    wl-clipboard
    xdg-terminal-exec
    xprop
    zenity
    zsh
)
# Version-specific Fedora package additions. Fedora 45 retired the Fortinet and vpnc plugins (its
# NetworkManager obsoletes them); OpenConnect's Fortinet protocol is left for a FortiGate there
if ((FEDORA_MAJOR_VERSION < 45)); then
    FEDORA_PACKAGES+=(NetworkManager-fortisslvpn NetworkManager-fortisslvpn-gnome)
fi
case "$FEDORA_MAJOR_VERSION" in
    42)
        FEDORA_PACKAGES+=(
            evolution-ews-core
            uld
        )
        ;;
    43)
        FEDORA_PACKAGES+=(
            evolution-ews-core
            gnupg2-scdaemon
        )
        ;;
    44 | 45)
        FEDORA_PACKAGES+=(
            gnupg2-scdaemon
        )
        ;;
esac

# Suricata's account, made here before its package makes it. The package would take the next free ID
# from the top of the system range, and every account created after it in this build (clamscan,
# setroubleshoot, docker, libvirt, wireshark and more) would move down by one, leaving the files each
# owns on a machine that updates to the wrong account. 850 is far below where those are handed out.
systemd-sysusers --inline 'u suricata 850 "Suricata IDS" / -'

# Install all Fedora packages (bulk - safe from COPR injection). From Fedora's repositories by name: the
# akmods Copr (03-install-kernel-akmods.sh) and negativo17's fedora-multimedia, which outranks Fedora's,
# are still enabled here, until 17-cleanup.sh
FEDORA_REPOS=(--repo=fedora --repo=updates)
if [[ "${UBLUE_IMAGE_TAG}" == "beta" ]]; then
    FEDORA_REPOS+=(--repo=updates-testing)
fi
echo "Installing ${#FEDORA_PACKAGES[@]} packages from Fedora repos..."
dnf -y install "${FEDORA_REPOS[@]}" "${FEDORA_PACKAGES[@]}"

# Fedora's Firefox opens on start.fedoraproject.org and pins it on the new tab page. Those two lines of
# its preferences are deleted, which leaves Firefox's own home page; nothing is put in their place
sed -i '/start\.fedoraproject\.org/d' /usr/lib64/firefox/browser/defaults/preferences/firefox-redhat-default-prefs.js

# ClamAV daemon (clamd@scan.service): listen on its local socket so clamdscan and ClamUI can use it,
# and let SELinux allow it to read files anywhere on the system
sed -i 's|^#LocalSocket |LocalSocket |' /etc/clamd.d/scan.conf
grep -q "^LocalSocket /run/clamd.scan/clamd.sock$" /etc/clamd.d/scan.conf
semanage boolean -m --on antivirus_can_scan_system

# Kept out of the weekly scan (amethystora-clamav-scan.timer). clamdscan takes its exclusions from the
# daemon's own configuration rather than from the command line, so they have to be set here. These are
# the kernel's virtual filesystems, the two content-addressed stores whose files are verified by their
# checksums and replaced whole, and the caches, which are large, rewritten constantly and rebuilt on
# demand. Everything a user writes stays in the scan, Flatpak application data included. The snapshots
# of ransomware protection are read-only copies of what is scanned already, one for each of the last
# day's hours and the last two weeks' days.
tee -a /etc/clamd.d/scan.conf >/dev/null <<'CLAMD'

# Amethystora: directories left out of the weekly scan (/usr/libexec/amethystora-clamav-scan)
ExcludePath ^/proc/
ExcludePath ^/sys/
ExcludePath ^/dev/
ExcludePath ^/run/
ExcludePath ^/var/lib/flatpak/
ExcludePath ^/var/lib/containers/
ExcludePath ^/var/home/[^/]+/\.cache/
ExcludePath ^/var/home/[^/]+/\.var/app/[^/]+/cache/
ExcludePath ^/var/home/[^/]+/\.local/share/containers/
ExcludePath ^/var/home/\.snapshots/

# Amethystora: real-time scanning, read by clamonacc (amethystora-clamav-onaccess.service), which runs
# only with REALTIME=on in /etc/amethystora/security.conf. Files closed in a home directory are scanned;
# root's own reads are not, or the weekly scan would have every file it opens scanned a second time.
OnAccessIncludePath /var/home
OnAccessExcludeRootUID yes
OnAccessExtraScanning yes
OnAccessPrevention no
CLAMD
grep -q "^ExcludePath \^/proc/$" /etc/clamd.d/scan.conf
grep -q "^OnAccessIncludePath /var/home$" /etc/clamd.d/scan.conf

# FIDO2 security keys (YubiKey, Thetis, and their fingerprint models) log in, unlock and approve sudo/polkit in
# place of the password, once registered with `ame security key`. Users without a key are not affected.
authselect enable-feature with-pam-u2f

dnf config-manager addrepo --from-repofile=https://pkgs.tailscale.com/stable/fedora/tailscale.repo
dnf config-manager setopt tailscale-stable.enabled=0
# Tailscale's package repository signing key, as served on 2026-10-08
pin_repo_key /etc/yum.repos.d/tailscale.repo 2596A99EAAB33821893C0A79458CA832957F5868
dnf -y install --enablerepo='tailscale-stable' tailscale

# The behaviour sensor (BEHAVIOUR in /etc/amethystora/security.conf, off as the image ships): Falco and its
# modern eBPF probe, from falcosecurity's own repository, whose packages and metadata are both signed.
# No driver is chosen at install: any other choice has its scriptlet load a test BPF program, which a build
# cannot, and enable Falco's own units. amethystora-behaviour.service runs it, with the configuration in
# /etc/falco/config.d/60-amethystora.yaml, and its rules follower, which would download rules, is masked
# (17-cleanup.sh). Apache-2.0; its eBPF probe is MIT or GPL-2.0-only, at the user's choice. The package leaves out
# the licences of the libraries compiled into usr/bin/falco, which ship in usr/share/licenses/falco/bundled at the
# versions Falco 0.45.2 and its libs 0.26.1 pin: look at them again when Falco moves.
cat >/etc/yum.repos.d/falcosecurity.repo <<'EOF'
[falcosecurity]
name=falcosecurity
baseurl=https://download.falco.org/packages/rpm
enabled=0
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://falco.org/repo/falcosecurity-packages.asc
EOF
# falcosecurity's package signing key, as served on 2026-10-10, which expires on 2028-12-10
pin_repo_key /etc/yum.repos.d/falcosecurity.repo 478B2FBBC75F4237B731DA4365106822B35B1B1F
# Falco 0.45.0's probe does not load into Fedora 44's kernel (falco#4017); 0.45.1 and later do (20-tests.sh)
FALCO_FRONTEND=noninteractive FALCO_DRIVER_CHOICE=none dnf -y install --enablerepo=falcosecurity falco

# The AI agents amethystora-agent offers are deliberately not installed here. They release far more
# often than the image, so it installs them into each user's home with their makers' own installers,
# where they update on a schedule the user controls.

# From che/nerd-fonts
copr_install_isolated "che/nerd-fonts" "nerd-fonts"

# From ublue-os/packages
copr_install_isolated "ublue-os/packages" "uupd"
copr_install_isolated "ublue-os/packages" "gnome-rounded-blur"

# sched-ext CPU schedulers and their loader (ame system scheduler), from CachyOS's COPR, which Bazzite uses
# too: Fedora's own scx packages are two years older than the kernel interface they would load into.
# scx_loader.service ships disabled (17-cleanup.sh), so the kernel's own scheduler runs until somebody
# picks another. GPL-2.0, so source-manifest.sh fetches the source packages into the image.
copr_install_isolated "bieszczaders/kernel-cachyos-addons" "scx-scheds" "scx-tools"
# libbpf reads the kernel's build configuration from /boot/config-RELEASE, which an ostree machine keeps in
# /usr/lib/modules instead, hidden from scx_loader by its ProtectKernelModules: without it every scheduler
# failed to load (-EINVAL). The loader's drop-in mounts this folder on its /boot.
mkdir -p /usr/lib/amethystora/kconfig
for config in /usr/lib/modules/*/config; do
    release="$(basename "$(dirname "${config}")")"
    ln -f "${config}" "/usr/lib/amethystora/kconfig/config-${release}" 2>/dev/null ||
        cp "${config}" "/usr/lib/amethystora/kconfig/config-${release}"
done

# Version-specific COPR packages
# case "$FEDORA_MAJOR_VERSION" in
#    42)
        # bazaar and uupd from ublue-os/packages
        # copr_install_isolated "ublue-os/packages" "bazaar" "uupd"
        # ;;
    # 43)
        # bazaar from ublue-os/packages
        # copr_install_isolated "ublue-os/packages" "bazaar"
        # ;;
# esac

# Packages to exclude - common to all versions
EXCLUDED_PACKAGES=(
    cosign
    fedora-bookmarks
    fedora-chromium-config
    fedora-chromium-config-gnome
    # The GNOME Classic session and the four extensions only it turns on (Apps Menu, Places Status Indicator,
    # Window List, Launch New Instance), which sat off in every other session (20-tests.sh)
    gnome-classic-session
    gnome-shell-extension-apps-menu
    gnome-shell-extension-common
    gnome-shell-extension-launch-new-instance
    gnome-shell-extension-places-menu
    gnome-shell-extension-window-list
    gnome-shell-extension-background-logo
    # The base image's copy of Blur my Shell, which Amethystora UI's glass module replaces (20-tests.sh)
    gnome-shell-extension-blur-my-shell
    # GSConnect and its Files extension, which the phone link (apps/connect) replaces. A package of the base or a
    # dependency of another must not bring them back (20-tests.sh)
    gnome-shell-extension-gsconnect
    nautilus-gsconnect
    gnome-software
    gnome-software-rpm-ostree
    gnome-terminal-nautilus
    # IBus started its predictive engine, a Python program of some 70 MB, in every session, with no input
    # source using it. `rpm-ostree install ibus-typing-booster` brings it back for those who type with it.
    ibus-typing-booster
    podman-docker
    yelp
)

# Remove excluded packages if they are installed
if [[ "${#EXCLUDED_PACKAGES[@]}" -gt 0 ]]; then
    readarray -t INSTALLED_EXCLUDED < <(rpm -qa --queryformat='%{NAME}\n' "${EXCLUDED_PACKAGES[@]}" 2>/dev/null || true)
    if [[ "${#INSTALLED_EXCLUDED[@]}" -gt 0 ]]; then
        dnf -y remove "${INSTALLED_EXCLUDED[@]}"
    else
        echo "No excluded packages found to remove."
    fi
fi

## Pins and Overrides
## Use this section to pin packages in order to avoid regressions
# Remember to leave a note with rationale/link to issue for each pin!
#
# Example:
#if [ "$FEDORA_MAJOR_VERSION" -eq "41" ]; then
#    Workaround pkcs11-provider regression, see issue #1943
#    rpm-ostree override replace https://bodhi.fedoraproject.org/updates/FEDORA-2024-dd2e9fb225
#fi

echo "::endgroup::"
