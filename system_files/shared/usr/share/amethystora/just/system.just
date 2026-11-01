# vim: set ft=make :

set default-list

# Rebase assistant: another image or stream, signed images only (IMAGE and STREAM preselect them)
[positional-arguments]
rebase IMAGE="" STREAM="":
    @/usr/bin/amethystora-rollback-helper ${1:+"$1"} ${2:+"$2"}

# What is wrong with this machine outside security, each with the guide that fixes it: failed services, updates, space, memory, crashes, the clock, the driver. Reads only (ID: one check, to see whether a fix worked)
[positional-arguments]
doctor ID="":
    @/usr/libexec/amethystora-doctor ${1:+--only "$1"}

# Devices whose driver is missing, refused or without its firmware, each with what fixes it: as the last check found
# them, or looked at now with sudo
[positional-arguments]
drivers *ARGS:
    @/usr/libexec/amethystora-driver-status "$@"

# Take an image of your own from REPOSITORY only when KEY, its cosign.pub, checks its signature
[no-cd]
[no-exit-message]
[positional-arguments]
trust-image REPOSITORY KEY:
    @sudo /usr/libexec/amethystora-trust-image "$1" "$2"

# Turn automatic updates on or off (ACTION: on or off; without it, you are asked)
[positional-arguments]
auto-updates ACTION="prompt":
    #!/usr/bin/bash
    # shellcheck source=/dev/null
    source gettext.sh
    CURRENT_STATE="disabled"
    if systemctl is-enabled -q rpm-ostreed-automatic.timer; then
        CURRENT_STATE="enabled"
    elif systemctl is-enabled -q uupd.timer; then
        CURRENT_STATE="enabled"
    fi

    if [ "${CURRENT_STATE}" == "enabled" ]; then
        echo "$(gettext "Automatic updates are currently enabled")"
    else
        echo "$(gettext "Automatic updates are currently disabled")"
    fi
    # The labels are what gum shows and prints back, so they are computed once and compared as they are
    # Translators: choices of a menu that turns automatic updates on or off
    ENABLE_LABEL="$(gettext "Enable")"
    # Translators: choices of a menu that turns automatic updates on or off
    DISABLE_LABEL="$(gettext "Disable")"
    # Translators: choices of a menu that turns automatic updates on or off
    CANCEL_LABEL="$(gettext "Cancel")"
    case "${1:-}" in
        on | enable) SELECTED_OPTION="${ENABLE_LABEL}" ;;
        off | disable) SELECTED_OPTION="${DISABLE_LABEL}" ;;
        *) SELECTED_OPTION="$(gum choose --header="$(gettext "Toggle automatic updates?")" "${ENABLE_LABEL}" "${DISABLE_LABEL}" "${CANCEL_LABEL}")" ;;
    esac
    [[ "${SELECTED_OPTION}" == "${CANCEL_LABEL}" || "${SELECTED_OPTION}" == "" ]] && exit 0

    if systemctl cat -- uupd.timer &> /dev/null; then
        TIMER="uupd.timer"
    else
        TIMER="rpm-ostreed-automatic.timer"
    fi
    if [ "${SELECTED_OPTION}" == "${ENABLE_LABEL}" ] ; then
        sudo systemctl enable --now "$TIMER"
        echo "$(gettext "Updates have been enabled")"
    elif [ "${SELECTED_OPTION}" == "${DISABLE_LABEL}" ] ; then
        sudo systemctl disable --now "$TIMER"
        echo "$(gettext "Updates have been disabled")"
    fi

# Tailscale, which joins this machine to your tailnet (ACTION: on or off; without it, says which). Its
# service runs only once it is on, or where the machine had joined a tailnet before; on asks for your
# password, and passes anything after it to `tailscale up`
[positional-arguments]
tailscale ACTION="" *ARGS:
    #!/usr/bin/bash
    # shellcheck source=/dev/null
    source gettext.sh
    case "${1:-}" in
        on | off) sudo /usr/libexec/amethystora-tailscale "$@" ;;
        "")
            state="$(/usr/libexec/amethystora-tailscale status)"
            case "${state}" in
                on) echo "$(gettext "Tailscale is on")" ;;
                off) echo "$(gettext "Tailscale is off")" ;;
                *) echo "Tailscale is ${state}" ;;
            esac
            ;;
        *) echo "Usage: ame system tailscale on|off [options for tailscale up]" >&2; exit 64 ;;
    esac

