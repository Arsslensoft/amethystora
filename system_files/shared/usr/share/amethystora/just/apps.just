# vim: set ft=make :

set default-list

# Install JetBrains Toolbox into your home, from JetBrains, checked against the checksum JetBrains publishes
jetbrains-toolbox:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    # A folder of its own, which it updates itself in
    DEST="${DATA}/jetbrains-toolbox"
    if [[ -x "${DEST}/bin/jetbrains-toolbox" ]]; then
        echo "$(eval_gettext "JetBrains Toolbox is installed in \${DEST}, and keeps itself up to date.")"
        gum confirm --default=false "$(gettext "Install it again?")" || exit 0
    else
        echo "$(eval_gettext "JetBrains Toolbox installs and updates JetBrains' IDEs. This downloads it from JetBrains into \${DEST}, and opens it.")"
        gum confirm "$(gettext "Install JetBrains Toolbox?")" || exit 0
    fi
    case "$(uname -m)" in
        x86_64) platform=linux ;;
        aarch64) platform=linuxARM64 ;;
        *) architecture="$(uname -m)"; echo "$(eval_gettext "JetBrains makes no Toolbox for \${architecture}.")"; exit 1 ;;
    esac
    # JetBrains' release feed, the one its download page reads
    release="$(curl -fsSL "https://data.services.jetbrains.com/products/releases?code=TBA&latest=true&type=release")"
    link="$(jq -er --arg p "${platform}" '.TBA[0].downloads[$p].link' <<<"${release}")"
    checksum="$(curl -fsSL "$(jq -er --arg p "${platform}" '.TBA[0].downloads[$p].checksumLink' <<<"${release}")")"
    TMP="$(mktemp -d)"
    trap 'rm -rf "${TMP}"' EXIT
    version="$(jq -r '.TBA[0].version' <<<"${release}")"
    echo "$(eval_gettext "Downloading JetBrains Toolbox \${version} from JetBrains")"
    curl -fL --progress-bar -o "${TMP}/toolbox.tar.gz" "${link}"
    if [[ "$(sha256sum "${TMP}/toolbox.tar.gz" | cut -d" " -f1)" != "${checksum%% *}" ]]; then
        echo "$(gettext "The download does not match the checksum JetBrains publishes for it. Nothing was installed.")"
        exit 1
    fi
    mkdir -p "${TMP}/toolbox"
    tar -xzf "${TMP}/toolbox.tar.gz" --strip-components=1 -C "${TMP}/toolbox"
    rm -rf "${DEST}"
    mv "${TMP}/toolbox" "${DEST}"
    mkdir -p "${HOME}/.local/bin" "${DATA}/applications"
    ln -sf "${DEST}/bin/jetbrains-toolbox" "${HOME}/.local/bin/jetbrains-toolbox"
    # Toolbox writes a launcher of its own when it first runs; this one shows it in the app grid until then
    if [[ -f "${DEST}/bin/jetbrains-toolbox.desktop" ]]; then
        sed -e "s|^Exec=.*|Exec=${DEST}/bin/jetbrains-toolbox %u|" -e "s|^Icon=.*|Icon=${DEST}/bin/toolbox-tray-color.png|" \
            "${DEST}/bin/jetbrains-toolbox.desktop" >"${DATA}/applications/jetbrains-toolbox.desktop"
    fi
    setsid -f "${DEST}/bin/jetbrains-toolbox" >/dev/null 2>&1
    echo "$(eval_gettext "Installed in \${DEST}. It opens now, and is in the app grid as JetBrains Toolbox.")"

