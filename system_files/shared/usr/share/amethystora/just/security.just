# vim: set ft=make :

set default-list

# Log in, unlock the screen and approve sudo with a FIDO2 security key (YubiKey, Thetis, and their fingerprint models)
[positional-arguments]
key ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # Keys are bound to the origin set in /etc/security/pam_u2f.conf, not to the hostname
    origin="pam://amethystora"
    keys="${HOME}/.config/Yubico/u2f_keys"
    action="${1:-}"
    add_key="$(gettext "Add a key")"
    add_fingerprint="$(gettext "Add a fingerprint key")"
    remove_keys="$(gettext "Remove all keys")"
    cancel="$(gettext "Cancel")"
    if [[ -z "${action}" ]]; then
        registered="$(awk -F: -v user="${USER}" '$1 == user { print NF - 1 }' "${keys}" 2>/dev/null || true)"
        count="${registered:-0}"
        echo "$(eval_gettext "Security keys registered for \${USER}: \${count}")"
        action="$(gum choose --header "$(gettext "Security key login")" "${add_key}" "${add_fingerprint}" "${remove_keys}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${add_key}" | add)
            flags=()
            ;;
        "${add_fingerprint}" | add-fingerprint)
            # The key checks the fingerprint itself (FIDO2 user verification), a touch alone is not enough
            flags=(--user-verification)
            echo "$(gettext "The key needs a PIN and an enrolled fingerprint. Enroll one in Firefox at about:webauthn, or on a YubiKey Bio with: ykman fido fingerprints add <name>")"
            ;;
        "${remove_keys}" | remove)
            rm -f "${keys}"
            echo "$(gettext "Security keys removed, sign in with your password")"
            exit 0
            ;;
        *)
            exit 0
            ;;
    esac
    echo "$(gettext "Insert the key, then touch it when it blinks (fingerprint keys: with an enrolled finger)")"
    credential="$(pamu2fcfg --nouser --origin="${origin}" --appid="${origin}" "${flags[@]}")"
    # One line per user: user:credential1:credential2...
    mkdir -p "$(dirname "${keys}")"
    line="$(grep "^${USER}:" "${keys}" 2>/dev/null || echo "${USER}")"
    (umask 077 && printf '%s%s\n' "${line}" "${credential}" >"${keys}.new")
    mv "${keys}.new" "${keys}"
    # The login screen and sudo may only read it with its SELinux label (auth_home_t)
    restorecon -R "$(dirname "${keys}")"
    echo "$(gettext "Security key added. Touch it to sign in, to unlock the screen and to approve sudo.")"
    echo "$(gettext "Your password keeps working.")"

# Log in, unlock the screen and approve sudo with a finger, on a machine with a fingerprint reader
[positional-arguments]
fingerprint ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    add_finger="$(gettext "Add a finger")"
    test_finger="$(gettext "Test a finger")"
    remove_fingers="$(gettext "Remove all fingers")"
    cancel="$(gettext "Cancel")"
    # A finger's name in the session's language (fprintd's own names are the ones typed)
    finger_name() {
        case "$1" in
            right-index-finger) gettext "right index finger" ;;
            left-index-finger) gettext "left index finger" ;;
            right-thumb) gettext "right thumb" ;;
            left-thumb) gettext "left thumb" ;;
            right-middle-finger) gettext "right middle finger" ;;
            left-middle-finger) gettext "left middle finger" ;;
            right-ring-finger) gettext "right ring finger" ;;
            left-ring-finger) gettext "left ring finger" ;;
            right-little-finger) gettext "right little finger" ;;
            left-little-finger) gettext "left little finger" ;;
            *) echo "${1//-/ }" ;;
        esac
    }
    # The image turns fingerprint login on for every account (08-hardening.sh, authselect's
    # with-fingerprint), so a finger enrolled here works at once. fprintd says "No devices available"
    # when there is no reader it can drive.
    listed="$(timeout 30 fprintd-list "${USER}" 2>&1 || true)"
    if [[ -z "${listed}" ]] || grep -qi "no devices" <<<"${listed}"; then
        echo "$(gettext "This machine has no fingerprint reader that fprintd can use.")"
        echo "$(gettext "The readers it supports: https://fprint.freedesktop.org/supported-devices.html")"
        exit 1
    fi
    mapfile -t fingers < <(sed -n 's/^ *- #[0-9]*: *//p' <<<"${listed}")
    if [[ ${#fingers[@]} -gt 0 ]]; then
        enrolled="${fingers[*]}"
        echo "$(eval_gettext "Fingers enrolled for \${USER}: \${enrolled}")"
    else
        echo "$(eval_gettext "No finger is enrolled for \${USER}.")"
    fi
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Fingerprint login")" "${add_finger}" "${test_finger}" "${remove_fingers}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${add_finger}" | add)
            finger="$(gum choose --header "$(gettext "Which finger?")" right-index-finger left-index-finger right-thumb \
                left-thumb right-middle-finger left-middle-finger right-ring-finger left-ring-finger \
                right-little-finger left-little-finger)" || exit 0
            [[ -n "${finger}" ]] || exit 0
            name="$(finger_name "${finger}")"
            echo "$(eval_gettext "Touch the reader with your \${name} again and again, a little differently each time, until it says the enrolment is complete.")"
            fprintd-enroll -f "${finger}" "${USER}"
            echo
            echo "$(gettext "Enrolled. The login screen, the lock screen, sudo and every password prompt of the desktop now take this finger as well as your password, which keeps working.")"
            echo "$(gettext "Two things still ask for the password: the disk passphrase at boot, and the keyring, which a login by finger leaves locked until an app needs it.")"
            ;;
        "${test_finger}" | test)
            fprintd-verify "${USER}"
            ;;
        "${remove_fingers}" | remove)
            gum confirm "$(eval_gettext "Remove every finger enrolled for \${USER}?")" || exit 0
            fprintd-delete "${USER}"
            echo "$(gettext "Removed. Sign in with your password.")"
            if [[ "$(/usr/libexec/amethystora-duress status-fingerprint)" == on ]]; then
                echo "$(gettext "A duress finger is gone with the others. Set one up again with: ame security duress")"
            fi
            ;;
        *) exit 0 ;;
    esac