# Switch between Amethystora and the Developer Experience (ACTION: on or off; without it, it flips)
[positional-arguments]
devmode ACTION="":
    #!/usr/bin/env bash
    # Homebrew's own commands, which a terminal that is not a login shell (one Control opens) does not have
    PATH="${PATH}:/home/linuxbrew/.linuxbrew/bin"
    set -e
    # shellcheck source=/dev/null
    source gettext.sh
    IMAGE_INFO_FILE="${IMAGE_INFO_FILE:-/usr/share/amethystora/image-info.json}"
    IMAGE_NAME="$(jq -rc '."image-name"' "${IMAGE_INFO_FILE}")"
    IMAGE_REF="$(jq -rc '."image-ref"' "${IMAGE_INFO_FILE}" | sed "s/.*ghcr/ghcr/")"
    CURRENT_IMAGE="${IMAGE_REF}:$(jq -rc '."image-tag"' "${IMAGE_INFO_FILE}")"
    IMAGE_BASE_NAME="$(cut -f1 -d\- <<< "${IMAGE_NAME}")"
    set +e

    # amethystora-dx and amethystora-dx-nvidia-open are the developer images
    DX=false
    if grep -q -E -e "-dx(-|$)" <<< "${IMAGE_NAME}" ; then
        DX=true
    fi
    case "${1:-}" in
        on)
            if [[ "${DX}" == true ]]; then
                echo "$(eval_gettext 'Developer mode is already on: this is ${IMAGE_NAME}.')"
                exit 0
            fi
            ;;
        off)
            if [[ "${DX}" != true ]]; then
                echo "$(eval_gettext 'Developer mode is already off: this is ${IMAGE_NAME}.')"
                exit 0
            fi
            ;;
        "") ;;
        *)
            echo "usage: ame system devmode [on|off]"
            exit 64
            ;;
    esac

    # As ame system rebase does: rpm-ostree keeps the packages layered on this machine, which bootc switch
    # would drop, and checks the signature the same way. bootc only where layering is locked, or sealed.
    switch_to() {
        if [[ -e /usr/share/amethystora/sealed ]] || grep -q "^LockLayering=true" /etc/rpm-ostreed.conf; then
            pkexec bootc switch --enforce-container-sigpolicy "$1"
        else
            rpm-ostree rebase "ostree-image-signed:docker://$1"
        fi
    }

    if [[ "${DX}" == true ]] ; then
        echo "$(gettext "This switches the system back to the image without the developer tools, at the next restart.")"
        gum confirm "$(gettext "Would you like to disable developer mode?")" || exit 0
        echo "$(gettext "Rebasing to a non developer image")"
        NEW_IMAGE="$(sed "s/\-dx//" <<< $CURRENT_IMAGE)"
        switch_to "${NEW_IMAGE}" || exit 1
        exit 0
    fi

    echo "$(gettext "This switches the system to the developer image, with VSCodium, Docker and Podman, Incus and virtual machines, at the next restart.")"
    gum confirm "$(gettext "Would you like to enable developer mode?")" || exit 0
    echo "$(gettext "Rebasing to a developer image")"
    NEW_IMAGE="$(sed "s/${IMAGE_BASE_NAME}/${IMAGE_BASE_NAME}-dx/" <<< "${CURRENT_IMAGE}")"
    switch_to "${NEW_IMAGE}" || exit 1
    echo "$(gettext "The development apps install by themselves once the development image has started")"
    if gum confirm "$(gettext "Do you want to install extra monospace fonts?")" ; then
        brew bundle install --file=/usr/share/amethystora/homebrew/fonts.Brewfile
    fi
    echo "$(gettext "After the restart, ame system dx-group lets your account use Docker, Incus and virtual machines without sudo, and says what that costs.")"