# Games: Steam from Flathub with ProtonPlus, the MangoHud overlay and gamescope, and X11 granted back to Steam alone
[positional-arguments]
gaming ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    STEAM=com.valvesoftware.Steam
    # Launchers for other stores, offered beside Steam. Each draws through X11 too. The names are what gum shows
    # and what it prints back, so they are looked up by the words shown
    declare -A LAUNCHERS=(
        ["$(gettext "Heroic (Epic, GOG and Amazon games)")"]=com.heroicgameslauncher.hgl
        ["$(gettext "Lutris (games from anywhere else)")"]=net.lutris.Lutris
    )
    # The choices are shown in the session's language, and what gum prints back is matched against the same words
    # Translators: a choice in a menu: to set gaming up
    set_up="$(gettext "Set up")"

    add_launcher="$(gettext "Add another launcher")"
    take_away="$(gettext "Take it all away")"
    action="${1:-}"
    if flatpak info "${STEAM}" >/dev/null 2>&1; then
        echo "$(gettext "Gaming is set up: Steam is installed.")"
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Games")" "${add_launcher}" "${take_away}" "$(gettext "Cancel")")"
    else
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Games")" "${set_up}" "$(gettext "Cancel")")"
    fi
    case "${action}" in
        "${set_up}" | "Set up" | on)
            echo "$(gettext "This installs from Flathub Steam, ProtonPlus, which adds other versions of Proton, and the MangoHud overlay and gamescope compositor that Steam's games can use.")"
            echo
            echo "$(gettext "The image takes X11 from every app, because an app on X11 can read what you type into other X11 apps. Steam and the Windows games it runs through Proton draw on nothing else, so X11 is granted back to Steam alone, for your account. The Security app's Apps page lists it, and \`ame apps gaming off\` takes it back with the rest.")"
            echo
            echo "$(gettext "Steam Input, which lets every controller work in every game, makes a virtual controller through /dev/uinput, which the image keeps from apps: a virtual keyboard can type into any window. This gives it back to whoever sits at the machine (ame security virtual-input), and so to every app that may reach all of /dev, the Chromium browsers among them.")"
            gum confirm "$(gettext "Set up gaming?")" || exit 0
            sudo /usr/libexec/amethystora-hardening set virtual-input off
            flatpak install --system -y flathub "${STEAM}" com.vysp3r.ProtonPlus
            # The overlay and the compositor are Vulkan layers of Steam's runtime, so of its branch
            branch="$(flatpak info --show-runtime "${STEAM}" | cut -d/ -f3)"
            flatpak install --system -y flathub "org.freedesktop.Platform.VulkanLayer.MangoHud//${branch}" \
                "org.freedesktop.Platform.VulkanLayer.gamescope//${branch}"
            flatpak override --user --socket=x11 "${STEAM}"
            mapfile -t chosen < <(printf '%s\n' "${!LAUNCHERS[@]}" | sort |
                gum choose --no-limit --header "$(gettext "Also install? (space to pick, enter when done)")" || true)
            for name in "${chosen[@]}"; do
                [[ -n "${name}" ]] || continue
                flatpak install --system -y flathub "${LAUNCHERS[${name}]}"
                flatpak override --user --socket=x11 "${LAUNCHERS[${name}]}"
            done
            echo
            echo "$(gettext "Ready. Open Steam from the app grid. In a game's Properties, under Launch Options:")"
            echo "  mangohud %command%                $(gettext "the overlay with frame rate and temperatures")"
            echo "  gamemoderun %command%             $(gettext "the CPU's performance mode while the game runs")"
            echo "  gamescope -f -- %command%         $(gettext "the game in gamescope, full screen")"
            ;;
        "${add_launcher}" | "Add another launcher" | launchers)
            mapfile -t chosen < <(printf '%s\n' "${!LAUNCHERS[@]}" | sort | gum choose --no-limit --header "$(gettext "Install which?")" || true)
            for name in "${chosen[@]}"; do
                [[ -n "${name}" ]] || continue
                flatpak install --system -y flathub "${LAUNCHERS[${name}]}"
                flatpak override --user --socket=x11 "${LAUNCHERS[${name}]}"
            done
            ;;
        "${take_away}" | "Take it all away" | off)
            gum confirm --default=false "$(gettext "Uninstall Steam, ProtonPlus and the launchers added with them, with their games?")" || exit 0
            for app in "${STEAM}" com.vysp3r.ProtonPlus "${LAUNCHERS[@]}"; do
                flatpak override --user --reset "${app}" 2>/dev/null || true
                if flatpak info "${app}" >/dev/null 2>&1; then
                    flatpak uninstall --system -y --delete-data "${app}"
                fi
            done
            # The Vulkan layers are left to flatpak's own clean-up of what nothing uses
            flatpak uninstall --system -y --unused
            # Virtual keyboards go back to root's, unless the tablet driver still needs them
            if [[ ! -e /etc/udev/rules.d/71-opentabletdriver.rules ]]; then
                sudo /usr/libexec/amethystora-hardening set virtual-input on
            fi
            echo "$(gettext "Done. X11 is taken back from them with their overrides, and /dev/uinput at the next restart.")"
            ;;
        *) exit 0 ;;
    esac