# A duress finger or password: one that unlocks at the login screen, is refused at sudo, and runs your action
[positional-arguments]
duress ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    # Its state is root's alone: /etc/amethystora/duress.d and the authselect features with-duress-fingerprint
    # and with-duress-password (/usr/libexec/amethystora-duress), and deliberately no key in
    # /etc/amethystora/security.conf, which anyone may read. Nothing shown to a person made to unlock the
    # machine should say which account has one.
    duress=/usr/libexec/amethystora-duress
    identify=(/usr/libexec/amethystora-duress-service identify "${USER}")
    setup_finger="$(gettext "Set up a duress finger")"
    setup_password="$(gettext "Set up a duress password")"
    test_finger="$(gettext "Test my duress finger")"
    change_finger="$(gettext "Change the duress finger's action")"
    change_password="$(gettext "Change the duress password's action")"
    remove_finger="$(gettext "Remove my duress finger")"
    remove_password="$(gettext "Remove my duress password")"
    turn_off_all="$(gettext "Turn off for everyone")"
    cancel="$(gettext "Cancel")"
    # A finger's name in the session's language (fprintd's own names are the ones typed)
    finger_name() {
        case "$1" in
            right-index-finger) gettext "right index finger" ;;
            left-index-finger) gettext "left index finger" ;;
            right-thumb) gettext "right thumb" ;;
            left-thumb) gettext "left thumb" ;;
            right-middle-finger) gettext "right middle finger" ;;
            left-middle-finger) gettext "left middle finger" ;;
            right-ring-finger) gettext "right ring finger" ;;
            left-ring-finger) gettext "left ring finger" ;;
            right-little-finger) gettext "right little finger" ;;
            left-little-finger) gettext "left little finger" ;;
            *) echo "${1//-/ }" ;;
        esac
    }
    if [[ "$("${duress}" status)" == on ]]; then
        echo "$(gettext "Duress protection is on on this machine.")"
    else
        echo "$(gettext "Duress protection is off on this machine.")"
    fi
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Duress finger or password")" \
            "${setup_finger}" "${setup_password}" \
            "${test_finger}" \
            "${change_finger}" "${change_password}" \
            "${remove_finger}" "${remove_password}" \
            "${turn_off_all}" "${cancel}")"
    fi
    # Touch the reader once and say which enrolled finger it matched, never running the action
    touch_once() {
        echo "$1" >&2
        "${identify[@]}" | sed -n 's/^matched=//p'
    }
    # The fingers the reader has enrolled, for the finger actions only; a duress password needs no reader
    finger_list() {
        if ! "${duress}" supported; then
            echo "$(gettext "This image's fingerprint service cannot tell one finger from another, which a duress finger needs.")"
            exit 1
        fi
        local listed
        listed="$(timeout 30 fprintd-list "${USER}" 2>&1 || true)"
        if [[ -z "${listed}" ]] || grep -qi "no devices" <<<"${listed}"; then
            echo "$(gettext "This machine has no fingerprint reader that fprintd can use.")"
            exit 1
        fi
        mapfile -t fingers < <(sed -n 's/^ *- #[0-9]*: *//p' <<<"${listed}")
    }
    # The warning both set-up paths show first: what a duress credential cannot do
    limits() {
        echo "$(gettext "It cannot stop anyone asking for your password instead, nor help against a copy of the disk made before, and anyone with root on this machine can read how it is set up.")"
        echo "$(gettext "In many places, destroying data while compelled by the authorities is an offence of its own.")"
        echo "$(gettext "What the action does is yours to decide: none of the examples destroys anything.")"
        echo
    }
    choose_action() {
        local src="$1" header question choice file url address='^https://[A-Za-z0-9._~:/?#@!&()*+,;=%-]+$'
        local send_alert take_key power_off own_script
        send_alert="$(gettext "Send an alert to an address of mine")"
        take_key="$(gettext "Take the disk's key out of the TPM")"
        power_off="$(gettext "Power off ten minutes later")"
        own_script="$(gettext "Run a script of my own")"
        if [[ "${src}" == fingerprint ]]; then
            header="$(gettext "What should the duress finger do?")"
            question="$(gettext "Use this as the duress finger's action?")"
        else
            header="$(gettext "What should the duress password do?")"
            question="$(gettext "Use this as the duress password's action?")"
        fi
        choice="$(gum choose --header "${header}" "${send_alert}" \
            "${take_key}" "${power_off}" "${own_script}" "${cancel}")"
        file="$(mktemp)"
        case "${choice}" in
            "${send_alert}")
                url="$(gum input --header "$(gettext "The address to send it to, such as an ntfy.sh topic only you know")" \
                    --placeholder "https://ntfy.sh/...")"
                # Written between double quotes in a script that root runs: no quote, $ or backquote gets in
                [[ "${url}" =~ ${address} ]] || { echo "$(gettext "Not an https:// address.")"; exit 1; }
                awk -v url="${url}" '$0 == "URL=\"\"" { print "URL=\"" url "\""; next } { print }' \
                    /usr/share/amethystora/duress/examples/alert >"${file}"
                ;;
            "${take_key}") cp /usr/share/amethystora/duress/examples/tpm-keyslot "${file}" ;;
            "${power_off}") cp /usr/share/amethystora/duress/examples/power-off-later "${file}" ;;
            "${own_script}")
                local own
                own="$(gum input --header "$(gettext "The script's path")")"
                [[ -f "${own}" ]] || { echo "$(gettext "No such file.")"; exit 1; }
                cp -- "${own}" "${file}"
                ;;
            *) exit 0 ;;
        esac
        echo "$(gettext "The action, which runs as root:")"
        echo
        cat "${file}"
        echo
        gum confirm "${question}" || exit 0
        sudo /usr/libexec/amethystora-security-config note duress action
        sudo "${duress}" action "${src}" "${file}"
        rm -f "${file}"
    }
    case "${action}" in
        "${setup_finger}" | setup)
            finger_list
            echo "$(gettext "A duress finger is one more finger, enrolled like the others. At the login screen and the lock screen it unlocks the machine as any finger does. At sudo, at password prompts and at a text console it is refused, as a finger that did not match. Either way the action you choose runs, as root, once until the next start, and nothing about it reaches the journal.")"
            echo
            limits
            if [[ ${#fingers[@]} -eq 0 ]]; then
                echo "$(gettext "Enroll an ordinary finger first, with: ame security fingerprint add")"
                exit 1
            fi
            # A reader that cannot identify compares a touch with one finger it chose itself, never the duress one.
            # fprintd does that with a single finger enrolled on any reader, so only two or more tell; with one,
            # the two touches after enrolling decide.
            if [[ ${#fingers[@]} -gt 1 ]] && ! "${identify[@]}" --selected | grep -qx "selected=any"; then
                echo "$(gettext "This reader cannot tell one finger from another, so it cannot have a duress finger.")"
                exit 1
            fi
            if ! /usr/libexec/amethystora-duress-service has-signal; then
                echo "$(gettext "The fingerprint service does not say which finger matched, so it cannot have a duress finger.")"
                exit 1
            fi
            gum confirm "$(eval_gettext "Set up a duress finger for \${USER}?")" || exit 0
            mapfile -t free < <(printf '%s\n' right-index-finger left-index-finger right-thumb left-thumb \
                right-middle-finger left-middle-finger right-ring-finger left-ring-finger right-little-finger \
                left-little-finger | grep -vxF -f <(printf '%s\n' "${fingers[@]}"))
            [[ ${#free[@]} -gt 0 ]] || { echo "$(gettext "Every finger is enrolled already: remove one first.")"; exit 1; }
            finger="$(gum choose --header "$(gettext "Which finger? Pick one you would not use by habit.")" "${free[@]}")"
            [[ -n "${finger}" ]] || exit 0
            name="$(finger_name "${finger}")"
            echo "$(eval_gettext "Touch the reader with your \${name} again and again, a little differently each time, until it says the enrolment is complete.")"
            fprintd-enroll -f "${finger}" "${USER}"
            matched="$(touch_once "$(eval_gettext "Now touch the reader once with your \${name}.")")"
            other="$(touch_once "$(gettext "And once with one of your other fingers.")")"
            if [[ "${matched}" != "${finger}" || -z "${other}" || "${other}" == "${finger}" ]]; then
                echo "$(eval_gettext "The reader did not tell the two apart, so the \${name} stays an ordinary finger.")"
                gum confirm "$(eval_gettext "Remove the \${name} again?")" && fprintd-delete "${USER}" -f "${finger}"
                exit 1
            fi
            sudo /usr/libexec/amethystora-security-config note duress on
            sudo "${duress}" set "${USER}" "${finger}"
            if ! sudo test -f /etc/amethystora/duress.d/action-fingerprint; then
                choose_action fingerprint
            fi
            sudo "${duress}" on
            # Translators: "Test my duress finger" is a choice of the menu of ame security duress; use its translation here.
            echo "$(gettext "Set up. Test it with: ame security duress, Test my duress finger, which never runs the action.")"

            ;;
        "${setup_password}" | setup-password)
            echo "$(gettext "A duress password is a second password, only yours. At the login screen and the lock screen it unlocks the machine like your real one. At sudo, at administrator prompts, at a text console and over SSH it is refused, as a wrong password. Either way the action you choose runs, as root, once until the next start, and nothing about it reaches the journal.")"
            echo
            limits
            if ! "${duress}" password-supported; then
                echo "$(gettext "This image cannot check a duress password.")"
                exit 1
            fi
            gum confirm "$(eval_gettext "Set up a duress password for \${USER}?")" || exit 0
            pw="$(gum input --password --header "$(eval_gettext "A duress password for \${USER}, not your login password")")"
            [[ -n "${pw}" ]] || exit 0
            pw2="$(gum input --password --header "$(gettext "Type it again")")"
            if [[ "${pw}" != "${pw2}" ]]; then
                echo "$(gettext "They did not match.")"
                exit 1
            fi
            sudo /usr/libexec/amethystora-security-config note duress password
            printf '%s' "${pw}" | sudo "${duress}" set-password "${USER}"
            if ! sudo test -f /etc/amethystora/duress.d/action-password; then
                choose_action password
            fi
            sudo "${duress}" on-password
            echo "$(gettext "Set up. Choose something you would not type by habit, and keep it different from your login password: a duress password the same as your real one never runs the action.")"
            ;;
        "${test_finger}" | test)
            finger="$(sudo "${duress}" finger "${USER}")"
            [[ -n "${finger}" ]] || { echo "$(eval_gettext "\${USER} has no duress finger.")"; exit 0; }
            matched="$(touch_once "$(gettext "Touch the reader once, with the duress finger or another.")")"
            if [[ "${matched}" == "${finger}" ]]; then
                echo "$(gettext "That was your duress finger: the login screen would let you in and run the action.")"
            elif [[ -n "${matched}" ]]; then
                name="$(finger_name "${matched}")"
                echo "$(eval_gettext "That was an ordinary finger (\${name}).")"
            else
                echo "$(gettext "That finger did not match any of yours.")"
            fi
            problems="$(sudo "${duress}" check fingerprint || true)"
            [[ -z "${problems}" ]] || { echo "$(gettext "But the action would not run now:")"; echo "${problems}"; }
            ;;
        "${change_finger}" | action-fingerprint)
            choose_action fingerprint
            ;;
        "${change_password}" | action-password)
            choose_action password
            ;;
        "${remove_finger}" | remove)
            finger="$(sudo "${duress}" finger "${USER}")"
            [[ -n "${finger}" ]] || { echo "$(eval_gettext "\${USER} has no duress finger.")"; exit 0; }
            sudo /usr/libexec/amethystora-security-config note duress off
            sudo "${duress}" unset "${USER}"
            name="$(finger_name "${finger}")"
            if gum confirm "$(eval_gettext "Delete the \${name} from the reader too? Otherwise it stays as an ordinary finger.")"; then
                fprintd-delete "${USER}" -f "${finger}"
            fi
            [[ -n "$(sudo "${duress}" accounts)" ]] || sudo "${duress}" off
            echo "$(gettext "Removed.")"
            ;;
        "${remove_password}" | remove-password)
            if ! sudo "${duress}" has-password "${USER}"; then
                echo "$(eval_gettext "\${USER} has no duress password.")"
                exit 0
            fi
            sudo /usr/libexec/amethystora-security-config note duress off
            sudo "${duress}" unset-password "${USER}"
            [[ -n "$(sudo "${duress}" password-accounts)" ]] || sudo "${duress}" off-password
            echo "$(gettext "Removed.")"
            ;;
        "${turn_off_all}" | off)
            gum confirm "$(gettext "Forget every account's duress finger, password and both actions? The fingers stay, as ordinary ones.")" || exit 0
            sudo /usr/libexec/amethystora-security-config note duress off
            sudo "${duress}" forget
            echo "$(gettext "Turned off.")"
            ;;
        *) exit 0 ;;
    esac

# Ask for a code from a phone app as well as the password when someone logs in over SSH with a password
[positional-arguments]
ssh-codes ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    # The switch is the two files this helper writes, and nothing in /etc/amethystora/security.conf: sshd
    # cannot read that file, and a second record of the same state could disagree with the first
    codes=/usr/libexec/amethystora-ssh-codes
    secret="${HOME}/.google_authenticator"
    setup_codes="$(gettext "Set up my codes")"
    turn_on="$(gettext "Turn on")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    if [[ "$("${codes}" status)" == on ]]; then
        echo "$(gettext "One-time codes for SSH are on: a password login over SSH also asks for a code.")"
    else
        echo "$(gettext "One-time codes for SSH are off: a password alone logs in over SSH, once the SSH server is on.")"
    fi
    if [[ -e "${secret}" ]]; then echo "$(eval_gettext "\${USER} has codes set up.")"; else echo "$(eval_gettext "\${USER} has no codes set up.")"; fi
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "One-time codes for SSH")" "${setup_codes}" "${turn_on}" "${turn_off}" "${cancel}")" || exit 0
    fi
    reload() {
        systemctl is-active --quiet sshd.service || return 0
        sudo systemctl reload sshd.service
    }
    case "${action}" in
        "${setup_codes}" | setup)
            if [[ -e "${secret}" ]]; then
                gum confirm "$(gettext "Replace your codes? The entry in your phone's app stops working.")" || exit 0
            fi
            echo "$(gettext "Scan the picture below with an authenticator app on your phone, such as Aegis or FreeOTP, then type the code it shows.")"
            google-authenticator --time-based --disallow-reuse --rate-limit=3 --rate-time=30 \
                --window-size=3 --qr-mode=UTF8 --issuer=Amethystora --force
            # sshd may only read and rewrite it with its SELinux label (auth_home_t)
            restorecon "${secret}"
            echo
            echo "$(gettext "Keep the emergency scratch codes above somewhere safe and away from this machine. Each one logs in once without the phone.")"
            ;;
        "${turn_on}" | on)
            if [[ ! -e "${secret}" && ! -s "${HOME}/.ssh/authorized_keys" ]]; then
                echo "$(gettext "Set up your own codes first, with: ame security ssh-codes setup")"
                echo "$(gettext "Or add an SSH key to ~/.ssh/authorized_keys. Otherwise you could not log in over SSH yourself.")"
                exit 1
            fi
            echo
            echo "$(gettext "A password login over SSH will also ask for a code from the account's own phone app. An account without codes of its own can then log in over SSH only with a key.")"
            gum confirm "$(gettext "Turn one-time codes for SSH on?")" || exit 0
            sudo /usr/libexec/amethystora-security-config note ssh-codes on
            sudo "${codes}" on
            # A server reloaded with settings it refuses stops, so it checks them first. One that is not
            # running checks them as it starts, and before its first start it has no host keys to check with.
            if systemctl is-active --quiet sshd.service && ! sudo sshd -t; then
                sudo "${codes}" off
                echo "$(gettext "The SSH server refused the settings, so nothing was changed.")"
                exit 1
            fi
            reload
            echo "$(gettext "On. A login with a key works as before.")"
            ;;
        "${turn_off}" | off)
            sudo /usr/libexec/amethystora-security-config note ssh-codes off
            sudo "${codes}" off
            reload
            echo "$(gettext "Off. A password alone logs in over SSH again, and your codes stay set up.")"
            ;;
        *) exit 0 ;;
    esac

# Let Secure Boot load the kernel modules shipped in the image (DisplayLink, v4l2loopback, ...)
secure-boot:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    ENROLLMENT_PASSWORD="amethystora"
    # One check, shared with `ame security status` (/usr/libexec/amethystora-mok-status). A missing
    # certificate is reported as missing, never counted as enrolled.
    MOK="$(/usr/libexec/amethystora-mok-status)"
    certs=()
    while read -r state cert rest; do
        case "${state}" in
            not-enrolled) certs+=("${cert}") ;;
            pending) echo "$(eval_gettext "\${cert} is waiting for the next boot: finish it in the blue MOK manager screen.")" ;;
            missing) echo "$(eval_gettext "\${cert} is not on this machine, so it can be neither checked nor enrolled.")" ;;
            unknown) echo "$(eval_gettext "Could not tell whether \${cert} is enrolled. mokutil said: \${rest}")" ;;
        esac
    done <<<"${MOK}"
    if [[ ${#certs[@]} -eq 0 ]]; then
        grep -qv "^enrolled " <<<"${MOK}" || echo "$(gettext "The Secure Boot keys are already enrolled.")"
        exit 0
    fi
    echo "$(gettext "The kernel modules built into the image, such as evdi for DisplayLink docks, are signed with keys of the image's own. With Secure Boot on, the kernel refuses them until those keys are enrolled.")"
    echo "$(gettext "Enrolling asks for your password here and queues the keys. At the next restart a blue screen, the MOK manager, asks you to accept them, and nothing is trusted unless you do.")"
    if ! mokutil --sb-state 2>/dev/null | grep -qi enabled; then
        echo "$(gettext "Secure Boot is off, so enrolling alone changes nothing: the kernel trusts these keys only with Secure Boot on. Turn it on in the firmware as well (ame system bios).")"
    fi
    gum confirm "$(gettext "Queue the keys for the next restart?")" || exit 0
    sudo mokutil --timeout -1
    printf '%s\n%s\n' "${ENROLLMENT_PASSWORD}" "${ENROLLMENT_PASSWORD}" | sudo mokutil --import "${certs[@]}"
    echo "$(gettext "At next reboot, the blue MOK manager screen appears (QWERTY keyboard layout).")"
    # Translators: the quoted words are the blue screen's own buttons, which are not translated.
    echo "$(eval_gettext "Select \"Enroll MOK\", then \"Continue\", \"Yes\", and type \"\${ENROLLMENT_PASSWORD}\" as the password.")"

# Kernel lockdown on a machine without Secure Boot, where it costs the modules signed with this machine's key,
# and its strict level, which also keeps root from reading the kernel's memory
[positional-arguments]
lockdown ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    turn_on="$(gettext "Turn on")"
    # Translators: a level of kernel lockdown, the stricter one (confidentiality), as a menu choice.
    strict="$(gettext "Strict")"

    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    # The kernel argument is the switch. Fedora's kernel locks itself down whenever it starts with Secure
    # Boot, whatever the arguments say, so the argument only decides it on a machine without.
    mode="$(sed -n 's/.*\[\([a-z]*\)\].*/\1/p' /sys/kernel/security/lockdown 2>/dev/null || true)"
    mode="${mode:-none}"
    echo "$(eval_gettext "Kernel lockdown is \${mode} now.")"
    secure_boot=false
    mokutil --sb-state 2>/dev/null | grep -qi enabled && secure_boot=true
    if [[ "${secure_boot}" == true ]]; then
        echo "$(gettext "Secure Boot is on, so the kernel locks itself down at every start whatever this says.")"
    elif grep -qw "lockdown=integrity" /proc/cmdline; then
        echo "$(gettext "Secure Boot is off, and the kernel argument lockdown=integrity turns it on.")"
    else
        echo "$(gettext "Secure Boot is off, and no kernel argument turns it on.")"
    fi
    if [[ -e /usr/share/amethystora/sealed ]]; then
        echo "$(gettext "This machine is sealed: its kernel arguments are inside the signed kernel image, and cannot change.")"
        exit 1
    fi
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Kernel lockdown")" "${turn_on}" "${strict}" "${turn_off}" "${cancel}")" || exit 0
    fi
    nvidia=false
    [[ "$(jq -r '."image-flavor" // ""' /usr/share/amethystora/image-info.json 2>/dev/null)" == nvidia* ]] && nvidia=true
    case "${action}" in
        "${turn_on}" | on)
            echo
            echo "$(gettext "Lockdown refuses what lets root rewrite the running kernel: a module without a signature it trusts, writing to /dev/mem, and BPF that writes into another program's memory. A compromise of root then ends at the next restart.")"
            if [[ "${secure_boot}" != true && "${nvidia}" == true ]]; then
                echo
                echo "$(gettext "Without Secure Boot, this machine's key is not trusted, and the NVIDIA driver is signed with it: it will not load, and the screen falls back to a basic driver. Turn Secure Boot on instead, and the kernel locks itself down with the driver working.")"
            elif [[ "${secure_boot}" != true ]]; then
                echo "$(gettext "Without Secure Boot, the modules built into the image, such as evdi for DisplayLink docks, are signed with a key the kernel does not trust then, and will not load.")"
            fi
            gum confirm "$(gettext "Turn kernel lockdown on, from the next restart?")" || exit 0
            sudo rpm-ostree kargs --append-if-missing=lockdown=integrity --delete-if-present=lockdown=confidentiality
            echo "$(gettext "On from the next restart.")"
            ;;
        "${strict}" | strict)
            echo
            echo "$(gettext "Strict lockdown (confidentiality) also refuses what lets root read the running kernel: BPF programs that read kernel memory, kprobes, perf with kernel data and /proc/kcore. bpftrace, perf and eBPF security tools stop working. Unlike the usual level, it applies with Secure Boot on too.")"
            gum confirm "$(gettext "Make kernel lockdown strict, from the next restart?")" || exit 0
            sudo rpm-ostree kargs --append-if-missing=lockdown=confidentiality
            echo "$(gettext "Strict from the next restart. ame security lockdown on goes back to the usual level.")"
            ;;
        "${turn_off}" | off)
            if [[ "${secure_boot}" == true ]]; then
                echo "$(gettext "Secure Boot is on, so the kernel stays locked down. This only matters if it is turned off.")"
            fi
            echo
            echo "$(gettext "Lockdown refuses what lets root rewrite the running kernel: a module without a signature it trusts, writing to /dev/mem, and BPF that writes into another program's memory. A compromise of root then ends at the next restart.")"
            if [[ "${secure_boot}" != true && "${nvidia}" != true ]]; then
                echo "$(gettext "To keep it and still load the modules built into the image, such as evdi for DisplayLink docks, turn Secure Boot on in the firmware and enroll their keys: ame security secure-boot.")"
            fi
            gum confirm "$(gettext "Leave lockdown out of the kernel arguments, from the next restart?")" || exit 0
            sudo rpm-ostree kargs --delete-if-present=lockdown=integrity --delete-if-present=lockdown=confidentiality
            echo "$(gettext "Off from the next restart. Updates keep it that way; ame security lockdown on puts it back.")"
            ;;
        *) exit 0 ;;
    esac

