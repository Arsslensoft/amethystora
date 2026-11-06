# vim: set ft=make :

# Back up this account to a repository that ransomware cannot delete from, to another of your computers, or start from a backup on a new one
[positional-arguments]
backup ACTION="" *MORE:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/amethystora"
    ENV_FILE="${CONFIG_DIR}/backup.env"
    PASSWORD_FILE="${CONFIG_DIR}/backup-password"
    action="${1:-}"
    # The choices are shown in the session's language, and what gum prints back is matched against the same words
    # Translators: a choice in a menu: to set the backup up
    set_up="$(gettext "Set up")"

    back_up_now="$(gettext "Back up now")"
    show_snapshots="$(gettext "Show snapshots")"
    turn_on="$(gettext "Turn on the daily run")"
    turn_off="$(gettext "Turn off the daily run")"
    recovery_key="$(gettext "Make a recovery key")"
    restore_from="$(gettext "Start from a backup")"
    if [[ -r "${ENV_FILE}" ]]; then
        backing_up_to="$(sed -n 's/^RESTIC_REPOSITORY=//p' "${ENV_FILE}" | sed -E 's#(://)[^/]*@#\1#')"
        echo "$(eval_gettext "Backing up to: \${backing_up_to}")"
        if systemctl --user is-enabled --quiet amethystora-backup.timer 2>/dev/null; then
            systemctl --user list-timers --no-pager amethystora-backup.timer
        else
            echo "$(gettext "The daily run is off.")"
        fi
    else
        echo "$(gettext "No backup is set up for this account.")"
    fi
    if [[ -z "${action}" ]]; then
        daily="${turn_off}"
        systemctl --user is-enabled --quiet amethystora-backup.timer 2>/dev/null || daily="${turn_on}"
        action="$(gum choose --header "$(gettext "Backups (restic)")" \
            "${set_up}" "${back_up_now}" "${show_snapshots}" "${daily}" "${recovery_key}" "${restore_from}" "$(gettext "Cancel")")" || exit 0
    fi
    case "${action}" in
        "${set_up}" | "Set up" | setup)
            echo
            echo "$(gettext "Ransomware encrypts everything it can reach, and a backup this machine can delete from is something it can reach. An append-only repository is one this machine may add snapshots to and read its own back from, and nothing else, so what was backed up before the machine was taken over is still there afterwards. Set that up at the other end:")"
            echo
            echo "  $(gettext "rest-server   run it with --append-only, then use  rest:https://user:pass@host:8000/")"
            echo "  $(gettext "Borg or SSH   set  append_only = yes  in the repository's config on the server")"
            echo "  $(gettext "Object store  turn on object lock or immutability, then use  rclone:remote:path")"
            echo "  $(gettext "A USB drive   unplug it between backups; unplugged is the oldest append-only there is")"
            echo
            current="$(sed -n 's/^RESTIC_REPOSITORY=//p' "${ENV_FILE}" 2>/dev/null || true)"
            repository="$(gum input --header "$(gettext "Repository")" \
                --placeholder "rest:https://user:pass@host:8000/" --value "${current}")" || exit 0
            [[ -n "${repository}" ]] || exit 0
            password="$(gum input --password \
                --header "$(gettext "Repository password (keep a copy elsewhere: without it the backup cannot be read)")")" || exit 0
            [[ -n "${password}" ]] || exit 0
            paths="$(gum input --header "$(gettext "What to back up, separated by spaces")" --value "${HOME}")" || exit 0
            [[ -n "${paths}" ]] || exit 0
            mkdir -p "${CONFIG_DIR}"
            (umask 077 && printf '%s' "${password}" >"${PASSWORD_FILE}")
            # Every value is quoted for the shell (%q): the file is sourced, and a repository or a folder may have spaces or $ in it
            (umask 077 && {
                echo "# Written by ame backup, read by /usr/libexec/amethystora-backup."
                printf '%s=%q\n' RESTIC_REPOSITORY "${repository}" RESTIC_PASSWORD_FILE "${PASSWORD_FILE}" AMETHYSTORA_BACKUP_PATHS "${paths}"
            } >"${ENV_FILE}")
            set -a && source "${ENV_FILE}" && set +a
            # An append-only repository refuses `restic init`, so finding one already there is normal
            if restic cat config >/dev/null 2>&1; then
                echo "$(gettext "Using the repository that is already there.")"
            else
                echo "$(gettext "Nothing there yet, creating the repository.")"
                restic init
            fi
            systemctl --user enable --now amethystora-backup.timer
            echo
            echo "$(gettext "Set up, and running once a day. Files come back from it in Backups, in the app grid.")"
            echo
            echo "$(gettext "Nothing on this machine ever deletes from the repository, which is the point, so old snapshots build up. Thin them out from somewhere else, with credentials this machine does not hold:")"
            echo "  restic -r <repository> forget --keep-daily 14 --keep-weekly 8 --keep-monthly 12 --prune"
            ;;
        "${back_up_now}" | "Back up now" | now)
            systemctl --user start --wait amethystora-backup.service || true
            journalctl --user -u amethystora-backup.service -n 30 --no-pager
            ;;
        "${show_snapshots}" | "Show snapshots" | snapshots)
            # Every point there is to go back to, the backup's and the hourly snapshots', as Backups shows them
            /usr/libexec/amethystora-restore points
            ;;
        "${turn_on}" | "Turn on the daily run" | on)
            if [[ ! -r "${ENV_FILE}" ]]; then
                echo "$(gettext "No backup is set up. Set one up with: ame backup setup")"
                exit 1
            fi
            systemctl --user enable --now amethystora-backup.timer
            echo "$(gettext "On, once a day.")"
            ;;
        "${turn_off}" | "Turn off the daily run" | off)
            systemctl --user disable --now amethystora-backup.timer
            echo "$(gettext "Off. The settings and the repository itself are left alone.")"
            ;;
        "${recovery_key}" | "Make a recovery key" | recovery-key)
            # A second key of the repository, shown once: on a new computer it is all the backup needs (amethystora-backup-recovery)
            exec /usr/libexec/amethystora-backup-recovery
            ;;
        other-computer)
            # The daily backup to another computer of yours at home (R7-35): `ame backup other-computer [setup|status|now|on|off|forget]`
            exec /usr/libexec/amethystora-backup-other computer "${2:-status}" "${@:3}"
            ;;
        store)
            # This computer as the other one's store: `ame backup store [on|off|pair|status|clients|forget FINGERPRINT]`
            exec /usr/libexec/amethystora-backup-other store "${2:-status}" "${@:3}"
            ;;
        "${restore_from}" | "Start from a backup" | restore-from)
            # On a new computer, or after the old one was lost: the files, the setup and the Vault from a backup, with only the recovery key
            exec /usr/libexec/amethystora-start-from-backup
            ;;
        *) exit 0 ;;
    esac

# Make this account's setup again from a backup, on a new machine: the image, apps, containers, theme
[positional-arguments]
restore-setup SNAPSHOT="":
    @/usr/libexec/amethystora-restore-setup ${1:+"$1"}