# Android apps, in Waydroid: set it up, start it, stop it, or take it all away again
[positional-arguments]
android ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    # The choices are shown in the session's language, and what gum prints back is matched against the same words
    # Translators: a choice in a menu: to set Android apps up
    set_up="$(gettext "Set up")"

    # Translators: a choice in a menu: to start Android
    start_label="$(gettext "Start")"

    # Translators: a choice in a menu: to stop Android
    stop_label="$(gettext "Stop")"

    take_away="$(gettext "Take it all away")"
    # Waydroid's container opens these to every account (chmod 777) and leaves them so: back to what udev gives them
    close_devices() {
        sudo chmod 0666 /dev/dri/renderD* 2>/dev/null || true
        sudo chmod 0660 /dev/dma_heap/system 2>/dev/null || true
    }
    # Written by `waydroid init`, so there whenever Android has been set up
    if sudo test -e /var/lib/waydroid/waydroid.cfg; then
        container_state="$(systemctl is-active waydroid-container.service || true)"
        echo "$(eval_gettext "Android is set up. Its container is \${container_state}.")"
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Android apps (Waydroid)")" "${start_label}" "${stop_label}" "${take_away}" "$(gettext "Cancel")")"
    else
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Android apps (Waydroid)")" "${set_up}" "$(gettext "Cancel")")"
    fi
    case "${action}" in
        "${set_up}" | "Set up" | on)
            echo "$(gettext "Waydroid runs Android, LineageOS 20, in a container beside your desktop, and the Android apps you install appear in the app grid. Setting it up downloads Android from Waydroid's servers, about a gigabyte, without Google's apps: get apps from F-Droid, or install an .apk with waydroid app install <file>. Its container service runs as root while it is on.")"
            nvidia=false
            if [[ "$(jq -r '."image-flavor" // ""' /usr/share/amethystora/image-info.json 2>/dev/null)" == nvidia* ]]; then
                nvidia=true
                echo
                echo "$(gettext "Waydroid cannot draw with NVIDIA's driver, so on this image Android draws in software: fine for most apps, slow for games.")"
            fi
            gum confirm "$(gettext "Set up Android apps?")" || exit 0
            sudo systemctl enable --now waydroid-container.service
            # Fedora's package leaves the download channels empty, so they are named here
            sudo waydroid init -c https://ota.waydro.id/system -v https://ota.waydro.id/vendor
            # What waydroid init writes has to carry the labels its own SELinux policy expects
            sudo restorecon -R /var/lib/waydroid
            if [[ "${nvidia}" == true ]]; then
                sudo sed -i '/^\[properties\]/a ro.hardware.gralloc=default\nro.hardware.egl=swiftshader' /var/lib/waydroid/waydroid.cfg
                sudo waydroid upgrade -o
            fi
            echo
            echo "$(gettext "Ready. Open Waydroid from the app grid; the first start takes a minute.")"
            ;;
        "${start_label}" | "Start" | start)
            sudo systemctl start waydroid-container.service
            setsid -f waydroid show-full-ui >/dev/null 2>&1
            ;;
        "${stop_label}" | "Stop" | stop)
            waydroid session stop 2>/dev/null || true
            sudo systemctl stop waydroid-container.service
            close_devices
            echo "$(gettext "Stopped. It starts again with ame apps android start.")"
            ;;
        "${take_away}" | "Take it all away" | off)
            gum confirm --default=false "$(gettext "Remove Android, with every app and everything saved in it?")" || exit 0
            waydroid session stop 2>/dev/null || true
            sudo systemctl disable --now waydroid-container.service
            close_devices
            sudo rm -rf /var/lib/waydroid
            # What Android saved in the home belongs to the container's own users, not to you
            sudo rm -rf -- "${XDG_DATA_HOME:-${HOME}/.local/share}/waydroid"
            rm -f "${XDG_DATA_HOME:-${HOME}/.local/share}"/applications/waydroid.*.desktop
            echo "$(gettext "Removed. Waydroid itself stays in the image, set up only when you ask.")"
            ;;
        *) exit 0 ;;
    esac

