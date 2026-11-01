# vim: set ft=make :

# Update system, flatpaks and homebrew packages all at once; restart to finish, in seconds when the kernel stays.
# night-restart off|02:00..05:00 chooses when Restart while you're away restarts, and tonight [off] sets it for tonight.
[positional-arguments]
update ACTION="" VALUE="":
    #!/usr/bin/bash
    # shellcheck source=/dev/null
    source gettext.sh
    # A soft reboot: only the system starts again, not the computer (amethystora-soft-reboot)
    if [[ "$1" == restart && -z "$2" ]]; then
        /usr/libexec/amethystora-soft-reboot restart && exit 0
        status=$?
        [[ ${status} == 1 ]] && echo "$(gettext "To restart as usual: systemctl reboot")" >&2
        exit "${status}"
    elif [[ "$1" == night-restart ]]; then
        # Restart while you're away: the time, between 02:00 and 05:00, or off (amethystora-night-restart)
        if [[ -z "$2" ]]; then
            time="$(/usr/libexec/amethystora-night-restart get)"
            if [[ -n "${time}" ]]; then
                echo "$(eval_gettext "Restart while you're away: at \${time}, from System Updates' Tonight.")"
            else
                echo "$(gettext "Restart while you're away is off.")"
            fi
            exit 0
        fi
        /usr/libexec/amethystora-night-restart set "$2" || exit $?
        if [[ "$2" == off ]]; then
            echo "$(gettext "Restart while you're away is off.")"
        else
            time="$2"
            echo "$(eval_gettext "System Updates offers Tonight at \${time} beside Restart now. It restarts then only if the screen is locked, no app holds the logout and the computer is on mains power.")"
        fi
        exit 0
    elif [[ "$1" == tonight ]]; then
        case "$2" in
            "") exec /usr/libexec/amethystora-night-restart arm ;;
            off)
                /usr/libexec/amethystora-night-restart cancel
                echo "$(gettext "The computer does not restart tonight.")"
                exit 0
                ;;
        esac
        echo "Usage: ame update tonight [off]" >&2
        exit 2
    elif [[ -n "$1" ]]; then
        echo "Usage: ame update [restart|night-restart off|HH:MM|tonight [off]]" >&2
        exit 2
    fi
    if systemctl cat -- uupd.timer &> /dev/null; then
        SERVICE="uupd.service"
    else
        SERVICE="rpm-ostreed-automatic.service"
    fi
    if systemctl is-active --quiet "$SERVICE"; then
        echo "$(eval_gettext "automatic updates are currently running, use \`journalctl -fexu \${SERVICE}\` to see logs")"
        exit 1
    fi
    if grep -q -E -e "LockLayering=false" /etc/rpm-ostreed.conf ; then
        rpm-ostree upgrade
    else
        sudo bootc upgrade
    fi
    # rpm-ostree used due to bootc upgrade not supporting local layered packages
    # Updates system Flatpaks
    if flatpak remotes | grep -q system; then
        flatpak update -y
    fi
    # Update user Flatpaks
    if flatpak remotes | grep -q user; then
        flatpak update --user -y
    fi
    # Guard Brew if the user does not own brew/doesn't exist
    if [[ -O /var/home/linuxbrew/.linuxbrew/bin/brew ]]; then
        # Upgrade will run brew update if needed
        /var/home/linuxbrew/.linuxbrew/bin/brew upgrade
    fi
