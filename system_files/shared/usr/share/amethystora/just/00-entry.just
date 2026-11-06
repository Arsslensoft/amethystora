# The commands that come with Amethystora, run by `ame`: `ame` lists them, `ame security` lists the
# ones in that group, and `ame security scan now` runs one. A `mod` is a group, in the file of its
# name; an `import` adds commands to the top level. ujust is the same command under its older name,
# and the names the commands had before they were grouped are the private aliases at the end.

set ignore-comments
set default-list

# The AI agent: open it (`ame agent`, alone, as amethystora-agent does), install it, or turn it off
mod agent

# Apps from Flathub and Homebrew, and the Arch User Repository for ame pkg
mod apps

# The theme, the wallpaper and how it changes, widgets, the keybindings, the boot menu and the terminal greeting
mod desktop

# Plugins: add them, read what each asks for, allow or block it, turn it on or off. Each runs in a sandbox and reaches only what you allowed
mod plugin

# What protects this machine and what it found: the report, virus scans, network protection, keys
mod security

# Your setup as a file of your own: save it, compare it, and apply it here or on another machine
mod setup

# The image, its streams and automatic updates, and the hardware
mod system

# The Vault: your passwords, one-time codes, keys and certificates, kept sealed, and the programs that ask for them
mod vault

import "backup.just"
import "changelog.just"
import "control.just"
import "report.just"
import "update.just"

# Packages from other distributions, in containers: ame pkg is amethystora-pkg (also amepkg)
[no-cd]
[no-exit-message]
[positional-arguments]
pkg *ARGS:
    @amethystora-pkg "$@"

# The names under ujust. They still work, with the same arguments, and are left out of the list.

[private]
alias auto-update := system::auto-updates
[private]
alias amethystora-apps := apps::flatpaks
[private]
alias amethystora-cli := apps::cli
[private]
alias background := desktop::background
[private]
alias bbrew := apps::bbrew
[private]
alias benchmark := system::benchmark
[private]
alias bios := system::bios
[private]
alias bios-info := system::bios-info
[private]
alias blocked-addresses := security::addresses
[private]
alias blocked-connections := security::connections
[private]
alias changelogs := changelog
[private]
alias check-idle-power-draw := system::power-draw
[private]
alias check-local-overrides := system::local-overrides
[private]
alias clean-system := system::clean
[private]
alias cncf := apps::cncf
[private]
alias device-info := system::device-info
[private]
alias devmode := system::devmode
[private]
alias dx-group := system::dx-group
[private]
alias enroll-secure-boot-key := security::secure-boot
[private]
alias install-agent := agent::install
[private]
alias install-jetbrains-toolbox := apps::jetbrains-toolbox
[private]
alias install-opentabletdriver := apps::opentabletdriver
[private]
alias install-system-flatpaks := apps::flatpaks
[private]
alias jetbrains-toolbox := apps::jetbrains-toolbox
[private]
alias keybindings := desktop::keybindings
[private]
alias logs-last-boot := system::logs-last-boot
[private]
alias logs-this-boot := system::logs-this-boot
[private]
alias powerwash := system::powerwash
[private]
alias rebase-helper := system::rebase
[private]
alias rollback-helper := system::rebase
[private]
alias security-audit := security::audit
[private]
alias security-events := security::events
[private]
alias security-settings := security::settings
[private]
alias security-status := security::status
[private]
alias setup-backup := backup
[private]
alias setup-browser-protection := security::browser
[private]
alias setup-disk-unlock := security::disk-unlock
[private]
alias setup-grub-theme := desktop::boot-menu
[private]
alias setup-input-remapper := security::input-remapper
[private]
alias setup-ransomware-protection := security::ransomware
[private]
alias setup-security-key := security::key
[private]
alias setup-usb-protection := security::usb
[private]
alias switch-stream := system::rebase
[private]
alias switch-streams := system::rebase
[private]
alias theme := desktop::theme
[private]
alias toggle-agentic := agent::toggle
[private]
alias toggle-aur := apps::aur
[private]
alias toggle-devmode := system::devmode
[private]
alias toggle-tpm2 := security::disk-unlock
[private]
alias toggle-updates := system::auto-updates
[private]
alias toggle-user-motd := desktop::greeting
[private]
alias upgrade := update
[private]
alias virus-scan := security::scan