# Drawing tablets: OpenTabletDriver from Flathub, with the udev rules of its pinned release (ACTION: on or off)
[positional-arguments]
opentabletdriver ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    APP=net.opentabletdriver.OpenTabletDriver
    RULES=/etc/udev/rules.d/71-opentabletdriver.rules
    BLACKLIST=/etc/modprobe.d/blacklist-opentabletdriver.conf
    # The image's own copy of the daemon's user unit (/usr/lib/systemd/user), off until this turns it on
    UNIT=amethystora-opentabletdriver.service
    # The release the udev rules come from, and the SHA-256 of its tarball, both bumped by Renovate
    # (.github/renovate.json5): nothing is copied into /etc from a download that does not match
    OTD_TAG="v0.6.8"
    OTD_SHA256="57255c83a84e4ca45b10dbf96f9e77951779131e9bd1e5e7e6fe1f233ee1e6bf"
    action="${1:-}"
    # The choices are shown in the session's language, and what gum prints back is matched against the same words
    # Translators: a choice in a menu: to set the drawing tablet driver up
    set_up="$(gettext "Set up")"

    # Translators: a choice in a menu: to uninstall the drawing tablet driver
    take_away="$(gettext "Take it away")"

    if flatpak info "${APP}" >/dev/null 2>&1 || [[ -e "${RULES}" ]]; then
        echo "$(gettext "OpenTabletDriver is set up.")"
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Drawing tablets (OpenTabletDriver)")" "${take_away}" "$(gettext "Cancel")")"
    else
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Drawing tablets (OpenTabletDriver)")" "${set_up}" "$(gettext "Cancel")")"
    fi
    case "${action}" in
        "${set_up}" | "Set up" | on)
            echo "$(eval_gettext "OpenTabletDriver drives most drawing tablets, Wacom, Huion, XP-Pen, Gaomon and many more, the same way on every one: pressure, the pen's buttons, and which part of the screen the tablet covers. This installs it from Flathub, and copies its udev rules from its \${OTD_TAG} release into /etc so that it can reach the tablet. It turns off the kernel's own tablet drivers (wacom, hid_uclogic), which would take the tablet first: if one already has, restart.")"
            echo
            echo "$(gettext "It moves the pointer through /dev/uinput, which the image keeps from apps: a virtual keyboard can type into any window. This gives it back to whoever sits at the machine (ame security virtual-input), and so to every app that may reach all of /dev, the Chromium browsers among them.")"
            gum confirm "$(gettext "Set up OpenTabletDriver?")" || exit 0
            sudo /usr/libexec/amethystora-hardening set virtual-input off
            TMP="$(mktemp -d)"
            trap 'rm -rf "${TMP}"' EXIT
            url="https://github.com/OpenTabletDriver/OpenTabletDriver/releases/download/${OTD_TAG}/opentabletdriver-${OTD_TAG#v}_linux-x64_binary.tar.gz"
            curl -fL --progress-bar -o "${TMP}/otd.tar.gz" "${url}"
            if [[ "$(sha256sum "${TMP}/otd.tar.gz" | cut -d" " -f1)" != "${OTD_SHA256}" ]]; then
                echo "$(eval_gettext "The download does not match the checksum pinned for \${OTD_TAG}. Nothing was installed.")"
                exit 1
            fi
            tar -xzf "${TMP}/otd.tar.gz" -C "${TMP}" --strip-components=1 \
                opentabletdriver/etc/udev/rules.d/70-opentabletdriver.rules
            # https://opentabletdriver.net/Wiki/Documentation/RequiredPermissions
            sudo install -m 0644 "${TMP}/etc/udev/rules.d/70-opentabletdriver.rules" "${RULES}"
            printf 'blacklist hid_uclogic\nblacklist wacom\n' | sudo tee "${BLACKLIST}" >/dev/null
            sudo udevadm control --reload-rules
            flatpak install --system -y flathub "${APP}"
            # An earlier recipe downloaded a unit of its own into the account; the image's takes its place
            if [[ -e "${HOME}/.config/systemd/user/opentabletdriver.service" ]]; then
                systemctl --user disable --now opentabletdriver.service 2>/dev/null || true
                rm -f "${HOME}/.config/systemd/user/opentabletdriver.service"
            fi
            systemctl --user daemon-reload
            systemctl --user enable --now "${UNIT}"
            echo
            echo "$(gettext "Ready. Plug the tablet in, and set it up in OpenTabletDriver, in the app grid.")"
            ;;
        "${take_away}" | "Take it away" | off)
            gum confirm --default=false "$(gettext "Uninstall OpenTabletDriver, with its settings and its udev rules?")" || exit 0
            # With the blacklist left behind, the kernel's own tablet drivers would stay off
            sudo rm -f "${BLACKLIST}" "${RULES}"
            sudo udevadm control --reload-rules
            # Virtual keyboards go back to root's, unless Steam Input still needs them
            if ! flatpak info --system com.valvesoftware.Steam >/dev/null 2>&1; then
                sudo /usr/libexec/amethystora-hardening set virtual-input on
            fi
            systemctl --user disable --now "${UNIT}" 2>/dev/null || true
            systemctl --user disable --now opentabletdriver.service 2>/dev/null || true
            rm -f "${HOME}/.config/systemd/user/opentabletdriver.service"
            systemctl --user daemon-reload
            if flatpak info "${APP}" >/dev/null 2>&1; then
                flatpak uninstall --system -y --delete-data "${APP}"
            fi
            echo "$(gettext "Uninstalled. Restart to let the kernel's own tablet drivers load again.")"
            ;;
        *) exit 0 ;;
    esac