# Choose your account's developer groups (Docker, Incus, libvirt, serial devices); three of them amount to root
[positional-arguments]
dx-group *GROUPS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # One list, which Control and the security report read too: group, "root" or "-", what it allows
    list=/usr/share/amethystora/developer-groups
    user="$(id -un)"
    member() { id -nG "${user}" | tr ' ' '\n' | grep -qx "$1"; }
    names=()
    roots=()
    echo "$(eval_gettext 'Developer groups for ${user} (* = in it):')"
    while read -r group root text; do
        # Only the groups this image has: Docker and Incus come with the developer image
        getent group "${group}" >/dev/null || continue
        names+=("${group}")
        [[ "${root}" == root ]] && roots+=("${group}")
        if member "${group}"; then mark="*"; else mark=" "; fi
        echo " ${mark} ${group}: ${text}"
    done < <(grep -v '^#' "${list}" | awk NF)
    if [[ ${#names[@]} -eq 0 ]]; then
        echo "   $(gettext "None: they come with developer mode (ame system devmode on).")"
        exit 0
    fi
    echo
    if [[ ${#roots[@]} -gt 0 ]]; then
        root_groups="${roots[*]}"
        echo "$(eval_gettext 'In ${root_groups}, any program running as you can become root without the sudo password.')"
    fi
    echo "$(gettext "Podman, and virtual machines on your own connection (qemu:///session), need no group at all.")"
    echo
    if [[ $# -gt 0 ]]; then
        wanted=("$@")
        [[ "${wanted[*]}" == none ]] && wanted=()
    else
        selected=""
        for group in "${names[@]}"; do
            if member "${group}"; then selected+="${selected:+,}${group}"; fi
        done
        choice="$(gum choose --no-limit --header "$(eval_gettext 'Which groups should ${user} be in?')" \
            --selected "${selected}" "${names[@]}")" || exit 0
        mapfile -t wanted <<<"${choice}"
    fi
    joins=()
    leaves=()
    for group in "${wanted[@]}"; do
        [[ -n "${group}" ]] || continue
        if [[ " ${names[*]} " != *" ${group} "* ]]; then
            group_list="${names[*]}"
            echo "$(eval_gettext 'Not a developer group here: ${group}. The ones there are: ${group_list}')"
            exit 64
        fi
        member "${group}" || joins+=("${group}")
    done
    for group in "${names[@]}"; do
        member "${group}" && [[ " ${wanted[*]} " != *" ${group} "* ]] && leaves+=("${group}")
    done
    if [[ ${#joins[@]} -eq 0 && ${#leaves[@]} -eq 0 ]]; then
        echo "$(gettext "Nothing to change.")"
        exit 0
    fi
    risky=()
    for group in "${joins[@]}"; do
        [[ " ${roots[*]} " == *" ${group} "* ]] && risky+=("${group}")
    done
    if [[ ${#risky[@]} -gt 0 ]]; then
        risky_groups="${risky[*]}"
        gum confirm "$(eval_gettext 'Join ${risky_groups}, which let any program you run become root without your password?')" || exit 0
    fi
    for group in "${joins[@]}"; do
        # The image's own groups are in /usr/lib/group until something copies them into /etc/group, and a
        # group without its /etc/gshadow line is an error to grpck
        grep -q "^${group}:" /etc/group || grep "^${group}:" /usr/lib/group | sudo tee -a /etc/group >/dev/null
        sudo grep -q "^${group}:" /etc/gshadow || echo "${group}:!::" | sudo tee -a /etc/gshadow >/dev/null
        sudo usermod -aG "${group}" "${user}"
        echo "$(eval_gettext 'Joined ${group}.')"
    done
    for group in "${leaves[@]}"; do
        sudo gpasswd -d "${user}" "${group}" >/dev/null
        echo "$(eval_gettext 'Left ${group}.')"
    done
    echo "$(gettext "Log out and back in for it to apply.")"

# Settings back as the image ships them, your files, apps and accounts kept: your account's, the machine's or both, each saved first (WHAT: account, machine, both, or undo with account or machine; without it, asks)
[no-exit-message]
[positional-arguments]
reset-settings WHAT="" PART="":
    @/usr/libexec/amethystora-reset-settings ask ${1:+"$1"} ${2:+"$2"}

# Factory reset this device (experimental)
powerwash:
    #!/usr/bin/bash
    # shellcheck source=/dev/null
    source gettext.sh
    # bootc's own test of this skips the composefs backend a sealed machine runs on
    if [[ -e /usr/share/amethystora/sealed ]]; then
        echo "$(gettext "Starting over has not been tried on a sealed machine yet. Install it again from the ISO instead.")"
        exit 1
    fi
    echo "$(gettext "To put only the settings back, and keep the accounts and their files: ame system reset-settings")"
    # The labels are what gum shows and prints back, so they are computed once and compared as they are
    NO_LABEL="$(gettext "No")"
    YES_LABEL="$(gettext "Yes - start over")"
    echo "$(gettext "This makes a new installation of this image beside this one, without your accounts, settings or files. Nothing is erased: the old system stays on the disk, and the boot menu can still start it.")"
    FIRST_CONFIRMATION=$(gum choose --header="$(gettext "Warning: This is an experimental feature.")" "${NO_LABEL}" "${YES_LABEL}")
    if [[ "$FIRST_CONFIRMATION" != "${YES_LABEL}" ]]; then
        echo "$(gettext "Powerwash cancelled.")"
        exit 0
    fi
    SECOND_CONFIRMATION=$(gum choose --header="$(gettext "Are you sure?")" "${NO_LABEL}" "${YES_LABEL}")
    if [[ "$SECOND_CONFIRMATION" != "${YES_LABEL}" ]]; then
        echo "$(gettext "Powerwash cancelled.")"
        exit 0
    fi
    sudo bootc install reset --experimental

# Boot into this device's BIOS/UEFI screen
bios:
    #!/usr/bin/bash
    # shellcheck source=/dev/null
    source gettext.sh
    if [ ! -d /sys/firmware/efi ]; then
      echo "$(gettext "Rebooting to legacy BIOS from OS is not supported.")"
      exit 1
    fi

    if gum confirm "$(gettext "Reboot into BIOS?")" ; then
      systemctl reboot --firmware-setup
    fi

# Show BIOS info
bios-info:
    #!/usr/bin/bash
    sudo bash <<'EOF'
    echo "Manufacturer: $(dmidecode -s baseboard-manufacturer)"
    echo "Product Name: $(dmidecode -s baseboard-product-name)"
    echo "Version: $(dmidecode -s bios-version)"
    echo "Release Date: $(dmidecode -s bios-release-date)"
    EOF

# Free disk space: unused container images and volumes, Flatpak runtimes, rpm-ostree's caches, Homebrew's cache
clean:
    #!/usr/bin/bash
    # Homebrew's own commands, which a terminal that is not a login shell (one Control opens) does not have
    PATH="${PATH}:/home/linuxbrew/.linuxbrew/bin"
    # shellcheck source=/dev/null
    source gettext.sh
    # Each thing is shown where its tool can show it, and asked about first
    podman system df
    if gum confirm "$(gettext "Prune all Podman images and volumes?")"; then
        podman image prune -af
        podman volume prune -f
    fi

    if command -v docker >/dev/null 2>&1; then
      docker system df
      if gum confirm "$(gettext "Prune all Docker images and volumes?")"; then
        docker image prune -af
        docker volume prune -f
      fi
    fi

    flatpak uninstall --unused
    # A sealed image has no rpm-ostree, nor its caches to clean
    if command -v rpm-ostree >/dev/null && gum confirm "$(gettext "Clear rpm-ostree's caches of downloaded updates and package lists?")"; then
        rpm-ostree cleanup -bm
    fi
    if [ -x /home/linuxbrew/.linuxbrew/bin/brew ]; then
        brew autoremove --dry-run
        brew cleanup --dry-run
        if gum confirm "$(gettext "Remove what Homebrew lists above?")"; then
            brew autoremove
            brew cleanup
        fi
    fi

# Pools of disks (RAID): how they are, then make one from spare disks, or replace or add a disk
[no-cd]
[no-exit-message]
[positional-arguments]
raid *ARGS:
    @/usr/libexec/amethystora-raid "$@"

# Show all messages from this boot
logs-this-boot:
    sudo journalctl --no-hostname -b 0

# Show all messages from last boot
logs-last-boot:
    sudo journalctl --no-hostname -b -1

# Check for local overrides
local-overrides:
    #!/usr/bin/bash
    RESET="\x1b[0m"
    if [ "${NO_COLOR}" == 1 ]; then
        BLUE_COLOR=""
        GREEN_COLOR=""
        YELLOW_COLOR=""
    else
        BLUE_COLOR="${BLUE_COLOR:-\x1b[38;5;117m}"
        GREEN_COLOR="${GREEN_COLOR:-\x1b[38;5;85m}"
        YELLOW_COLOR="${YELLOW_COLOR:-\x1b[93m}"
    fi

    diff -qr \
      --suppress-common-lines \
      --color="always" \
      --exclude "passwd*" \
      --exclude "group*" \
      --exclude="subgid*" \
      --exclude="subuid*" \
      --exclude="machine-id" \
      --exclude="adjtime" \
      --exclude="fstab" \
      --exclude="system-connections" \
      --exclude="shadow*" \
      --exclude="gshadow*" \
      --exclude="ssh_host*" \
      --exclude="cmdline" \
      --exclude="crypttab" \
      --exclude="hostname" \
      --exclude="localtime" \
      --exclude="locale*" \
      --exclude="*lock" \
      --exclude=".updated" \
      --exclude="*LOCK" \
      --exclude="vconsole*" \
      --exclude="00-keyboard.conf" \
      --exclude="grub" \
      --exclude="system.control*" \
      --exclude="cdi" \
      --exclude="default.target" \
      /usr/etc /etc 2>/dev/null | sed \
          -e '/Binary\ files\ /d ; /Common subdirectories/d' \
          -e "/Files/ s/^/${BLUE_COLOR}&/g" \
          -e "/Files/ s/\$/&${RESET}/g" \
          -e "/Only in/ s/^/${GREEN_COLOR}&/g" \
          -e "/Only in/ s/\$/&${RESET}/g" \
          -e "s/\:.*/${RESET}${YELLOW_COLOR}&${RESET}/g"

# Gathering what a bug report needs is `ame report`, which keeps it on this machine until you choose otherwise
[private]
device-info:
    @ame report

# Measure Idle Power Draw
power-draw:
    #!/usr/bin/bash
    # shellcheck source=/dev/null
    source gettext.sh
    if ! command -v powerstat ; then
        echo "$(gettext "Powerstat is not available")"
        exit 1
    fi
    sudo powerstat -a -r

# Run a one minute system benchmark
benchmark:
    #!/usr/bin/env bash
    # Homebrew's own commands, which a terminal that is not a login shell (one Control opens) does not have
    PATH="${PATH}:/home/linuxbrew/.linuxbrew/bin"
    # shellcheck source=/dev/null
    source gettext.sh
    if ! command -v "stress-ng" ; then
        if gum confirm "$(gettext "Stress does not seem to be on your path, do you wish to install it?")" ; then
            set -eu
            brew install stress-ng
            brew link stress-ng
            set +eu
        else
            exit 0
        fi
    fi

    echo "$(gettext "Running a 1 minute benchmark ...")"
    # stress-ng writes its files where it runs: a folder of its own, gone afterwards
    work="$(mktemp -d)"
    trap 'rm -rf "${work}"' EXIT
    cd "${work}"
    stress-ng --matrix 0 -t 1m --times

# Pick the CPU scheduler: the kernel's own, or a sched-ext one for games (lavd), a busy desktop (bpfland) or audio work (flash)
[positional-arguments]
scheduler NAME="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # scx_loader starts default_sched at boot while it is enabled (17-cleanup.sh ships it disabled)
    config=/etc/scx_loader/config.toml
    # What the kernel itself says runs: "enabled" with the scheduler's name while a sched-ext one does
    running() {
        [[ "$(cat /sys/kernel/sched_ext/state 2>/dev/null)" == enabled ]] && cat /sys/kernel/sched_ext/root/ops 2>/dev/null
    }
    current="$(running || true)"
    if [[ -n "${current}" ]]; then
        echo "$(eval_gettext 'Scheduling the CPU now: ${current}')"
    else
        echo "$(gettext "Scheduling the CPU now: the kernel's own (EEVDF)")"
    fi
    name="${1:-}"
    if [[ -z "${name}" ]]; then
        echo
        echo "kernel   $(gettext "The kernel's own, which suits nearly everything (the default)")"
        echo "lavd     $(gettext "Keeps games smooth while other work runs, made for gaming handhelds and laptops")"
        echo "bpfland  $(gettext "Keeps the desktop responsive while something heavy builds or renders")"
        echo "flash    $(gettext "Steady timing, for audio and music work")"
        name="$(gum choose --header "$(gettext "CPU scheduler")" kernel lavd bpfland flash)" || exit 0
    fi
    case "${name}" in
        kernel | off)
            sudo systemctl disable --now scx_loader.service
            sudo rm -f "${config}"
            echo "$(gettext "The kernel's own scheduler, now and after every restart.")"
            ;;
        lavd | bpfland | flash)
            echo "$(gettext "A sched-ext scheduler is a program the kernel hands its scheduling to. If it stops, the kernel takes over again by itself, so the worst it can do is make things slower.")"
            sudo mkdir -p "${config%/*}"
            printf 'default_sched = "scx_%s"\ndefault_mode = "Auto"\n' "${name}" | sudo tee "${config}" >/dev/null
            sudo systemctl enable scx_loader.service
            sudo systemctl restart scx_loader.service
            # The loader answers before the scheduler is in: give the kernel a moment to say so
            for _ in 1 2 3 4 5 6 7 8 9 10; do
                [[ "$(running || true)" == "${name}"* ]] && break
                sleep 1
            done
            if [[ "$(running || true)" != "${name}"* ]]; then
                echo
                echo "$(eval_gettext 'The kernel did not take scx_${name}. What the loader said:')"
                journalctl -u scx_loader.service -n 15 --no-pager -q || true
                echo
                echo "$(gettext "Back to the kernel's own scheduler.")"
                sudo systemctl disable --now scx_loader.service
                sudo rm -f "${config}"
                exit 1
            fi
            echo "$(eval_gettext "scx_\${name}, now and after every restart. ame system scheduler kernel puts the kernel's own back.")"
            ;;
        *) exit 0 ;;
    esac

# How swapped-out memory is compressed (MODE: default, fast or compact; without it, explains and asks), from the next restart
[positional-arguments]
zram MODE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # The image's own is zram-generator.conf.d/10-amethystora.conf; a choice goes after it, in /etc
    choice=/etc/systemd/zram-generator.conf.d/90-amethystora-choice.conf
    current="$(sed -n 's/^# mode: //p' "${choice}" 2>/dev/null || true)"
    current="${current:-default}"
    echo "$(eval_gettext 'Swapped-out memory is compressed: ${current}')"
    mode="${1:-}"
    if [[ -z "${mode}" ]]; then
        echo
        echo "$(gettext "When RAM runs short, what programs are not using moves into compressed memory, and comes back when a program needs it. Measured on a 4 GB machine holding 3 GB of program data:")"
        echo
        echo "default  $(gettext "Compressed quickly as it leaves RAM, and what stays unused repacked smaller every hour on mains power: about 420 MB of RAM for each GB moved out, back at about 310 MB/s.")"
        echo "fast     $(gettext "Compressed quickly and left so (Fedora's own): nothing runs in the background, but about 540 MB of RAM for each GB, back at about 250 MB/s.")"
        echo "compact  $(gettext "Compressed small from the start: about 430 MB of RAM for each GB, but moving out and back is about half as quick, at about 130 MB/s.")"
        echo
        mode="$(gum choose --header "$(gettext "Swapped-out memory")" default fast compact)" || exit 0
    fi
    case "${mode}" in
        default) sudo rm -f "${choice}" ;;
        fast | compact)
            algorithm=lzo-rle
            [[ "${mode}" == compact ]] && algorithm=zstd
            sudo mkdir -p "${choice%/*}"
            printf '# mode: %s\n[zram0]\ncompression-algorithm = %s\n' "${mode}" "${algorithm}" | sudo tee "${choice}" >/dev/null
            ;;
        *) exit 0 ;;
    esac
    echo "$(eval_gettext '${mode}, from the next restart.')"
