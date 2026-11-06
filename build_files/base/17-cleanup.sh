#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# ibus-mozc starts Xwayland at every login, so that ibus-x11 runs before any X11 app does. IBus starts
# ibus-x11 itself whenever Xwayland starts (/etc/xdg/Xwayland-session.d/10-ibus-x11), and GNOME starts
# Xwayland when an X11 app needs it: without this, a session with no X11 app runs no Xwayland, some 80 MB.
test -x /etc/xdg/Xwayland-session.d/10-ibus-x11
rm -f /etc/xdg/autostart/ibus-mozc-launch-xwayland.desktop

# The login chord is Amethystora UI's sounds module's, once a session. libcanberra-gtk3 also plays desktop-login from an
# autostart entry, which the freedesktop theme never had a sound for, and which the Soft set has: it would play the chord
# a second time. gnome-session 50 reads /usr/share/gnome/autostart before /etc/xdg/autostart and keeps the first entry of
# a name, so the hidden copy in /etc/xdg/autostart alone cannot stand in for this one: it goes, and the copy stays for
# whatever reads only the XDG folders (systemd's autostart generator).
test -s /etc/xdg/autostart/libcanberra-login-sound.desktop
rm -f /usr/share/gnome/autostart/libcanberra-login-sound.desktop

# Setup Systemd
# systemctl --global enable bazaar.service
systemctl --global enable podman-auto-update.timer
systemctl --global enable amethystora-user-setup.service
# Says at login what the scheduled scans found while nobody was logged in, which would otherwise sit
# in a log file until somebody thought to look
systemctl --global enable amethystora-security-alert.service
# Upgrades every account's amethystora-pkg containers once a day. It holds back the way uupd's automatic
# updates do, and does nothing for an account that has never made one.
systemctl --global enable amethystora-pkg-upgrade.timer
# Says at login when the machine started an older version than its default one, and opens System Updates
systemctl --global enable amethystora-update-alert.service
# With encrypted DNS on (ame security dns), offers to pause it on a network that wants a login first.
# It does not start while encrypted DNS is off.
systemctl --global enable amethystora-dns-portal.service
# Clear voice and the sound presets (ame desktop clear-voice, sound-preset) start with PipeWire, and only for an
# account that turned them on: their ExecCondition reads its key, so for everybody else nothing runs
systemctl --global enable amethystora-clear-voice.service amethystora-sound-presets.service
# Offers the image whose NVIDIA driver drives the machine's card, or the one without it on a machine whose
# card none drives, once, to whoever logs in first
systemctl enable amethystora-gpu-check.timer
# One number a start, how long it took, for Control's System page (ame system start-times): nothing runs but for a moment
systemctl enable amethystora-start-times.service
# Looks once a day for a newer system and newer versions of layered packages, without installing them, for System
# Updates (ame update check off turns it off)
systemctl enable amethystora-update-check.timer
# Finds devices whose driver is missing, refused or without its firmware, for ame system drivers, and says each
# once with what fixes it
systemctl enable amethystora-driver-check.timer
# The pools of disks (ame system raid): an hourly health check that tells whoever is logged in, and a
# monthly scrub that repairs a damaged copy from a good one. Both are skipped on a machine with no pool
# and no btrfs over several disks; `ame system raid scrub off` turns the scrub off.
systemctl enable amethystora-raid-check.timer
systemctl enable amethystora-raid-scrub.timer
# The warning before the system's disk fails: an hourly read of its health, `ame system disk-warning off` turns it off
systemctl enable amethystora-disk-health.timer
# The system over several disks, at every start: a mirror onto a new disk finished after its restart, and the
# boot menu entry that starts the system with a disk missing kept current. Skipped on a system on one disk.
systemctl enable amethystora-raid-boot.service
# An update that does not start goes back by itself (ame update boot-guard). Both are skipped until it is turned on.
systemctl enable amethystora-boot-guard.service amethystora-boot-guard-good.service
# The image's kernel arguments for an update rpm-ostree stages, at the same restart (amethystora-kargs): after each
# staging, and at shutdown just before ostree finalizes it. Both are skipped on a sealed image.
systemctl enable amethystora-kargs.path amethystora-kargs-finalize.service
# A mirror's restart from its new disk, before /boot is mounted. Skipped unless a mirror waits for it.
systemctl enable amethystora-raid-mirror.service
# The file-watch sensor (amethystora-sensor, plans/done/PLAN-audit-cost.md): it records the files the
# audit rules watch without their per-system-call cost. Enabled, but its ExecCondition skips it unless the
# boot-time plan gives it a key, which it does only when its load probe passed and AUDIT_LOG and WATCHER are
# on; a kernel it cannot load on falls back to the audit rules, with nothing lost but the saving.
systemctl enable amethystora-sensor.service
# Swapped-out memory that stays unused, repacked smaller every hour on mains power (zram-generator.conf.d)
systemctl enable amethystora-zram-recompress.timer
# amepkg is amethystora-pkg under a shorter name. Made here, because a link in system_files is one the
# build's rsync would skip.
ln -sf amethystora-pkg /usr/bin/amepkg
systemctl enable brew-setup.service
systemctl enable clamav-freshclam.service
# clamd starts through its socket when something scans, and stops when nothing does (clamd@.socket)
systemctl enable clamd@scan.socket
systemctl enable amethystora-clamd-idle.timer
# The Vault's service starts through its socket when something asks, and exits when no vault is open (plans/PLAN-vault.md)
systemctl enable amethystora-vault.socket amethystora-vault-ssh.socket amethystora-vault-net.socket
systemctl enable dconf-update.service
# Blocks addresses that fail to log in too often (08-hardening.sh, /etc/fail2ban/jail.d/10-amethystora.conf)
systemctl enable fail2ban.service
systemctl enable amethystora-flatpak-remotes.service
# input-remapper is installed but not enabled: it runs as root and reads every key pressed on every
# input device, which is a keylogger by any other name and is wanted only by people who remap keys.
# `ame security input-remapper` turns it on for them.
systemctl disable input-remapper.service
# The same for Android apps, whose container runs as root (ame apps android), and for the sched-ext CPU
# schedulers, which replace the kernel's own (ame system scheduler)
systemctl disable waydroid-container.service
systemctl disable scx_loader.service
# L2TP/IPsec VPNs (NetworkManager-l2tp, 04-packages.sh): Fedora's kernel blacklists every module of
# kernel-modules-extra that a net- alias would let any account load, and the plugin's L2TP daemon gets
# l2tp_netlink and l2tp_ppp by those aliases, which a blacklist stops. They load on demand when a VPN
# uses them; l2tp_eth (L2TPv3 Ethernet pseudowires, which no VPN plugin uses) stays blacklisted.
rm -f /etc/modprobe.d/l2tp_netlink-blacklist.conf /etc/modprobe.d/l2tp_ppp-blacklist.conf
# USBGuard's D-Bus service, masked, not disabled: D-Bus activation would start it, it would start USBGuard
# with whatever rules are there, and GNOME's plugin in every session would add a rule that allows every new
# device while the screen is unlocked, which undoes `ame security usb on`. Only `ame security usb locked`
# unmasks it.
systemctl mask usbguard-dbus.service
systemctl enable rpm-ostree-countme.timer
systemctl enable tailscaled.service
systemctl enable amethystora-system-setup.service
# Keeps updates signature-checked (/usr/libexec/amethystora-signed-updates)
systemctl enable amethystora-signed-updates.service