# What the security settings are actually doing, and what to do about anything off. Changes nothing
status:
    @/usr/libexec/amethystora-security-status

# What each Flatpak app can reach beyond its sandbox, and taking back what you granted one
[positional-arguments]
apps ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # The one list of it, which the security report and the Security app's Apps page read too
    permissions=/usr/libexec/amethystora-app-permissions
    echo "$(gettext "What each Flatpak app can reach beyond its sandbox without asking you. What was granted on this machine, in Flatseal or with flatpak override, is said beside it.")"
    "${permissions}"
    echo
    action="${1:-}"
    take_back="$(gettext "Take back what I granted an app")"
    open_flatseal="$(gettext "Open Flatseal")"
    cancel="$(gettext "Cancel")"
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "App permissions")" "${take_back}" "${open_flatseal}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${take_back}" | reset)
            mapfile -t granted < <("${permissions}" --json | jq -r '.[] | select(.user_overrides) | "\(.name) (\(.app))"')
            if [[ ${#granted[@]} -eq 0 ]]; then
                echo "$(gettext "You have granted no app anything of your own.")"
                exit 0
            fi
            choice="$(printf '%s\n' "${granted[@]}" | gum choose --header "$(gettext "Take back from which app?")")" || exit 0
            [[ -n "${choice}" ]] || exit 0
            app="${choice##*(}"
            app="${app%)}"
            name="${choice% (*}"
            echo "$(eval_gettext "This removes every permission you granted \${name}, and leaves what the app asks for itself and what the image allows every app.")"
            gum confirm "$(gettext "Take them back?")" || exit 0
            flatpak override --user --reset "${app}"
            echo "$(gettext "Taken back, from the app's next start.")"
            ;;
        "${open_flatseal}" | flatseal)
            setsid -f flatpak run com.github.tchx84.Flatseal >/dev/null 2>&1
            ;;
        *) exit 0 ;;
    esac

# Installed apps and the system's files against what their updates installed: what the last check found, a
# check now, or an app put back as its update installed it
[positional-arguments]
app-integrity ACTION="status" APP="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # What the check found, as the security watcher passes it on; of your own apps only which
    status=/var/lib/amethystora/security/status.json
    me="$(id -un)"
    action="${1:-status}"
    app="${2:-}"
    if ! /usr/libexec/amethystora-security-config --is APP_INTEGRITY on; then
        echo "$(gettext "Installed apps are not checked against their updates. Turn it on with:")"
        echo "  ame security settings app-integrity on"
        exit 0
    fi
    case "${action}" in
        status)
            if ! jq -e '.integrity.time' "${status}" >/dev/null 2>&1; then
                echo "$(gettext "Not checked yet. The first check runs within a day, or now with: ame security app-integrity check")"
                exit 0
            fi
            # The numbers and the time the report holds, said here in the session's language
            IFS=$'\t' read -r when has_system files deployments apps runtimes < <(jq -r '.integrity as $i |
                [($i.time | strflocaltime("%c")), (if $i.system then "yes" else "no" end), ($i.system.objects | tostring),
                 ($i.system.deployments | tostring), (($i.flatpak.apps // 0) | tostring), (($i.flatpak.runtimes // 0) | tostring)] | @tsv' "${status}")
            [[ "${deployments}" =~ ^[0-9]+$ ]] || deployments=0
            {
                echo "$(eval_gettext "Last checked: \${when}")"
                if [[ "${has_system}" == yes ]]; then
                    # Translators: the number that decides the plural is the one of deployments (the system's installed copies).
                    echo "$(eval_ngettext "The system: \${files} files, in \${deployments} deployment" "The system: \${files} files, in \${deployments} deployments" "${deployments}")"

                else
                    echo "$(gettext "The system: sealed, so it checks every file itself as it is read")"
                fi
                echo "$(eval_gettext "The system's Flatpaks: \${apps} apps and \${runtimes} runtimes")"
                echo
                if [[ "$(jq -r --arg me "${me}" '[.integrity.findings[] | select(.account == "-" or .account == $me)] | length' "${status}")" -eq 0 ]]; then
                    echo "$(gettext "Nothing changed outside an update.")"
                else
                    jq -r '.integrity.findings[] | select(.account == "-") | "  \(.text)"' "${status}"
                    jq -r --arg me "${me}" '.integrity.findings[] | select(.account == $me) | (.app // "?")' "${status}" |
                        while IFS= read -r own_app; do
                            echo "  $(eval_gettext "A file of your own app \${own_app} changed outside an update")"
                        done
                fi
            } | /usr/libexec/amethystora-security-text --lines
            ;;
        check)
            echo "$(gettext "Checking. The first check reads every app once, which takes a few minutes; after that, only what changed.")"
            # The person at the machine may start it without a password (50-amethystora-integrity.rules)
            started="$(date +%s)"
            systemctl start amethystora-integrity.service
            # The check starts the watcher as it ends, which says what is new and passes it on to the report and to this
            for _ in $(seq 1 60); do
                [[ "$(jq -r '.integrity.time // 0' "${status}" 2>/dev/null)" -ge "${started}" ]] && break
                sleep 1
            done
            ame security app-integrity status
            ;;
        repair)
            # The apps to put back: the one named, or every one the last check named, with the installation each is
            # in and the remote it came from. The system's own files are not among them: something with root
            # changed those, and the machine cannot vouch for its own repair.
            mapfile -t targets < <(jq -r --arg me "${me}" --arg app "${app}" '.integrity.findings // [] | .[]
                | select((.account == "-" or .account == $me) and (.installation == "system" or .installation == "user"))
                | select($app == "" or .app == $app)
                | [.installation, (.ref // ""), (.origin // "")] | @tsv' "${status}" 2>/dev/null | sort -u)
            if [[ -n "${app}" && ${#targets[@]} -eq 0 ]]; then
                for installation in system user; do
                    ref="$(flatpak info "--${installation}" --show-ref "${app}" 2>/dev/null)" || continue
                    origin="$(flatpak info "--${installation}" --show-origin "${app}")"
                    targets+=("${installation}"$'\t'"${ref}"$'\t'"${origin}")
                    break
                done
                [[ ${#targets[@]} -gt 0 ]] || { echo "$(eval_gettext "\${app} is not installed.")"; exit 1; }
            fi
            if [[ ${#targets[@]} -eq 0 ]]; then
                echo "$(gettext "The last check named no app to put back.")"
                if jq -e '.integrity.findings // [] | any(.[]; .installation == "os")' "${status}" >/dev/null 2>&1; then
                    echo "$(gettext "It found the system's own files changed. Something with administrator rights changed them, so back up your files and reinstall from the installer: see Starting over in the manual.")"
                fi
                exit 0
            fi
            echo "$(gettext "Each app is put back as its update installed it: its files are fetched again from where it came from and installed again. Its data in ~/.var/app stays as it is.")"
            if [[ -n "${app}" ]]; then
                question="$(eval_gettext "Put back \${app}?")"
            else
                question="$(gettext "Put back the apps the last check named?")"
            fi
            gum confirm "${question}" || exit 0
            # Each installation's repository first, which fetches again what it holds that no longer matches, then each
            # app installed again from it
            for installation in $(printf '%s\n' "${targets[@]}" | cut -f1 | sort -u); do
                if [[ "${installation}" == system ]]; then
                    sudo flatpak repair --system
                else
                    flatpak repair --user
                fi
            done
            for target in "${targets[@]}"; do
                IFS=$'\t' read -r installation ref origin <<<"${target}"
                [[ -n "${ref}" && -n "${origin}" ]] || continue
                if [[ "${installation}" == system ]]; then
                    sudo flatpak install --system --reinstall --noninteractive -y "${origin}" "${ref}"
                else
                    flatpak install --user --reinstall --noninteractive -y "${origin}" "${ref}"
                fi
            done
            echo
            ame security app-integrity check
            ;;
        *)
            echo "usage: ame security app-integrity [status | check | repair [APP]]"
            exit 64
            ;;
    esac

# Audit this machine's security settings with Lynis and say what to do about each finding
audit:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    echo "$(gettext "Auditing this machine. It takes a few minutes and changes nothing.")"
    echo "$(gettext "Settings for the audit are in /etc/lynis/custom.prf.")"
    echo
    # Lynis reads files only root can read; without sudo it silently skips those tests
    sudo lynis audit system
    echo
    echo "$(gettext "Full output: /var/log/lynis.log")"
    echo "$(gettext "Findings, one per line: /var/log/lynis-report.dat")"
    echo
    echo "$(gettext "The monthly audit compares each run with the one before and reports only what is new.")"
    echo "$(gettext "What it said last time: journalctl -u amethystora-lynis-audit.service")"

# Every detection switch (security.conf): what each does, and changing one. Off costs nothing
[positional-arguments]
settings SETTING="" VALUE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    config=/usr/libexec/amethystora-security-config
    setting="${1:-}"
    value="${2:-}"
    cancel="$(gettext "Cancel")"
    if [[ -z "${setting}" ]]; then
        "${config}" --list
        echo
        echo "$(gettext "What the image enforces, and stricter settings, each changed with ame security NAME:")"
        /usr/libexec/amethystora-hardening status
        echo
        echo "$(gettext "Everything off at once, or back as the image ships it: ame security profile")"
        mapfile -t names < <("${config}" --list --json | jq -r '.settings[].setting')
        setting="$(gum choose --header "$(gettext "Change which?")" --height 16 "${names[@]}" realtime "${cancel}")" || exit 0
    fi
    case "${setting}" in
        on-detection)
            if [[ -z "${value}" ]]; then
                printf '%-12s%s\n' report "$(gettext "Say so, and leave the file where it is (the default).")"
                printf '%-12s%s\n' quarantine "$(gettext "Move it where nothing can open it. ame security scan restore puts it back.")"
                printf '%-12s%s\n' delete "$(gettext "Delete it. A match can be a false positive, and this cannot be undone.")"
                value="$(gum choose --header "$(gettext "Files a virus scan finds")" report quarantine delete)" || exit 0
            fi
            if [[ "${value}" == delete ]]; then
                gum confirm "$(gettext "Delete what scans find, with no way to get it back?")" || exit 0
            fi
            sudo "${config}" set ON_DETECTION "${value}"
            echo "$(eval_gettext "Files a virus scan finds: \${value}. This applies to the weekly scan, the scans you start in the Security app, and real-time scanning.")"
            ;;
        realtime)
            if [[ -z "${value}" ]]; then
                printf '%-5s%s\n' on "$(gettext "Read each change as the audit log records it, and scan every file written in a home folder as it is closed. Costs some CPU while files are being written.")"
                printf '%-5s%s\n' off "$(gettext "Look every 15 minutes, and scan weekly (the default).")"
                value="$(gum choose --header "$(gettext "Real-time watching")" on off)" || exit 0
            fi
            sudo "${config}" set REALTIME "${value}"
            # The units check the setting as they start (ExecCondition), so a restart applies it either
            # way. The real-time watcher goes on following network protection's alerts while that is on.
            sudo systemctl restart amethystora-security-realtime.service amethystora-clamav-onaccess.service
            if [[ "${value}" == on ]]; then
                echo "$(gettext "On. Real-time scanning starts once clamd has its virus signatures.")"
            else
                echo "$(gettext "Off.")"
            fi
            ;;
        network)
            if [[ -z "${value}" ]]; then
                printf '%-7s%s\n' off "$(gettext "Nothing is inspected (the default).")"
                printf '%-7s%s\n' watch "$(gettext "Suricata inspects every connection this machine makes and receives, and says what it recognises. Nothing is blocked. Start here: a week of it shows what blocking would get in the way of.")"
                printf '%-7s%s\n' block "$(gettext "As watch, and connections to known malware, command servers and exploits are cut off. Each one is a notification, and ame security connections allows it again.")"
                echo "$(gettext "Whatever goes wrong with it, connections go on uninspected rather than stop.")"
                value="$(gum choose --header "$(gettext "Network protection")" off watch block)" || exit 0
            fi
            sudo "${config}" set NETWORK "${value}"
            if [[ "${value}" == off ]]; then
                sudo systemctl stop amethystora-ips.service
                echo "$(gettext "Off. Nothing is inspected.")"
            else
                echo "$(eval_gettext "Getting the rules for \${value}. The first download takes a minute or two.")"
                if ! sudo /usr/libexec/amethystora-ips rules; then
                    echo "$(gettext "The rules could not be downloaded now. They are tried again every day, and until then Suricata inspects with whatever rules it had, or none.")"
                fi
                # Already running, the rules step reloaded it; this starts it when it was off
                sudo systemctl start amethystora-ips.service
                echo "$(eval_gettext "On: \${value}. See what it blocks and notices with: ame security connections")"
            fi
            # The real-time watcher follows Suricata's alerts while this is on, and the report's view of
            # it is the security watcher's to write
            sudo systemctl restart amethystora-security-realtime.service
            sudo systemctl start --no-block amethystora-security-watch.service
            ;;
        "" | "${cancel}" | Cancel) exit 0 ;;
        *)
            key="$(tr 'a-z-' 'A-Z_' <<<"${setting}")"
            entry="$("${config}" --list --json | jq -c --arg key "${key}" '.settings[] | select(.key == $key)')"
            if [[ -z "${entry}" ]]; then
                echo "$(eval_gettext "There is no setting called \${setting}. ame security settings lists them.")"
                exit 64
            fi
            title="$(jq -r .title <<<"${entry}")"
            # Site checking is a choice in browser protection, which allows Osprey in the browsers
            if [[ "${key}" == OSPREY ]]; then
                exec ame security browser osprey "${value}"
            fi
            echo "${title}: $(jq -r .value <<<"${entry}")"
            jq -r .text <<<"${entry}" | fold -s -w 96
            if [[ -z "${value}" ]]; then
                mapfile -t values < <(jq -r '.allowed[]' <<<"${entry}")
                value="$(gum choose --header "${title}" "${values[@]}" "${cancel}")" || exit 0
                [[ -n "${value}" && "${value}" != "${cancel}" ]] || exit 0
            fi
            # What turning these down gives up, said before it is done
            case "${key}:${value}" in
                HARDENING:off)
                    echo "$(gettext "Turning a security switch off, this one included, is then not said, and from the next restart not recorded either.")" ;;
                AUDIT_LOG:off)
                    echo "$(gettext "From the next restart nothing records changes to accounts, sudo, what starts by itself or the security settings, and the switches that read the audit log have nothing to read.")" ;;
                WATCHER:off)
                    echo "$(gettext "Nothing then reads the audit log or the journal, or says what changed. The audit log goes on recording, for sudo ausearch, until its own switch is off too.")" ;;
                VIRUS_SCAN:off)
                    echo "$(gettext "No scanning, no signatures downloaded, and nothing holding them in memory. A package file you open is installed without a scan.")" ;;
                NOTIFY:none)
                    echo "$(gettext "A virus found is not a notification either. Everything still shows in the report and in ame security events.")" ;;
            esac
            if [[ "${value}" == off || "${value}" == none ]]; then
                gum confirm "${title}: ${value}?" || exit 0
            fi
            # What a switch sends to somebody else is said, and asked, before it is turned on
            explain="$(jq -r '.explain // empty' <<<"${entry}")"
            if [[ -n "${explain}" && "${value}" != off ]]; then
                echo
                fold -s -w 96 <<<"${explain}"
                gum confirm "$(eval_gettext "Turn \${title} on, knowing that?")" || exit 0
            fi
            # Hash reputation asks with the owner's own abuse.ch key, kept where only root can read it
            if [[ "${key}" == REPUTATION && "${value}" == on ]]; then
                echo
                echo "$(gettext "Make a free key at https://auth.abuse.ch/ (sign in with a GitHub, Google or similar account).")"
                abusech="$(gum input --password --prompt "$(gettext "abuse.ch key")> " | tr -cd 'A-Za-z0-9_-')" || exit 0
                [[ -n "${abusech}" ]] || { echo "$(gettext "No key, so nothing is looked up.")"; exit 0; }
                printf '%s\n' "${abusech}" | sudo install -D -m 0600 /dev/stdin /etc/amethystora/abusech.key
            fi
            sudo "${config}" set "${key}" "${value}"
            # Each unit reads its key as it starts, so a restart applies it either way
            for step in $(jq -r '.applies[]' <<<"${entry}"); do
                case "${step}" in
                    restart:*) sudo systemctl restart "${step#restart:}" || true ;;
                    run) sudo systemctl start --no-block amethystora-security-watch.service || true ;;
                    reboot) echo "$(gettext "The audit log's own part of this takes effect at the next restart.")" ;;
                esac
            done
            echo "${title}: ${value}."
            ;;
    esac