# Microsoft's wireless adapter for Xbox controllers (ACTION: on or off; without it, you are asked). Its driver is in
# the image, and its firmware, which the image may not pass on, is downloaded from Microsoft. Wired and Bluetooth
# controllers need neither.
[positional-arguments]
xbox-adapter ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # One line an adapter: product ID, driver package, its SHA-256, the file in it, that file's SHA-256
    PINS=/usr/share/amethystora/xone/firmware
    # Read by the kernel before the image's own firmware, so it is root's, and the security watcher takes no file
    # in it but the ones pinned above (tmpfiles.d/amethystora-firmware.conf)
    FIRMWARE=/var/lib/amethystora/firmware
    KARG="firmware_class.path=${FIRMWARE}"
    BLACKLIST=/etc/modprobe.d/xone.conf
    action="${1:-}"
    # The choices are shown in the session's language, and what gum prints back is matched against the same words
    # Translators: a choice in a menu: to set the wireless adapter up
    set_up="$(gettext "Set up")"

    # Translators: a choice in a menu: to uninstall the wireless adapter's firmware
    take_away="$(gettext "Take it away")"

    if [[ -e "${BLACKLIST}" ]] || compgen -G "${FIRMWARE}/xone_dongle_*.bin" >/dev/null; then
        echo "$(gettext "The Xbox wireless adapter is set up.")"
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Xbox wireless adapter")" "${take_away}" "$(gettext "Cancel")")"
    else
        [[ -n "${action}" ]] || action="$(gum choose --header "$(gettext "Xbox wireless adapter")" "${set_up}" "$(gettext "Cancel")")"
    fi
    case "${action}" in
        "${set_up}" | "Set up" | on)
            if [[ -e /usr/share/amethystora/sealed ]]; then
                echo "$(gettext "This machine is sealed: its kernel arguments are inside the signed kernel image, and firmware kept outside the image would not be covered by its seal. The adapter cannot be set up here; Xbox controllers still work over Bluetooth or a USB cable.")"
                exit 1
            fi
            echo "$(eval_gettext "Microsoft's wireless adapter for Xbox controllers, and the one some laptops have built in, needs Microsoft's firmware, which the image may not pass on. This downloads Microsoft's driver packages from its update catalog, checks each against the checksum pinned in the image, and keeps the firmware in \${FIRMWARE}, where the kernel looks first from the next restart. The firmware is subject to Microsoft's Terms of Use: https://www.microsoft.com/en-us/legal/terms-of-use")"
            echo
            echo "$(gettext "The adapter is a MediaTek MT7612U chip, and Linux's Wi-Fi driver for that chip (mt76x2u) would take it first, so this keeps that driver from loading: a USB Wi-Fi stick with the same chip (Netgear A6210 and many AC1200 sticks) stops working until ame apps xbox-adapter off.")"
            gum confirm "$(gettext "Set up the Xbox wireless adapter?")" || exit 0
            TMP="$(mktemp -d)"
            trap 'rm -rf "${TMP}"' EXIT
            while read -r id url package file sum; do
                curl -fL --progress-bar -o "${TMP}/${id}.cab" "${url}"
                if [[ "$(sha256sum "${TMP}/${id}.cab" | cut -d" " -f1)" != "${package}" ]]; then
                    echo "$(eval_gettext "Microsoft's driver package for adapter \${id} does not match its pinned checksum. Nothing was installed.")"
                    exit 1
                fi
                cabextract -q -F "${file}" -d "${TMP}/${id}" "${TMP}/${id}.cab"
                if [[ "$(sha256sum "${TMP}/${id}/${file}" | cut -d" " -f1)" != "${sum}" ]]; then
                    echo "$(gettext "The firmware in it does not match its pinned checksum. Nothing was installed.")"
                    exit 1
                fi
                mv "${TMP}/${id}/${file}" "${TMP}/xone_dongle_${id}.bin"
            done < <(grep -v '^#' "${PINS}")
            sudo install -d -m 0755 "${FIRMWARE}"
            sudo install -m 0644 "${TMP}"/xone_dongle_*.bin "${FIRMWARE}/"
            sudo restorecon -R "${FIRMWARE}" 2>/dev/null || true
            sudo install -m 0644 /usr/share/amethystora/xone/xone.conf "${BLACKLIST}"
            sudo rpm-ostree kargs --append-if-missing="${KARG}" >/dev/null
            echo
            echo "$(gettext "Ready from the next restart: restart, then plug the adapter in and pair a controller with it.")"
            ;;
        "${take_away}" | "Take it away" | off)
            sudo rm -f "${FIRMWARE}"/xone_dongle_*.bin "${BLACKLIST}"
            [[ -e /usr/share/amethystora/sealed ]] || sudo rpm-ostree kargs --delete-if-present="${KARG}" >/dev/null
            echo "$(gettext "Taken away, from the next restart: MT7612U Wi-Fi sticks work again then.")"
            ;;
        *) exit 0 ;;
    esac

