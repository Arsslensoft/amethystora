# What the installer in the live session installs: the image the live system carries a copy of
# (iso/live/build.sh puts it in the read-only image store), so that installing needs no network.
# build.sh replaces @IMAGE@ with the image the ISO was built from. Through bootc install, not ostreecontainer
# (ostree container image deploy): it unpacks without syncing the disk after each layer. Anaconda has the
# command from Fedora 43. The installer ISO (iso/iso.toml) cannot take it: its builder
# writes an ostreecontainer line of its own, and the two commands refuse each other.
bootc --source-imgref=containers-storage:@IMAGE@ --target-imgref=@IMAGE@

# Left alone, Anaconda runs `firewall-offline-cmd --enabled --service=ssh`, which adds ssh to the image's
# default zone on every machine. The firewall is the image's: build.sh fails without this line. iso/iso.toml's
# installer has Anaconda's Network module off, so it has no such step, and no such command to give.
firewall --use-system-defaults

# Then the installed machine follows that image in the registry rather than the copy, signature-checked
# from its first update, as one installed from the installer ISO does (iso/iso.toml). build.sh fails on
# a switch without the flag. The marker tells the first boot that this is a new installation, which gets
# the hourly snapshots of the home folders (system-setup.hooks.d/26-snapshots-new-installation.sh).
# Anaconda goes by the script's last exit status only, hence set -e. On the NVIDIA images, with Secure
# Boot on, the driver's key is queued for the MOK manager, for the reason iso/iso.toml gives.
%post --erroronfail
set -e
bootc switch --mutate-in-place --enforce-container-sigpolicy --transport registry @IMAGE@
mkdir -p /var/lib/amethystora && touch /var/lib/amethystora/new-installation
if [ -e /usr/lib/bootc/kargs.d/00-nvidia.toml ] && mokutil --sb-state 2>/dev/null | grep -qi enabled; then
    mokutil --timeout -1 || :
    { echo amethystora; echo amethystora; } | mokutil --import /usr/share/amethystora/secure-boot/*.der || :
fi
%end