# Every detection switch at once: off turns off everything that can be, default puts each back as shipped
[positional-arguments]
profile NAME="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    config=/usr/libexec/amethystora-security-config
    name="${1:-}"
    cancel="$(gettext "Cancel")"
    current="$("${config}" --profile || true)"
    echo "$(eval_gettext "The switches match the profile: \${current}")"
    # What each profile does, also when one is named (the Security app names it): said before anything changes
    echo
    printf '%-10s%s\n' off "$(gettext "Turn off everything that can be: the security watcher, the virus scanner, the settings audit, real-time watching, network protection and the audit log. Nothing of it runs, the audit log from the next restart.")"
    printf '%-10s%s\n' default "$(gettext "Every switch back as the image ships it: network protection and real-time watching off, what looks for new threats off, the rest on, and found files reported.")"
    printf '%-10s%s\n' light "$(gettext "As default, and the cheapest of what looks for new threats: odd processes, SELinux, AI agents and developer tools recorded; ransomware, agents' settings and apps' permissions said.")"
    printf '%-10s%s\n' balanced "$(gettext "As light, all of those said, with secrets read, programs run from downloads and BPF recorded, the chains of steps put together, and the command-server lists daily.")"
    printf '%-10s%s\n' thorough "$(gettext "Everything that looks for new threats on: the decoy key, the YARA rules, the behaviour sensor (Falco, a few hundred megabytes) recording, odd DNS names and hunting rules, the command-server lists hourly. Costs memory, CPU and downloads; developers will see noise.")"
    echo "$(gettext "None of them turns on what sends something to somebody else: site checking, asking the agent about events and hash reputation are each chosen on their own.")"
    if [[ -z "${name}" ]]; then
        name="$(gum choose --header "$(gettext "Security profile")" off default light balanced thorough "${cancel}")" || exit 0
    fi
    case "${name}" in
        off)
            gum confirm "$(gettext "Turn every detection switch off?")" || exit 0 ;;
        default | light | balanced)
            gum confirm "$(eval_gettext "Set the security profile \${name}?")" || exit 0 ;;
        thorough)
            gum confirm "$(gettext "Turn on everything that looks for new threats?")" || exit 0 ;;
        *) exit 0 ;;
    esac
    sudo "${config}" profile "${name}"
    # Each unit reads its key as it starts, so a restart starts or stops it, and the rules and YARA sets are
    # fetched, or taken away, at once
    sudo systemctl restart clamav-freshclam.service clamd@scan.socket amethystora-clamav-onaccess.service \
        amethystora-ips.service amethystora-security-realtime.service amethystora-dns-names.service \
        amethystora-behaviour.service || true
    # A restart, since a run already under way has passed the sets this profile just turned off and would keep them
    sudo systemctl restart --no-block amethystora-ips-rules.service amethystora-yara.service || true
    sudo systemctl start --no-block amethystora-security-watch.service || true
    echo "$(gettext "Done. The audit log and its rules follow at the next restart.")"
    echo
    echo "$(gettext "A profile leaves alone what the image enforces from the start, each with its own switch:")"
    /usr/libexec/amethystora-hardening status
    echo "$(gettext "...and what you turned on yourself, such as ame security usb, dns or ransomware. ame security status shows each of them.")"

# Accept a check of the report that cannot be fixed on this machine, or stop a security feature saying what you know is fine
[positional-arguments]
allow FEATURE="report" ENTRY="" ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    helper=/usr/libexec/amethystora-security-allow
    feature="${1:-report}"
    cancel="$(gettext "Cancel")"
    if ! "${helper}" --features | grep -qxF -- "${feature}"; then
        features="$("${helper}" --features | tr '\n' ' ' || true)"
        echo "$(eval_gettext "These have an allowlist: \${features}")"
        exit 64
    fi
    if [[ "${feature}" != report ]]; then
        entry="${2:-}"
        if [[ -z "${entry}" ]]; then
            echo "$(eval_gettext "Allowed for \${feature} (ame security allow \${feature} ENTRY remove takes one back):")"
            "${helper}" list "${feature}" | /usr/libexec/amethystora-security-text --lines | sed 's/^/  /'
            exit 0
        fi
        shown="$(/usr/libexec/amethystora-security-text "${entry}")"
        if [[ "${3:-}" == remove ]]; then
            sudo "${helper}" remove "${feature}" "${entry}"
            echo "$(eval_gettext "Said again: \${shown}")"
            exit 0
        fi
        echo "$(eval_gettext "What \${feature} finds matching this is recorded, and no longer said:")"
        echo "  ${shown}"
        echo "$(gettext "Allow only what you know: whatever matches, malware included, goes unannounced from now on.")"
        gum confirm "$(gettext "Allow it?")" || exit 0
        sudo "${helper}" add "${feature}" "${entry}"
        echo "$(eval_gettext "Allowed. To take it back: ame security allow \${feature} ENTRY remove")"
        exit 0
    fi
    entry="${2:-}"
    accepted="$("${helper}" list report)"
    if [[ "${3:-}" == remove ]]; then
        line="$(awk -v id="${entry}" '$1 == id { print; exit }' <<<"${accepted}")"
        if [[ -z "${line}" ]]; then
            if [[ -n "${entry}" ]]; then
                echo "$(eval_gettext "\${entry} is not accepted on this machine.")"
            else
                echo "$(gettext "That check is not accepted on this machine.")"
            fi
            exit 0
        fi
        sudo "${helper}" remove report "${line}"
        echo "$(eval_gettext "Shown again: \${entry} is a warning while it needs your attention.")"
        exit 0
    fi
    if [[ -n "${accepted}" ]]; then
        echo "$(gettext "Accepted on this machine (ame security allow report ID remove shows one again):")"
        sed 's/^/  /' <<<"${accepted}"
        echo
    fi
    # The checks that need attention now, as the report gives them to this account
    open="$(/usr/libexec/amethystora-security-status --json |
        jq -r '.checks[] | select(.group != "optional" and .group != "sends" and (.state == "check" or .state == "off")) | "\(.id)\t\(.state)\t\(.text)"')"
    if [[ -z "${entry}" ]]; then
        if [[ -z "${open}" ]]; then
            echo "$(gettext "Nothing needs your attention.")"
            exit 0
        fi
        mapfile -t choices < <(cut -f1,3 <<<"${open}")
        choice="$(gum choose --header "$(gettext "Accept which?")" "${choices[@]}" "${cancel}")" || exit 0
        [[ -n "${choice}" && "${choice}" != "${cancel}" ]] || exit 0
        entry="${choice%%$'\t'*}"
    fi
    line="$(awk -F'\t' -v id="${entry}" '$1 == id { print; exit }' <<<"${open}")"
    if [[ -z "${line}" ]]; then
        echo "$(eval_gettext "\${entry} is not a check that needs your attention now.")"
        exit 0
    fi
    IFS=$'\t' read -r id state text <<<"${line}"
    echo "$(eval_gettext "\"\${text}\" stays in the report as information, out of the summary and never said, until it turns worse. Whatever causes it stays as it is.")"
    gum confirm "$(gettext "Accept it?")" || exit 0
    sudo "${helper}" add report "${id} ${state}"
    echo "$(eval_gettext "Accepted. ame security allow report \${id} remove shows it again.")"