# Audit watches on the paths that would have to change for something to survive a reboot, which this
# service writes at every boot from /usr/share/amethystora/audit and for each home, for every key not
# switched off in /etc/amethystora/security.conf (AUDIT_LOG=off skips both units)
systemctl enable auditd.service
systemctl enable amethystora-audit-rules.service

# Weekly ClamAV scan of the home and temporary directories, monthly Lynis audit of the settings. What
# the scan finds is only reported unless ON_DETECTION in /etc/amethystora/security.conf says otherwise.
systemctl enable amethystora-clamav-scan.timer
systemctl enable amethystora-lynis-audit.timer
# The security watcher reads the audit log and the journal every 15 minutes (amethystora-security-watch)
systemctl enable amethystora-security-watch.timer
# Real-time watching and scanning. Enabled, but each checks REALTIME in /etc/amethystora/security.conf as
# it starts and is skipped while that is off, which is the default
systemctl enable amethystora-security-realtime.service
systemctl enable amethystora-clamav-onaccess.service
# Network protection and its daily rules, the same way: enabled, and skipped while NETWORK=off. The
# package's own unit sniffs one named interface and never blocks, so it stays off.
systemctl enable amethystora-ips.service
systemctl enable amethystora-ips-rules.timer
systemctl disable suricata.service
# Odd names in DNS, the same way: enabled, and skipped while DNS_NAMES=off
systemctl enable amethystora-dns-names.service
# YARA rules for the virus scanner, daily, skipped while every YARA_* switch is off
systemctl enable amethystora-yara.timer
# Installed apps and the system's files checked against their updates, daily and after each change to the
# system's Flatpaks, the same way: skipped while APP_INTEGRITY=off
systemctl enable amethystora-integrity.timer amethystora-integrity.path
# The behaviour sensor, Falco, the same way: enabled, and skipped while BEHAVIOUR=off. Falco's own units stay
# off, and its rules follower, which would download rules, is masked: the rules are the package's.
systemctl enable amethystora-behaviour.service
systemctl mask falcoctl-artifact-follow.service