# Install applications from curated Brewfiles (includes full-desktop.Brewfile with GNOME Circle apps)
bbrew:
    #!/usr/bin/env bash
    # shellcheck source=/dev/null
    source gettext.sh
    # Homebrew's own commands, which a terminal that is not a login shell (one Control opens) does not have
    PATH="${PATH}:/home/linuxbrew/.linuxbrew/bin"
    if ! command -v bbrew &>/dev/null ; then
        if gum confirm "$(gettext "Bold Brew is not installed, would you like to install it?")" ; then
            set -e
            brew install Valkyrie00/homebrew-bbrew/bbrew
            set +e
        else
            exit 0
        fi
    fi
    echo "$(gettext "Note: Some of these take a while to load, we'll keep working on it.")"
    bbrew -f "/usr/share/amethystora/homebrew/$(gum choose $(find "/usr/share/amethystora/homebrew"/*.Brewfile -exec basename '{}' '.Brewfile' ';')).Brewfile"

# Install CNCF projects from a curated Brewfile
cncf:
    #!/usr/bin/env bash
    # shellcheck source=/dev/null
    source gettext.sh
    # Homebrew's own commands, which a terminal that is not a login shell (one Control opens) does not have
    PATH="${PATH}:/home/linuxbrew/.linuxbrew/bin"
    if ! command -v bbrew &>/dev/null ; then
        if gum confirm "$(gettext "Bold Brew is not installed, would you like to install it?")" ; then
            brew install Valkyrie00/homebrew-bbrew/bbrew
        else
            exit 0
        fi
    fi
    bbrew -f "/usr/share/amethystora/homebrew/cncf.Brewfile"