# The image's kernel settings (standard), stricter ones (strict: no io_uring, debuggers for root only), or off
[positional-arguments]
kernel ACTION="":
    @/usr/libexec/amethystora-hardening ask kernel "${1:-}"

# Rarely used kernel modules kept from loading: old network protocols and filesystems, CAN, the floppy
[positional-arguments]
blocked-modules ACTION="":
    @/usr/libexec/amethystora-hardening ask blocked-modules "${1:-}"

# FireWire kept from loading, since its devices read and write memory by themselves; off for FireWire sound cards
[positional-arguments]
firewire ACTION="":
    @/usr/libexec/amethystora-hardening ask firewire "${1:-}"

# Kernel arguments, each on its own: the image's (init_on_alloc, vsyscall=none...) and stricter ones (init_on_free...)
[positional-arguments]
kernel-args ARGUMENT="" ACTION="":
    @/usr/libexec/amethystora-hardening ask kernel-args "${1:-}" "${2:-}"

# The Amethystora firewall zone, which lets in only what a desktop needs
[positional-arguments]
firewall ACTION="":
    @/usr/libexec/amethystora-hardening ask firewall "${1:-}"

# Blocking an address after repeated failed logins over SSH (fail2ban)
[positional-arguments]
failed-logins ACTION="":
    @/usr/libexec/amethystora-hardening ask failed-logins "${1:-}"

# Locking an account for ten minutes after ten wrong passwords in a row
[positional-arguments]
lockout ACTION="":
    @/usr/libexec/amethystora-hardening ask lockout "${1:-}"

# The image's override that takes X11, the input devices and the Flatpak service from every Flatpak app
[positional-arguments]
app-sandbox ACTION="":
    @/usr/libexec/amethystora-hardening ask app-sandbox "${1:-}"

# Keeping /dev/uinput, which makes virtual keyboards, from apps; Steam Input and drawing tablets need it
[positional-arguments]
virtual-input ACTION="":
    @/usr/libexec/amethystora-hardening ask virtual-input "${1:-}"

# The image's SSH server settings: no root login, three tries, no X11 forwarding
[positional-arguments]
ssh-settings ACTION="":
    @/usr/libexec/amethystora-hardening ask ssh-settings "${1:-}"

# Switching this machine back to signature-checked updates at every start
[positional-arguments]
signed-updates ACTION="":
    @/usr/libexec/amethystora-hardening ask signed-updates "${1:-}"

# Temporary IPv6 addresses, so that sites cannot follow this machine by one address
[positional-arguments]
ipv6-privacy ACTION="":
    @/usr/libexec/amethystora-hardening ask ipv6-privacy "${1:-}"

# Bluetooth's random address, and visible and pairable for 30 seconds at a time
[positional-arguments]
bluetooth-privacy ACTION="":
    @/usr/libexec/amethystora-hardening ask bluetooth-privacy "${1:-}"

# The clock set only from time servers that prove who they are (NTS), never one a network suggests
[positional-arguments]
time ACTION="":
    @/usr/libexec/amethystora-hardening ask time "${1:-}"

# A zone that answers nothing on networks you have not named home
[positional-arguments]
firewall-away ACTION="":
    @/usr/libexec/amethystora-hardening ask firewall-away "${1:-}"

# Which saved networks are home: the only ones phone pairing is open on
home-networks:
    @/usr/libexec/amethystora-hardening ask-home

# Avahi, ModemManager and passim off: no printer or device discovery, no mobile broadband, no firmware sharing
[positional-arguments]
network-daemons ACTION="":
    @/usr/libexec/amethystora-hardening ask network-daemons "${1:-}"

# Everything this machine sends by itself, to whom, and what stops working without it
sends:
    @/usr/libexec/amethystora-sends

# Fedora's count of its machines: once a week, no identifier
[positional-arguments]
countme ACTION="":
    @/usr/libexec/amethystora-hardening ask countme "${1:-}"

# NetworkManager's check of whether a network reaches the internet, which fetches a page of Fedora's
[positional-arguments]
connectivity-check ACTION="":
    @/usr/libexec/amethystora-hardening ask connectivity-check "${1:-}"

# The fetch of the firmware catalogue, once a day at most
[positional-arguments]
firmware-metadata ACTION="":
    @/usr/libexec/amethystora-hardening ask firmware-metadata "${1:-}"

# Whether apps may ask where this computer is (for this account)
[positional-arguments]
location ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    turn_on="$(gettext "Turn on")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    if [[ "$(gsettings get org.gnome.system.location enabled)" == true ]]; then
        echo "$(gettext "On: apps such as Weather, Maps and the automatic time zone may ask where this computer is.")"
    else
        echo "$(gettext "Off: no app can ask where this computer is.")"
    fi
    echo "$(gettext "When one asks, the Wi-Fi networks in reach and their signal go to BeaconDB (api.beacondb.net), and the address alone when none is in reach. Nothing is sent back to improve its map. This setting is this account's.")"
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Location services")" "${turn_on}" "${turn_off}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${turn_on}" | on)
            gsettings set org.gnome.system.location enabled true
            echo "$(gettext "On.")"
            ;;
        "${turn_off}" | off)
            gsettings set org.gnome.system.location enabled false
            echo "$(gettext "Off. Apps that need a place guess or ask you.")"
            ;;
        *) exit 0 ;;
    esac

# Bluetooth kept from loading at all
[positional-arguments]
block-bluetooth ACTION="":
    @/usr/libexec/amethystora-hardening ask block-bluetooth "${1:-}"

# The USB webcam driver kept from loading
[positional-arguments]
block-webcam ACTION="":
    @/usr/libexec/amethystora-hardening ask block-webcam "${1:-}"

# USB sticks and disks mounted only when opened in Files
[positional-arguments]
block-automount ACTION="":
    @/usr/libexec/amethystora-hardening ask block-automount "${1:-}"

# No installing GNOME extensions from extensions.gnome.org
[positional-arguments]
block-user-extensions ACTION="":
    @/usr/libexec/amethystora-hardening ask block-user-extensions "${1:-}"

# GNOME without Xwayland, so that no app can use X11
[positional-arguments]
block-xwayland ACTION="":
    @/usr/libexec/amethystora-hardening ask block-xwayland "${1:-}"

# Only the Flathub apps whose makers Flathub verified
[positional-arguments]
flathub-verified ACTION="":
    @/usr/libexec/amethystora-hardening ask flathub-verified "${1:-}"

# An administrator's password to install or remove Flatpak apps for the whole machine
[positional-arguments]
flatpak-password ACTION="":
    @/usr/libexec/amethystora-hardening ask flatpak-password "${1:-}"

# Nothing started from /tmp, /var/tmp or /dev/shm
[positional-arguments]
noexec-temp ACTION="":
    @/usr/libexec/amethystora-hardening ask noexec-temp "${1:-}"

# No copy of a crashed program's memory kept on disk
[positional-arguments]
no-coredumps ACTION="":
    @/usr/libexec/amethystora-hardening ask no-coredumps "${1:-}"

# npm, pnpm and uv refusing package versions published in the last few days
[positional-arguments]
package-cooldown ACTION="":
    @/usr/libexec/amethystora-hardening ask package-cooldown "${1:-}"

# Homebrew checking each bottle's build attestation; needs gh and a GitHub login
[positional-arguments]
brew-attestations ACTION="":
    @/usr/libexec/amethystora-hardening ask brew-attestations "${1:-}"

# Powering off once every desktop session has been locked for 18 hours, so that the disk keys leave memory
[positional-arguments]
locked-poweroff ACTION="":
    @/usr/libexec/amethystora-hardening ask locked-poweroff "${1:-}"

# A password before anyone edits a boot entry or opens the boot menu's command line
[positional-arguments]
boot-password ACTION="":
    @/usr/libexec/amethystora-hardening ask boot-password "${1:-}"

# The audit rules, locked until reboot, instead of the file-watch sensor: the same records, harder to
# stop, but every system call is a little slower
[positional-arguments]
audit-locked ACTION="":
    @/usr/libexec/amethystora-hardening ask audit-locked "${1:-}"

# VSCodium's extensions updated only by you, and only those you list (dx)
[positional-arguments]
vscodium-extensions ACTION="":
    @/usr/libexec/amethystora-hardening ask vscodium-extensions "${1:-}"

# The screen locked when the paired phone has left for a minute and nobody has touched the computer for a minute
# (WALK_AWAY in security.conf, off as the image ships): it never unlocks anything
[positional-arguments]
walk-away ACTION="":
    @ame security settings walk-away "${1:-}"

# What the security watcher noticed: accounts, what starts by itself, settings, keyboard readers
[positional-arguments]
events ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    if /usr/libexec/amethystora-security-config --is WATCHER off; then
        echo "$(gettext "The security watcher is turned off, so nothing new is looked for. ame security settings turns it on.")"
        echo
    elif [[ "${1:-}" == now ]]; then
        echo "$(gettext "Looking now...")"
        sudo systemctl start --wait amethystora-security-watch.service || true
    fi
    sudo /usr/libexec/amethystora-security-watch --show
    echo
    echo "$(gettext "Each line is something that changed, with the program that changed it. If you did it, or installed something that did, there is nothing to do. The full audit record of one kind:")"
    echo "sudo ausearch -i -k <$(/usr/libexec/amethystora-security-watch --keys)>"
    echo "$(gettext "and, for the kinds the file-watch sensor records, -if /var/log/amethystora-sensor/audit.log as well.")"
    # Seen is seen: what was just shown stops being new, for the report and the next login
    sudo /usr/libexec/amethystora-security-watch --seen

# Connections network protection blocked or noticed, and how to allow one again
[positional-arguments]
connections ACTION="" RULE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    ips=/usr/libexec/amethystora-ips
    case "${1:-}" in
        allow | block)
            if [[ -z "${2:-}" ]]; then
                echo "usage: ame security connections ${1:-} <rule number>"
                exit 64
            fi
            sudo "${ips}" "${1:-}" "${2:-}"
            ;;
        "")
            protection="$(/usr/libexec/amethystora-security-config NETWORK || true)"
            echo "$(eval_gettext "Network protection: \${protection} (ame security settings network)")"
            echo
            # The watcher is what reads Suricata's log, so what it saw a moment ago is shown too
            sudo systemctl start --wait amethystora-security-watch.service || true
            # Host names and rule messages come from the network: shown so that none can act on the terminal
            sudo "${ips}" blocked | /usr/libexec/amethystora-security-text --lines
            echo
            echo "$(gettext "If a connection was yours and wanted, allow its rule again with:")"
            echo "  ame security connections allow <rule number>"
            echo "$(gettext "That leaves the one rule out on this machine; ame security connections block <rule number> puts it back. The rules allowed here are listed in /etc/amethystora/ips-allowed.")"
            ;;
        *)
            echo "usage: ame security connections [allow <rule number> | block <rule number>]"
            exit 64
            ;;
    esac

# Addresses fail2ban has blocked for too many failed logins, and how to let one back in
[positional-arguments]
addresses ADDRESS="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # fail2ban runs only while SSH is on (its sshd jail): with it stopped, nothing is blocked
    if ! systemctl is-active --quiet fail2ban.service; then
        echo "$(gettext "fail2ban is not running, so no address is blocked. It runs while SSH is on.")"
        exit 0
    fi
    if [[ -n "${1:-}" ]]; then
        sudo fail2ban-client set sshd unbanip "${1:-}"
        exit 0
    fi
    sudo fail2ban-client status sshd
    echo
    echo "$(gettext "Let one back in with: ame security addresses <address>")"
    echo "$(gettext "Blocking is set up in /etc/fail2ban/jail.d/10-amethystora.conf.")"

# Turn key remapping on or off. It is off by default: the service reads every key pressed on the machine
[positional-arguments]
input-remapper ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    turn_on="$(gettext "Turn on")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    if systemctl is-enabled --quiet input-remapper.service 2>/dev/null; then
        echo "$(gettext "Key remapping is on.")"
    else
        echo "$(gettext "Key remapping is off. Input Remapper's window still opens, but cannot apply a mapping.")"
    fi
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Key remapping (Input Remapper)")" "${turn_on}" "${turn_off}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${turn_on}" | on)
            echo
            echo "$(gettext "The service runs as root and reads every input device on this machine, which is every key typed into every application, password prompts included. That is what remapping a key needs, and it is why this is off unless you ask for it.")"
            gum confirm "$(gettext "Turn key remapping on?")" || exit 0
            sudo /usr/libexec/amethystora-security-config note key-remapping on
            sudo systemctl enable --now input-remapper.service
            echo "$(gettext "On. Map keys in the Input Remapper application.")"
            ;;
        "${turn_off}" | off)
            sudo /usr/libexec/amethystora-security-config note key-remapping off
            sudo systemctl disable --now input-remapper.service
            echo "$(gettext "Off. Saved mappings stay on disk and come back if you turn it on again.")"
            ;;
        *) exit 0 ;;
    esac