systemctl enable flatpak-preinstall.service

# Updater
systemctl enable uupd.timer

# Firmware: fwupd's daily look at LVFS for new firmware, which Fedora leaves to GNOME Software on its
# desktops. This image has no GNOME Software, so without it the Updates app and `fwupdmgr get-updates`
# would only ever know of the firmware there was when somebody last ran `fwupdmgr refresh`.
systemctl enable fwupd-refresh.timer

#disable the old rpm-ostreed-automatic.timer
systemctl disable rpm-ostreed-automatic.timer

# The image's own file types (usr/share/mime/packages: a shared look, ame desktop theme share), which the package
# manager's trigger would read in for a package's file but not for one the build copies in
update-mime-database /usr/share/mime

# Hide Desktop Files. Hidden removes mime associations
for file in fish htop nvtop kitty-open; do
    if [[ -f "/usr/share/applications/$file.desktop" ]]; then
        sed -i 's@\[Desktop Entry\]@\[Desktop Entry\]\nHidden=true@g' /usr/share/applications/"$file".desktop
    fi
done

# One Terminal in the app grid: kitty's launcher, under the names and keywords of Ptyxis's and the icon packs' terminal.
# Ptyxis's is NoDisplay and not Hidden, since xdg-terminal-exec drops a Hidden one and Ptyxis stays its second choice.
# Its D-Bus name, which is all Files' Open in Console asks for, starts the default terminal instead
sed '/^\[Desktop Action/q' /usr/share/applications/org.gnome.Ptyxis.desktop | grep -E '^(Name\[|GenericName|Keywords)' |
    sed 's/ptyxis;//' >/tmp/terminal-names
sed -i -e 's/^Name=kitty$/Name=Terminal/' -e '/^GenericName=/d' -e '/^Name=Terminal$/r /tmp/terminal-names' \
    -e 's/^Icon=kitty$/Icon=utilities-terminal/' /usr/share/applications/kitty.desktop
rm -f /tmp/terminal-names
sed -i '0,/^\[Desktop Entry\]$/s//[Desktop Entry]\nNoDisplay=true/' /usr/share/applications/org.gnome.Ptyxis.desktop
sed -i 's|^Exec=.*|Exec=/usr/libexec/amethystora-terminal-here|' /usr/share/dbus-1/services/org.gnome.Ptyxis.service

#Add the Flathub Flatpak remote and remove the Fedora Flatpak remote. It is added from the .flatpakrepo
# the base image carries, which machines add it from too (flatpak-add-flathub-repos.service), once the
# key inside has been checked against Flathub's fingerprint, rather than from Flathub's server unchecked
# shellcheck source=build_files/shared/copr-helpers.sh
source /ctx/build_files/shared/copr-helpers.sh
FLATHUB_REPO=/etc/flatpak/remotes.d/flathub.flatpakrepo
sed -n 's/^GPGKey=//p' "${FLATHUB_REPO}" | base64 -d >/tmp/flathub.gpg
[[ "$(key_fingerprints /tmp/flathub.gpg)" == 6E5C05D979C76DAF93C081354184DD4D907A7CAE ]]
flatpak remote-add --system --if-not-exists flathub "${FLATHUB_REPO}"
systemctl disable flatpak-add-fedora-repos.service

# NOTE: With isolated COPR installation, most repos are never enabled globally.
# We only need to clean up repos that were enabled during the build process.

# Disable every repository but Fedora's own (validate-repos.sh checks it): COPRs, RPM Fusion, Tailscale,
# Cisco's openh264 and fedora-multimedia, negativo17's, which left on outranks Fedora's repositories for
# anything installed after this, here or by the user. The base image turns that one on in
# /etc/dnf/repos.override.d, which dnf5 and rpm-ostree read after the repository files, wherever those are
# (Fedora's own are in /usr/share/dnf5/repos.d from 45), so they are turned off there too, by setopt
FEDORA_OWN_REPOS=(fedora updates updates-archive)
[[ "${UBLUE_IMAGE_TAG:-}" == beta ]] && FEDORA_OWN_REPOS+=(updates-testing)
REPO_IDS="$(dnf5 repo list --enabled --json | jq -r '.[].id')"
readarray -t ENABLED_REPOS < <(grep -vxF -f <(printf '%s\n' "${FEDORA_OWN_REPOS[@]}") <<<"${REPO_IDS}")
for repo in "${ENABLED_REPOS[@]}"; do
    dnf5 config-manager setopt "${repo}.enabled=0"
done

echo "::endgroup::"