# Install the apps this image comes with now, instead of waiting for flatpak-preinstall.service
flatpaks:
    flatpak preinstall

# Configure Amethystora-CLI Terminal Experience with Brew
[no-exit-message]
cli:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    brewfile=/usr/share/amethystora/homebrew/cli.Brewfile
    echo "$(gettext "This turns the terminal's extras on in your shell's settings, and installs these from Homebrew:")"
    sed -n 's/^brew "\(.*\)"$/\1/p' "${brewfile}" | paste -sd' ' | fold -s -w 78
    gum confirm "$(gettext "Install the command line tools?")" || exit 0
    amethystora-bling on
    PATH="${PATH}:/home/linuxbrew/.linuxbrew/bin" brew bundle --file="${brewfile}"

# Sites in windows of their own, with a launcher in the app grid: list, install, remove (amethystora-webapp)
[no-cd]
[no-exit-message]
[positional-arguments]
webapps *ARGS:
    @amethystora-webapp "$@"

# Let amethystora-pkg build packages from the Arch User Repository, or stop it again (ACTION: on or off; off by default, and without it, it flips)
[positional-arguments]
aur ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # Read by amethystora-pkg, which shows its paru manager and arch-aur template only while it is there
    FLAG="${XDG_CONFIG_HOME:-$HOME/.config}/amethystora/aur"
    action="${1:-}"
    if [[ -z "${action}" ]]; then
        if [[ -e "${FLAG}" ]]; then action=off; else action=on; fi
    fi
    case "${action}" in
        on)
            if [[ -e "${FLAG}" ]]; then
                echo "$(gettext "The AUR is already on.")"
                exit 0
            fi
            ;;
        off)
            rm -f "${FLAG}"
            echo "$(gettext "The AUR is off. Containers made with it keep what they have, and what came from Arch's own repositories in them is still upgraded every day. Nothing more is built from the AUR.")"
            exit 0
            ;;
        *)
            echo "usage: ame apps aur [on|off]"
            exit 64
            ;;
    esac
    echo "$(gettext "The Arch User Repository (AUR) is build scripts, PKGBUILDs, that anyone can upload and nobody reviews, and malware has been found in it more than once. What you build from it runs as you, with your files, your display and your session, as anything in a container does.")"
    echo
    echo "$(gettext "Turned on, amethystora-pkg offers the arch-aur template, whose containers keep their settings in a home folder of their own, and the paru manager. Before anything is built it shows what the AUR knows of it: who maintains it, how many vote for it, how old it is, whether it is flagged out of date. paru then shows you the PKGBUILD itself: read it. Packages from the AUR are never upgraded unattended.")"
    gum confirm --default=false "$(gettext "Turn the AUR on?")" || exit 0
    mkdir -p "$(dirname "${FLAG}")"
    touch "${FLAG}"
    echo "$(gettext "On. Make a container for it with: amepkg containers new --template arch-aur")"