# Refuse USB devices plugged in while the screen is locked, or any not plugged in at setup, against keyloggers and keystroke injectors
[positional-arguments]
usb ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    block_locked="$(gettext "Block new devices only while the screen is locked")"
    block_all="$(gettext "Block every new device until I allow it")"
    allow_device="$(gettext "Allow a blocked device")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    # Locked whenever USBGuard's D-Bus service may run, as the report counts it: unmasked by hand under "on",
    # GNOME's plugin would let every new device through while the screen is unlocked
    dbus="$(systemctl is-enabled usbguard-dbus.service 2>/dev/null || true)"
    if ! systemctl is-enabled --quiet usbguard.service 2>/dev/null; then
        mode=off
        echo "$(gettext "USB protection is off: any USB device plugged in is trusted.")"
    elif [[ "${dbus}" == masked* || "${dbus}" == not-found || -z "${dbus}" ]]; then
        mode=on
        echo "$(gettext "USB protection is on: a new USB device is blocked until you allow it. Devices allowed so far:")"
        sudo usbguard list-devices --allowed | sed 's/^/  /'
    else
        mode=locked
        echo "$(gettext "USB protection is on while the screen is locked: a USB device plugged in then is blocked.")"
    fi
    # GNOME's part stopped and masked, so that nothing starts it again (17-cleanup.sh). A masked unit keeps
    # the links that enabled it, so it is disabled first.
    dbus_off() {
        if [[ "$(systemctl is-enabled usbguard-dbus.service 2>/dev/null)" != masked ]]; then
            sudo systemctl disable --now usbguard-dbus.service
        fi
        sudo systemctl mask usbguard-dbus.service
    }
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "USB protection (USBGuard)")" \
            "${block_locked}" "${block_all}" \
            "${allow_device}" "${turn_off}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${block_locked}" | locked)
            echo
            echo "$(gettext "A USB device can say it is a keyboard and type by itself. While the screen is locked, a USB device plugged in is blocked, so that nobody can plug one into a locked machine:")"
            echo "  - $(gettext "a plain keyboard or mouse still works, so that you can unlock, with a notification;")"
            echo "  - $(gettext "anything else is blocked: a USB stick, a network adapter, a keyboard that is also something else. Plug it in again once you have unlocked, and it works.")"
            echo "$(gettext "What is plugged in at boot, and what you plug in while the screen is unlocked, works as before. The login screen, and another account's unlocked session, count as unlocked.")"
            gum confirm "$(gettext "Block new USB devices while the screen is locked?")" || exit 0
            # One rule, which allows every device: GNOME blocks what is new only while the screen is locked
            echo 'allow id *:* label "amethystora-usb-locked"' | sudo tee /etc/usbguard/rules.conf >/dev/null
            sudo chmod 0600 /etc/usbguard/rules.conf
            sudo sed -i -E 's|^#?IPCAllowedGroups=.*|IPCAllowedGroups=wheel|' /etc/usbguard/usbguard-daemon.conf
            sudo /usr/libexec/amethystora-security-config note usb-protection locked
            sudo systemctl unmask usbguard-dbus.service
            sudo systemctl enable usbguard.service usbguard-dbus.service
            sudo systemctl restart usbguard.service
            sudo systemctl start usbguard-dbus.service
            echo
            echo "$(gettext "On. A USB device plugged in while the screen is locked is blocked.")"
            ;;
        "${block_all}" | on)
            echo
            echo "$(gettext "A USB device can say it is a keyboard and type by itself, and a keylogger can sit between a real keyboard and this machine. USBGuard blocks anything that was not already plugged in, until you allow it by hand.")"
            echo
            echo "$(gettext "Plug in everything you use first - keyboard, mouse, dock, webcam, headset, security keys. What is connected now is what gets allowed.")"
            gum confirm "$(gettext "Everything plugged in?")" || exit 0
            # Before the policy, so that no session's plugin adds its rule that allows every new device
            dbus_off
            # Built from what is connected right now, so the keyboard running this command is allowed
            usbguard generate-policy | sudo tee /etc/usbguard/rules.conf >/dev/null
            sudo chmod 0600 /etc/usbguard/rules.conf
            # Without this only root could list or allow a device, and the prompt below would not work
            sudo sed -i -E 's|^#?IPCAllowedGroups=.*|IPCAllowedGroups=wheel|' /etc/usbguard/usbguard-daemon.conf
            sudo /usr/libexec/amethystora-security-config note usb-protection on
            sudo systemctl enable usbguard.service
            # Restarted rather than started: a rule a session added while the screen-lock mode was on goes
            sudo systemctl restart usbguard.service
            echo
            echo "$(gettext "On. A USB device plugged in from now on is blocked until you allow it here.")"
            ;;
        "${allow_device}" | allow)
            if ! systemctl is-active --quiet usbguard.service; then
                echo "$(gettext "USB protection is off, so nothing is being blocked.")"
                exit 1
            fi
            if [[ "${mode}" == locked ]]; then
                echo "$(gettext "A device blocked while the screen was locked works once you plug it in again.")"
            fi
            blocked="$(sudo usbguard list-devices --blocked)"
            if [[ -z "${blocked}" ]]; then
                echo "$(gettext "Nothing is blocked.")"
                exit 0
            fi
            choice="$(gum choose --header "$(gettext "Allow which device?")" --height 12 <<<"${blocked}")" || exit 0
            [[ -n "${choice}" ]] || exit 0
            if [[ "${mode}" == locked ]]; then
                sudo usbguard allow-device "${choice%%:*}"
                echo "$(gettext "Allowed.")"
            else
                # -p writes it into the rules, so the device is still allowed after a reboot
                sudo usbguard allow-device -p "${choice%%:*}"
                echo "$(gettext "Allowed, now and after a reboot.")"
            fi
            ;;
        "${turn_off}" | off)
            sudo /usr/libexec/amethystora-security-config note usb-protection off
            dbus_off
            sudo systemctl disable --now usbguard.service
            echo "$(gettext "Off. Every USB device is trusted again.")"
            ;;
        *) exit 0 ;;
    esac

# A folder in your home, Private, encrypted on its own, which closes whenever the screen locks
[positional-arguments]
private-folder ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    helper=/usr/libexec/amethystora-private-folder
    action="${1:-}"
    make_one="$(gettext "Make one")"
    # Translators: a choice about the private folder, which is closed: open it.
    open_it="$(gettext "Open")"

    # Translators: a choice about the private folder, which is open: close it.
    close_it="$(gettext "Close")"

    change_password="$(gettext "Change its password")"
    take_away="$(gettext "Take it away")"
    cancel="$(gettext "Cancel")"
    header="$(gettext "Private folder")"
    "${helper}" status
    if [[ -z "${action}" ]]; then
        case "$("${helper}" status --json | jq -r .state)" in
            none) action="$(gum choose --header "${header}" "${make_one}" "${cancel}")" ;;
            closed) action="$(gum choose --header "${header}" "${open_it}" "${change_password}" "${take_away}" "${cancel}")" ;;
            *) action="$(gum choose --header "${header}" "${close_it}" "${change_password}" "${take_away}" "${cancel}")" ;;
        esac
    fi
    case "${action}" in
        "${make_one}" | create) exec "${helper}" create ;;
        "${open_it}" | open) exec "${helper}" open --show ;;
        "${close_it}" | close) exec "${helper}" close ;;
        "${change_password}" | password) exec "${helper}" password ;;
        "${take_away}" | remove) exec "${helper}" remove ;;
        *) exit 0 ;;
    esac

# Let browsers install only the extensions on a list, keep web pages from USB, serial, HID and Bluetooth devices, and turn off what browsers send to their makers
[positional-arguments]
browser ACTION="" ID="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    policy=/usr/libexec/amethystora-browser-policy
    turn_on="$(gettext "Turn on")"
    allow_extension="$(gettext "Allow an extension")"
    disallow_extension="$(gettext "Take an extension off the list")"
    add_osprey="$(gettext "Add site checking (Osprey)")"
    take_osprey="$(gettext "Take site checking away")"
    stop_data="$(gettext "Turn data collection off")"
    allow_data="$(gettext "Let browsers collect data again")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    if [[ "$("${policy}" status)" == on ]]; then
        echo "$(gettext "Browser protection is on: browsers install only the extensions on the list.")"
    else
        echo "$(gettext "Browser protection is off: a browser installs any extension you let it.")"
    fi
    if [[ "$("${policy}" status privacy)" == on ]]; then
        echo "$(gettext "Data collection is off: browsers do not send usage data, studies or sponsored-content pings to their makers.")"
    else
        echo "$(gettext "Data collection is on, as each browser ships it.")"
    fi
    allowed="$("${policy}" allowed)"
    if [[ -n "${allowed}" ]]; then
        echo "$(gettext "Allowed on this machine, as well as the password managers:")"
        sed 's/^/  /' <<<"${allowed}"
    fi
    if /usr/libexec/amethystora-security-config --is OSPREY on; then
        echo "$(gettext "Site checking (Osprey) is allowed in browsers.")"
    fi
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Browser protection")" \
            "${turn_on}" "${allow_extension}" "${disallow_extension}" "${add_osprey}" \
            "${take_osprey}" "${stop_data}" "${allow_data}" "${turn_off}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${turn_on}" | on)
            echo
            echo "$(gettext "An extension sees every page you open, which is how most password stealers arrive.")"
            echo "$(gettext "With this on, browsers install only the Bitwarden, 1Password, Proton Pass and KeePassXC password managers and the extensions you allow here, and web pages are kept from USB, serial, HID and Bluetooth devices.")"
            echo
            echo "$(gettext "It covers Firefox, Brave, Chrome, Chromium and Edge, for every account on this machine.")"
            echo "$(gettext "Extensions that are not on the list go: Firefox removes them, with their settings, the next time it starts, and the others turn theirs off. Allow the ones you use first.")"
            gum confirm "$(gettext "Turn browser protection on?")" || exit 0
            sudo /usr/libexec/amethystora-security-config note browser-protection on
            sudo "${policy}" on
            echo "$(gettext "On. Each browser takes it up the next time it starts.")"
            ;;
        "${allow_extension}" | allow)
            id="${2:-}"
            if [[ -z "${id}" ]]; then
                echo "$(gettext "Firefox lists each extension's ID in about:support, under Add-ons. The others show it on their extensions page with developer mode on.")"
                id="$(gum input --prompt "$(gettext "Extension ID")> " --placeholder "uBlock0@raymondhill.net")" || exit 0
            fi
            [[ -n "${id}" ]] || exit 0
            sudo "${policy}" allow "${id}"
            echo "$(eval_gettext "Allowed. To take it back: ame security browser disallow \${id}")"
            ;;
        "${disallow_extension}" | disallow)
            id="${2:-}"
            if [[ -z "${id}" ]]; then
                [[ -n "${allowed}" ]] || { echo "$(gettext "No extension of this machine's own is on the list.")"; exit 0; }
                mapfile -t ids <<<"${allowed}"
                id="$(gum choose --header "$(gettext "Take which off?")" "${ids[@]}" "${cancel}")" || exit 0
                [[ -n "${id}" && "${id}" != "${cancel}" ]] || exit 0
            fi
            sudo "${policy}" disallow "${id}"
            echo "$(gettext "Off the list. With protection on, browsers turn it off at their next start, and Firefox removes it.")"
            ;;
        "${add_osprey}" | osprey)
            if [[ "${2:-on}" == off ]]; then
                sudo /usr/libexec/amethystora-security-config set OSPREY off
                sudo /usr/libexec/amethystora-security-config note site-checking off
                sudo "${policy}" refresh
                if [[ "$("${policy}" status)" == on ]]; then
                    echo "$(gettext "Site checking is off: browsers turn Osprey off at their next start, and Firefox removes it.")"
                else
                    echo "$(gettext "Site checking is off. Browser protection is off too, so remove Osprey yourself where it is installed: Firefox's Add-ons and themes, or the other browsers' extensions page.")"
                fi
                exit 0
            fi
            echo
            echo "$(gettext "Osprey: Browser Protection checks each site you open against more than twenty threat-intelligence providers, and warns you or blocks the site. Amethystora does not make it: Osprey Project LLC does, under the GPL-3.0, and you install it from your browser's own store.")"
            echo
            echo "$(gettext "It sends the address of every site you open to Osprey Project LLC, in California, which passes it on to its providers without the query string and without your address. That is why it is off until you choose it, and why no profile turns it on.")"
            gum confirm "$(gettext "Allow Osprey in your browsers?")" || exit 0
            sudo /usr/libexec/amethystora-security-config set OSPREY on
            sudo /usr/libexec/amethystora-security-config note site-checking on
            sudo "${policy}" refresh
            echo
            echo "$(gettext "Allowed. Install it from your browser's store; nothing installs it for you:")"
            echo "  Firefox:                https://addons.mozilla.org/firefox/addon/osprey-browser-protection/"
            echo "  Chrome, Chromium, Brave: https://chromewebstore.google.com/detail/jmnpibhfpmpfjhhkmpadlbgjnbhpjgnd"
            echo "  Edge:                   https://microsoftedge.microsoft.com/addons/detail/nopglhplnghfhpniofkcopmhbjdonlgn"
            xdg-open "https://addons.mozilla.org/firefox/addon/osprey-browser-protection/" >/dev/null 2>&1 || true
            echo "$(gettext "Its own settings, which providers it asks, are Osprey's: Amethystora sets none of them.")"
            ;;
        "${take_osprey}")
            exec ame security browser osprey off
            ;;
        "${stop_data}" | privacy)
            if [[ "${2:-on}" == off ]]; then
                sudo /usr/libexec/amethystora-security-config note browser-privacy off
                sudo "${policy}" privacy off
                echo "$(gettext "Data collection is back as each browser ships it, from its next start.")"
                exit 0
            fi
            echo
            echo "$(gettext "Firefox, as Fedora builds it, sends Mozilla usage data, takes part in studies and shows sponsored shortcuts and stories on its new tab page. Chrome, Chromium, Brave and Edge send their makers usage and crash reports too.")"
            echo "$(gettext "With this on, they stop: Firefox's telemetry and studies, sponsored shortcuts, stories and suggestions, and the usage, survey and personalisation reports of the others. Nothing is installed, no page is changed, and what you search for goes where it went.")"
            echo
            echo "$(gettext "Firefox then says it is managed by your organization and greys out its data settings. It covers Firefox, Brave, Chrome, Chromium and Edge, for every account on this machine, from their next start.")"
            gum confirm "$(gettext "Turn browsers' data collection off?")" || exit 0
            sudo /usr/libexec/amethystora-security-config note browser-privacy on
            sudo "${policy}" privacy on
            echo "$(gettext "Off. Each browser takes it up the next time it starts.")"
            ;;
        "${allow_data}")
            exec ame security browser privacy off
            ;;
        "${turn_off}" | off)
            sudo /usr/libexec/amethystora-security-config note browser-protection off
            sudo "${policy}" off
            echo "$(gettext "Off. Browsers install any extension again; the ones Firefox removed do not come back.")"
            ;;
        *) exit 0 ;;
    esac

