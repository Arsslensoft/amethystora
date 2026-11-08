# What this machine sends

A few things contact the outside by themselves, outside the [settings](security-settings.md) you
chose. Most of them are how the machine stays updated. Nothing here is turned off for you.

`ame security sends`, and **What this machine sends by itself** in the report and in **Security**, list the
ones that apply on yours, with what each sends and to whom, and what stops working without it.

| What | Sent to | When | Switch |
|---|---|---|---|
| **Fedora's count of its machines** | Fedora's mirror list | Once a week at most | `ame security countme` |
| **The connectivity check** | `fedoraproject.org/static/hotspot.txt`, plain HTTP | At each connection, then every five minutes | `ame security connectivity-check` |
| **Location services** | BeaconDB, when an app asks | When an app asks | `ame security location`, for your account |
| **Virus signatures** and the other downloads of [Settings](security-settings.md) | The feeds' own hosts | Daily, when on | [Settings](security-settings.md) |
| **Automatic updates** | `ghcr.io`, Flathub, Homebrew, your containers' registries | Daily | `ame system auto-updates` |
| **Flatpak installs and updates** | Flathub | Each install and update | None |
| **The firmware catalogue** | LVFS | Once a day at most | `ame security firmware-metadata` |
| **Firmware sharing** | Machines on your network | When firmware is looked for | `ame security network-daemons` |
| **The clock** | `2.fedora.pool.ntp.org`, and the servers each network names | Continuously | `ame security time` |

`ame security countme`, `connectivity-check` and `firmware-metadata` say what they change, ask, and are
undone by the same command with `on`.

### Fedora's count

It says that this is Amethystora, which Fedora version and image it runs (the `NAME`, `VERSION_ID` and
`VARIANT_ID` of `/etc/os-release`) and a bucket of how many weeks the machine has been counted: 1 for the
first, 2 for weeks two to four, 3 and 4 after. No identifier is sent, but Fedora sees the address. It is how
Fedora knows how many machines run its packages; off, it counts one fewer and nothing on this machine
changes. Sealed images do not send it.

### The connectivity check

It is plain HTTP, so every network on the way sees it, and Fedora sees the address every five minutes. Off,
the login page of a hotel or train network is not offered by itself (open any web page to reach it), and
GNOME does not warn that a network has no internet. GNOME's own switch, in **Settings**, **Privacy**,
**Connectivity**, does the same; the report shows what NetworkManager does.

### Location

Asked for in the first-run setup. With it on, an app that asks where you are sends the Wi-Fi networks in
reach and their signal to BeaconDB, and the address alone when none is. Nothing is sent back to improve its
map.

### Flatpak and firmware

Flatpak tells Flathub which app is installed or updated, the version it is replacing and the system as
`amethystora;44;x86_64`. The last is left out by an empty file, `/etc/flatpak/no-report-os-info`.

The firmware catalogue is fetched at most once a day and says nothing about this machine; off,
[Updates](updates.md#firmware) offers no firmware.

### The Vault

The Vault sends nothing by itself. Only if you connect a Bitwarden or Vaultwarden server (its address is
then a row of `ame security sends`) or turn on its service logos does it contact anything, and each says
what it sends first ([Sync](vault-sync.md#sync), [Service logos](vault-items.md#service-logos)).

### Everything else

Homebrew's analytics are off for every account, and Tailscale talks to its servers and relays only where
you turned it on. Firefox has its own settings for what it sends, under **Privacy & Security**.