# Look up names over DNS-over-TLS with a resolver you choose, instead of in plain text with the network's
[positional-arguments]
dns ACTION="" *SERVERS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # The switch is the two files this helper writes, and its list of resolvers is the only one
    helper=/usr/libexec/amethystora-dns
    action="${1:-}"
    [[ $# -gt 0 ]] && shift
    servers=("$@")
    own_servers="$(gettext "Servers of my own")"
    pause_ten="$(gettext "Pause for ten minutes")"
    end_pause="$(gettext "End the pause")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    if [[ "$("${helper}" status)" == on ]]; then
        resolver="$("${helper}" name)"
        echo "$(eval_gettext "Encrypted DNS is on: names are looked up with \${resolver}, over DNS-over-TLS.")"
        if "${helper}" paused; then
            echo "$(gettext "It is paused: the network's own DNS servers answer, in plain text, for up to ten minutes.")"
        fi
    else
        echo "$(gettext "Encrypted DNS is off: names are looked up with the DNS servers the network hands out, in plain text, so the network sees each name and can answer what it likes.")"
    fi
    mapfile -t resolvers < <("${helper}" list)
    if [[ -z "${action}" ]]; then
        options=()
        for line in "${resolvers[@]}"; do
            IFS=$'\t' read -r _ name who _ <<<"${line}"
            options+=("$(eval_gettext "\${name}, run by \${who}")")
        done
        choice="$(gum choose --header "$(gettext "Encrypted DNS: send every lookup to")" "${options[@]}" \
            "${own_servers}" "${pause_ten}" "${end_pause}" "${turn_off}" "${cancel}")" || exit 0
        case "${choice}" in
            "${own_servers}") action=custom ;;
            "${pause_ten}") action=pause ;;
            "${end_pause}") action=resume ;;
            "${turn_off}") action=off ;;
            *)
                for line in "${resolvers[@]}"; do
                    IFS=$'\t' read -r id name who _ <<<"${line}"
                    [[ "${choice}" == "$(eval_gettext "\${name}, run by \${who}")" ]] && action="${id}"
                done
                ;;
        esac
    fi
    case "${action}" in
        "" | Cancel) exit 0 ;;
        off)
            sudo /usr/libexec/amethystora-security-config note encrypted-dns off
            sudo "${helper}" off
            systemctl --user stop amethystora-dns-portal.service 2>/dev/null || true
            echo "$(gettext "Off. Names are looked up with the network's own DNS servers again.")"
            exit 0
            ;;
        pause)
            echo "$(gettext "For ten minutes, the network's own DNS servers answer, in plain text, so that a login page can load. It ends by itself, or with: ame security dns resume")"
            sudo "${helper}" pause
            echo "$(gettext "Paused. Log in to the network's page now.")"
            exit 0
            ;;
        resume)
            sudo "${helper}" resume
            echo "$(gettext "Encrypted again.")"
            exit 0
            ;;
        custom)
            if [[ ${#servers[@]} -eq 0 ]]; then
                echo "$(gettext "Each server as ADDRESS#NAME, NAME being what its certificate is for, separated by spaces.")"
                input="$(gum input --prompt "$(gettext "Servers")> " --placeholder "9.9.9.9#dns.quad9.net")" || exit 0
                read -ra servers <<<"${input}"
            fi
            [[ ${#servers[@]} -gt 0 ]] || exit 0
            name="${servers[*]}"
            who=""
            note="-"
            ;;
        *)
            line=""
            for candidate in "${resolvers[@]}"; do
                [[ "${candidate%%$'\t'*}" == "${action}" ]] && line="${candidate}"
            done
            if [[ -z "${line}" ]]; then
                echo "usage: ame security dns [quad9|mullvad|cloudflare|google|custom ADDRESS#NAME...|pause|resume|off]"
                exit 64
            fi
            IFS=$'\t' read -r _ name who note _ <<<"${line}"
            servers=()
            ;;
    esac
    echo
    echo "$(eval_gettext "Every name this machine looks up then goes to \${name}, encrypted, and to nobody else.")"
    [[ -z "${who}" ]] || echo "$(eval_gettext "\${name} is run by \${who}.")"
    echo "$(gettext "It sees every name, and the network you are on sees none of them and cannot answer in its place.")"
    [[ "${note}" == "-" ]] || echo "${note}"
    echo
    echo "$(gettext "Names on the network itself, such as a printer's, still go to the network's own servers, and so does the check of whether a network reaches the internet. A VPN's own names keep working, and Tailscale's. Firefox asks this resolver too, unless DNS over HTTPS is turned on in its own settings.")"
    echo
    echo "$(gettext "A network that wants a login first, as in hotels and on trains, cannot show its page while this is on. A notification offers to pause it then: ame security dns pause.")"
    gum confirm "$(eval_gettext "Send every lookup to \${name}?")" || exit 0
    sudo /usr/libexec/amethystora-security-config note encrypted-dns on
    sudo "${helper}" on "${action}" "${servers[@]}"
    # What offers the pause on a network that wants a login first, from now rather than the next login
    systemctl --user start amethystora-dns-portal.service 2>/dev/null || true
    echo "$(gettext "On. To go back to the network's own DNS servers: ame security dns off")"

# Give each network its own hardware address for this machine, and keep the machine's name from them
[positional-arguments]
mac ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    # The switch is the one file this helper copies into /etc/NetworkManager/conf.d
    helper=/usr/libexec/amethystora-mac
    turn_away="$(gettext "Only away from home")"
    turn_on="$(gettext "Every network")"
    choose_home="$(gettext "Choose home networks")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    case "$("${helper}" status)" in
        away) echo "$(gettext "On away from home: the networks you named home see this machine's own hardware address and its name, every other network an address of its own and not its name.")" ;;
        on) echo "$(gettext "On: each network sees an address of its own for this machine, and not its name.")" ;;
        *) echo "$(gettext "Off: wired networks see this machine's own hardware address, Wi-Fi networks one for each network name, and every network is told the machine's name.")" ;;
    esac
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "A different address on each network")" "${turn_away}" "${turn_on}" "${choose_home}" "${turn_off}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${choose_home}" | home)
            /usr/libexec/amethystora-hardening ask-home
            ;;
        "${turn_away}" | "${turn_on}" | away | on)
            mode=on
            [[ "${action}" == "${turn_away}" || "${action}" == away ]] && mode=away
            echo
            echo "$(gettext "A wired network knows a computer by its hardware (MAC) address, the same on every network it joins, and every network is told its name, and over IPv6 an identity made from it. With this on, each Wi-Fi and wired network sees an address and an identity of its own for this machine, the same each time it comes back, so a router keeps what it reserved for it and a login page remembers it. No network is told its name.")"
            echo
            echo "$(gettext "Wi-Fi networks already see an address of their own for each network name without this. A wired network cannot be told apart before it connects, so every wired network without a saved connection of its own sees the same made-up address: not the hardware's, but the same in an office and in a hotel.")"
            echo
            echo "$(gettext "A network that lets in only registered devices, as some universities and offices do, then refuses it. For that network, open it in Settings, then Identity, and set Cloned Address to Permanent.")"
            echo
            echo "$(gettext "Avahi still announces this machine's name on every network, as it can be told about interfaces and not networks. ame security network-daemons turns it off.")"
            if [[ "${mode}" == away ]]; then
                if [[ ! -s /etc/amethystora/firewall-home ]]; then
                    echo
                    echo "$(gettext "No network is named home yet, so this would be the same as every network. Name yours first, to keep your router's reservation for this machine:")"
                    /usr/libexec/amethystora-hardening ask-home
                    [[ -s /etc/amethystora/firewall-home ]] || exit 0
                fi
                echo
                echo "$(gettext "The networks you named home, with ame security home-networks, keep this machine's own hardware address and are told its name, so that a router's reservation and Wake-on-LAN keep working. A wired connection named home is the one used on every wired network.")"
                gum confirm "$(gettext "Use a different address on each network, except at home?")" || exit 0
            else
                gum confirm "$(gettext "Use a different address on each network?")" || exit 0
            fi
            sudo /usr/libexec/amethystora-security-config note network-address "${mode}"
            sudo "${helper}" "${mode}"
            echo "$(gettext "On, from the next time each network connects. Turn Wi-Fi off and on to start now.")"
            ;;
        "${turn_off}" | off)
            sudo /usr/libexec/amethystora-security-config note network-address off
            sudo "${helper}" off
            echo "$(gettext "Off, from the next time each network connects.")"
            ;;
        *) exit 0 ;;
    esac

# Unlock the encrypted disk with this machine's TPM instead of typing the passphrase at every boot
[positional-arguments]
disk-unlock ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    action="${1:-}"
    set_up_pin="$(gettext "Set up with a PIN")"
    remove_key="$(gettext "Remove the TPM key")"
    cancel="$(gettext "Cancel")"
    # The encrypted partitions this system runs from: every disk under its own filesystem, and never a
    # pool's, whose disks open with a key file of their own
    mapfile -t DEVICES < <(/usr/libexec/amethystora-raid root-luks)
    if [[ ${#DEVICES[@]} -eq 0 ]]; then
        echo "$(gettext "The system is not on an encrypted partition, so there is nothing to unlock.")"
        echo "$(gettext "Disk encryption is chosen while installing and cannot be turned on afterwards.")"
        exit 1
    fi
    if [[ ! -e /dev/tpmrm0 ]]; then
        echo "$(gettext "This machine has no usable TPM (there is no /dev/tpmrm0), so the key has nowhere to go.")"
        exit 1
    fi
    for DEVICE in "${DEVICES[@]}"; do
        echo "$(eval_gettext "Encrypted partition: \${DEVICE}")"
        if sudo cryptsetup luksDump "${DEVICE}" | grep -q "systemd-tpm2"; then
            echo "$(gettext "The TPM already holds a key for it. Setting it up again replaces that key.")"
        else
            echo "$(gettext "The TPM holds no key for it: the passphrase is typed at every boot.")"
        fi
    done
    echo
    echo "$(gettext "What this does: the TPM hands over the key only while Secure Boot is on and the enrolled signing keys are the ones enrolled now (PCR 7 and 14). Turning Secure Boot off, or adding a key to sign something of your own with, stops the disk opening.")"
    if [[ -e /usr/share/amethystora/sealed ]]; then
        echo "$(gettext "This machine is sealed: its initramfs is inside the UKI that the enrolled key signs, so it cannot be rewritten without the machine refusing to start it. But neither PCR measures the disk the system starts from, nor /etc on it: someone with a disk of their own made to look like this one starts this same signed system on it, with their own settings, and without a PIN the TPM would hand that system the key. So it is set up only with a PIN here too.")"
    else
        echo "$(gettext "What it does not do: it does not measure the boot menu or the initramfs, which are plain files on this image and change with every kernel update. Someone who takes the machine could start it into a shell from the boot menu, or with a disk of their own made to look like this one, and neither changes PCR 7 or 14: without a PIN, the TPM would hand them the key. So here it is set up only with a PIN. That does not stop someone who rewrites the initramfs and gives the machine back: theirs can ask for the PIN itself and keep it once you type it.")"
    fi
    echo
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "TPM disk unlock")" "${set_up_pin}" "${remove_key}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${set_up_pin}" | on-pin) FLAGS=(--tpm2-with-pin=yes) ;;
        on)
            echo "$(gettext "Only with a PIN: ame security disk-unlock on-pin")"
            exit 1
            ;;
        "${remove_key}" | off)
            for DEVICE in "${DEVICES[@]}"; do
                sudo systemd-cryptenroll --wipe-slot=tpm2 "${DEVICE}"
            done
            echo "$(gettext "Removed. The passphrase is asked for at every boot again.")"
            # The watcher reads the LUKS header for the report's Disk unlock check
            sudo systemctl start --no-block amethystora-security-watch.service || true
            exit 0
            ;;
        *) exit 0 ;;
    esac
    if ! mokutil --sb-state 2>/dev/null | grep -qi enabled; then
        echo "$(gettext "Secure Boot is off on this machine, and binding to it is the whole of what this protects.")"
        echo "$(gettext 'Turn Secure Boot on and run `ame security secure-boot` first.')"
        gum confirm "$(gettext "Enrol anyway?")" || exit 0
    fi
    echo "$(gettext "Type the disk passphrase when asked, once for each encrypted disk of the system. It keeps working afterwards: this adds a second way in, not a replacement, so a cleared TPM or a replaced board does not lock you out of your disk.")"
    # Any key the TPM held before goes once this one is in (never the new one): one enrolled with other
    # PCRs, or none, as an older unlocker did, would otherwise open the disk without these checks
    for DEVICE in "${DEVICES[@]}"; do
        sudo systemd-cryptenroll --wipe-slot=tpm2 --tpm2-device=auto --tpm2-pcrs=7+14 "${FLAGS[@]}" "${DEVICE}"
    done
    # The initramfs is built without this machine's own settings in it, so the instruction to ask the
    # TPM has to come from the kernel command line. Without it the enrolled key is never used. A sealed
    # image needs none, and could take none: systemd finds its root partition by type and, started from
    # a measured UKI, asks the TPM by itself (tpm2-device=auto).
    if [[ ! -e /usr/share/amethystora/sealed ]]; then
        sudo rpm-ostree kargs --append-if-missing=rd.luks.options=tpm2-device=auto
    fi
    sudo systemctl start --no-block amethystora-security-watch.service || true
    echo
    echo "$(gettext "Done. It takes effect at the next boot.")"

# Hourly read-only snapshots of the home folders, which ransomware running as you cannot change or delete
[positional-arguments]
ransomware ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    SNAPSHOTS=/var/home/.snapshots
    action="${1:-}"
    turn_on="$(gettext "Turn on")"
    put_back="$(gettext "Put files back")"
    turn_off="$(gettext "Turn off")"
    cancel="$(gettext "Cancel")"
    # Newest first, by name, which is the time each was taken
    mapfile -t TAKEN < <(find "${SNAPSHOTS}" -mindepth 1 -maxdepth 1 -regextype egrep \
        -regex '.*/[0-9]{4}-[0-9]{2}-[0-9]{2}_[0-9]{4}' -printf '%f\n' 2>/dev/null | sort -r)
    count="${#TAKEN[@]}"
    if systemctl is-enabled --quiet amethystora-home-snapshot.timer 2>/dev/null; then
        if [[ "${count}" -gt 0 ]]; then
            latest="${TAKEN[0]}"
            echo "$(eval_ngettext "Ransomware protection is on: \${count} snapshot, the latest \${latest}." "Ransomware protection is on: \${count} snapshots, the latest \${latest}." "${count}")"
        else
            echo "$(eval_gettext "Ransomware protection is on: \${count} snapshots, the latest not taken yet.")"
        fi
    else
        echo "$(gettext "Ransomware protection is off.")"
    fi
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Ransomware protection (snapshots of the home folders)")" \
            "${turn_on}" "${put_back}" "${turn_off}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${turn_on}" | on)
            if [[ "$(stat -f -c %T /var/home)" != btrfs || "$(stat -c %i /var/home)" != 256 ]]; then
                echo "$(gettext "The home folders are not on a btrfs subvolume of their own, so they cannot be snapshotted. The installer sets them up that way unless the disk was partitioned by hand.")"
                exit 1
            fi
            echo
            echo "$(gettext "Every hour, a read-only snapshot of every home folder on this machine. Ransomware running as you can encrypt everything you can write, and a snapshot is not something you can write: changing or deleting one takes root. Kept: every snapshot from the last day, and one a day for two weeks.")"
            echo
            echo "$(gettext "The cost is disk space. A snapshot holds on to what was deleted or changed since, so a file you delete to make room frees its space only once the last snapshot that has it is gone, up to two weeks later. Snapshots are never deleted to make room, because ransomware that rewrites every file would then delete the ones from before it.")"
            echo
            echo "$(gettext "What this does not survive is ransomware that gets root. An append-only backup somewhere else does: ame backup")"
            gum confirm "$(gettext "Turn ransomware protection on?")" || exit 0
            sudo /usr/libexec/amethystora-security-config note ransomware-protection on
            sudo systemctl enable --now amethystora-home-snapshot.timer
            sudo systemctl start amethystora-home-snapshot.service
            # Real-time scanning lists the folders it leaves out as it starts, and the snapshots are one
            if systemctl is-active --quiet amethystora-clamav-onaccess.service; then
                sudo systemctl restart amethystora-clamav-onaccess.service
            fi
            echo
            echo "$(gettext "On. Put files back with: ame security ransomware restore")"
            ;;
        "${put_back}" | restore)
            home="$(realpath "${HOME}")"
            if [[ "${home}" != /var/home/* ]]; then
                echo "$(eval_gettext "Your home folder, \${home}, is not one the snapshots cover.")"
                exit 1
            fi
            # The snapshots that hold your home folder, newest first, as the Backups window lists them
            mapfile -t YOURS < <(/usr/libexec/amethystora-restore points --local --json | jq -r '.points[].id')
            if [[ ${#YOURS[@]} -eq 0 ]]; then
                echo "$(gettext "There are no snapshots to put files back from.")"
                exit 1
            fi
            echo "$(gettext "Backups, in the app grid, shows each folder as it was and puts back single files.")"
            snapshot="$(gum choose --header "$(gettext "From which snapshot? (date_time)")" --height 16 "${YOURS[@]}")" || exit 0
            [[ -n "${snapshot}" ]] || exit 0
            from="${SNAPSHOTS}/${snapshot}/${home#/var/home/}"
            open_files="$(gettext "Open it in Files")"
            put_folder="$(gettext "Put a folder back")"
            how="$(gum choose --header "$(eval_gettext "Your home folder as it was at \${snapshot}")" \
                "${open_files}" "${put_folder}" "${cancel}")" || exit 0
            case "${how}" in
                "${open_files}")
                    echo "$(gettext "Read-only: copy what you want back into your home folder.")"
                    xdg-open "${from}"
                    ;;
                "${put_folder}")
                    folder="$(gum input --header "$(gettext "Which folder, inside your home? (empty for all of it)")" \
                        --placeholder "Documents")" || exit 0
                    folder="${folder#/}"
                    folder="${folder%/}"
                    if [[ ! -d "${from}/${folder}" ]]; then
                        echo "$(eval_gettext "There is no \${folder} in that snapshot.")"
                        exit 1
                    fi
                    echo "$(eval_gettext "Files in \${home}/\${folder} with the same name as one in the snapshot are replaced by the snapshot's. Files the snapshot does not have, such as what ransomware renamed, stay.")"
                    if [[ -n "${folder}" ]]; then
                        question="$(eval_gettext "Put \${folder} back as it was at \${snapshot}?")"
                    else
                        question="$(eval_gettext "Put your home folder back as it was at \${snapshot}?")"
                    fi
                    gum confirm "${question}" || exit 0
                    mkdir -p "${home}/${folder}"
                    # A reflink shares the snapshot's blocks, so this takes no time and no space. A link ransomware
                    # left at a name is replaced, never written through
                    cp -a --reflink=auto --remove-destination "${from}/${folder}/." "${home}/${folder}/"
                    echo "$(gettext "Put back. Delete what ransomware left next to your files, then check they open.")"
                    ;;
                *) exit 0 ;;
            esac
            ;;
        "${turn_off}" | off)
            sudo /usr/libexec/amethystora-security-config note ransomware-protection off
            sudo systemctl disable --now amethystora-home-snapshot.timer
            echo "$(gettext "Off. No more snapshots are taken.")"
            if [[ ${#TAKEN[@]} -gt 0 ]] && gum confirm "$(eval_ngettext "Delete the \${count} snapshot already taken, and free its space?" "Delete the \${count} snapshots already taken, and free their space?" "${count}")"; then
                for snapshot in "${TAKEN[@]}"; do
                    sudo btrfs subvolume delete "${SNAPSHOTS}/${snapshot}"
                done
            fi
            ;;
        *) exit 0 ;;
    esac

# Show what the virus scans found, run one now, or put back or delete a file in quarantine
[positional-arguments]
scan ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    FINDINGS=/var/log/amethystora-clamav-findings.log
    QUARANTINE=/usr/libexec/amethystora-quarantine
    action="${1:-}"
    scan_now="$(gettext "Scan now")"
    show_quarantine="$(gettext "Show the quarantine")"
    put_back="$(gettext "Put a file back")"
    delete_file="$(gettext "Delete a file for good")"
    show_log="$(gettext "Show the full log")"
    cancel="$(gettext "Cancel")"
    if sudo test -s "${FINDINGS}"; then
        echo "$(gettext "The last scan found something:")"
        # File names are whatever somebody called a file: shown so that none can act on the terminal
        sudo cat "${FINDINGS}" | /usr/libexec/amethystora-security-text --lines | sed 's/^/  /'
    else
        echo "$(gettext "The last scan found nothing.")"
    fi
    mode="$(/usr/libexec/amethystora-security-config ON_DETECTION || true)"
    echo "$(eval_gettext "On detection: \${mode} (ame security settings on-detection)")"
    # As the report says it: the unit knows only a run of this start, its timer the last one it started
    last_run="$({ systemctl show --timestamp=unix -p InactiveEnterTimestamp --value amethystora-clamav-scan.service
        systemctl show --timestamp=unix -p LastTriggerUSec --value amethystora-clamav-scan.timer; } 2>/dev/null |
        sed -n 's/^@\([0-9][0-9]*\)$/\1/p' | sort -n | tail -n1 || true)"
    if [[ -n "${last_run}" ]]; then
        last_run="$(date -d "@${last_run}" '+%a %-d %b, %H:%M')"
    else
        last_run="$(gettext "not yet")"
    fi
    echo "$(eval_gettext "Last run:    \${last_run}")"
    AGE="$(/usr/libexec/amethystora-clamav-signature-age)" && SIG=0 || SIG=$?
    case "${SIG}" in
        0) echo "$(eval_ngettext "Signatures:  updated \${AGE} day ago" "Signatures:  updated \${AGE} days ago" "${AGE}")" ;;
        2) echo "$(eval_ngettext "Signatures:  \${AGE} day old. Too old for a clean scan to mean much; check the network." "Signatures:  \${AGE} days old. Too old for a clean scan to mean much; check the network." "${AGE}")" ;;
        *) echo "$(gettext "Signatures:  none yet. clamav-freshclam.service has not finished its first download.")" ;;
    esac
    if [[ -z "${action}" ]]; then
        action="$(gum choose --header "$(gettext "Virus scan (ClamAV)")" "${scan_now}" "${show_quarantine}" \
            "${put_back}" "${delete_file}" "${show_log}" "${cancel}")" || exit 0
    fi
    case "${action}" in
        "${scan_now}" | now)
            if /usr/libexec/amethystora-security-config --is VIRUS_SCAN off; then
                echo "$(gettext "The virus scanner is turned off. Turn it on first with: ame security settings virus-scan")"
                exit 1
            fi
            echo "$(gettext "Scanning the home and temporary directories. This takes a while.")"
            sudo systemctl start --wait amethystora-clamav-scan.service || true
            sudo journalctl -u amethystora-clamav-scan.service -n 40 --no-pager
            ;;
        "${show_quarantine}" | quarantine)
            list="$(sudo "${QUARANTINE}" list)"
            if [[ -z "${list}" ]]; then
                echo "$(gettext "The quarantine is empty.")"
            else
                echo "$(gettext "In quarantine (id, when, what it matched, where it was):")"
                sed 's/^/  /' <<<"${list}"
                echo "$(gettext "Put one back with: ame security scan restore, or delete it for good with: ame security scan delete")"
            fi
            ;;
        "${put_back}" | restore)
            list="$(sudo "${QUARANTINE}" list)"
            if [[ -z "${list}" ]]; then
                echo "$(gettext "The quarantine is empty.")"
                exit 0
            fi
            choice="$(gum choose --header "$(gettext "Put which file back?")" <<<"${list}")" || exit 0
            id="${choice%%$'\t'*}"
            path="${choice##*$'\t'}"
            echo "$(gettext "A file is only in quarantine because it matched known malware. Put it back only if you are sure that match is wrong.")"
            gum confirm "$(eval_gettext "Put \${path} back?")" || exit 0
            if gum confirm "$(gettext "Tell the scanner to leave this exact file alone from now on?")"; then
                sudo "${QUARANTINE}" restore "${id}" trust
            else
                sudo "${QUARANTINE}" restore "${id}"
                echo "$(gettext "The next scan will find it again unless you let the scanner leave it alone.")"
            fi
            ;;
        "${delete_file}" | delete)
            list="$(sudo "${QUARANTINE}" list)"
            if [[ -z "${list}" ]]; then
                echo "$(gettext "The quarantine is empty.")"
                exit 0
            fi
            choice="$(gum choose --header "$(gettext "Delete which file for good?")" <<<"${list}")" || exit 0
            id="${choice%%$'\t'*}"
            path="${choice##*$'\t'}"
            gum confirm "$(eval_gettext "Delete \${path} for good? Nothing can bring it back.")" || exit 0
            sudo "${QUARANTINE}" delete "${id}"
            ;;
        "${show_log}" | log)
            sudo journalctl -u amethystora-clamav-scan.service -u amethystora-clamav-onaccess.service --no-pager
            ;;
        *) exit 0 ;;
    esac
