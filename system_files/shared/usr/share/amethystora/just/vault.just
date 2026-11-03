# vim: set ft=make :

set default-list

# What every recipe here says when the service says no, in one place: `run` is the command line with its refusal turned into a
# sentence, and the command line's own words come from here through its --say options
words := '''
    vault_says() {
        case "$1" in
            0) return 0 ;;
            1) gettext "No: that was refused, or it did not match."; echo ;;
            2) gettext "The vault is closed. Open it with: ame vault open"; echo ;;
            3) gettext "There is no vault on this account yet. Set one up with: ame vault setup"; echo ;;
            4) gettext "That is too large for the vault."; echo ;;
            5) gettext "The vault did not understand that."; echo ;;
            64) echo "usage: ame vault" ;;
            7) gettext "That was not typed the same both times, so nothing was kept."; echo ;;
            10) gettext "That is outside your home, through a link, or not a plain file or folder."; echo ;;
            11) gettext "Nothing is there."; echo ;;
            12) gettext "That holds no secret, and stays where it is."; echo ;;
            13) gettext "Something else is already there, and it was left as it is."; echo ;;
            14) gettext "Another command has that file open."; echo ;;
            15) gettext "It changed meanwhile, so nothing was removed."; echo ;;
            16) gettext "This system gives a command no namespace of its own, so the file stays sealed."; echo ;;
            17) gettext "That tool is not installed."; echo ;;
            18) gettext "That is protected already. Take it out of the vault first with: ame vault unprotect"; echo ;;
            19) gettext "That file has another name, which would keep its content in the open."; echo ;;
            20) gettext "Something above it cannot be changed, so nothing was moved."; echo ;;
            21) gettext "Something took the file's place away while the command ran, and nothing was sealed."; echo ;;
            22) gettext "That variable changes how programs start, so a secret is never put in it. Name another: VARIABLE=NAME"; echo ;;
            *) gettext "The vault's service could not do that. Try again in a moment."; echo ;;
        esac
        exit "$1"
    }
    run() {
        local code=0
        /usr/libexec/amethystora-vault-cli "$@" \
            --say-password="$(gettext 'Vault password or recovery key: ')" \
            --say-current="$(gettext 'Current vault password: ')" \
            --say-new="$(gettext 'New vault password: ')" \
            --say-again="$(gettext 'Once more: ')" \
            --say-name="$(gettext 'Name for the vault: ')" \
            --say-hidden="$(gettext 'To make it hidden, type: hidden ')" \
            --say-recovery="$(gettext 'Your recovery key. Write it down: it is the only way back if every other way in is lost.')" \
            --say-type-back="$(gettext 'Type the recovery key back: ')" \
            --say-this-password="$(gettext 'Password: ')" \
            --say-file-password="$(gettext 'Password of the file: ')" \
            --say-key-passphrase="$(gettext 'Passphrase of the key: ')" \
            --say-link="$(gettext 'The otpauth:// link: ')" \
            --say-codes-found="$(gettext 'Codes found: {count}. Keep them? y/n ')" \
            --say-restore-key="$(gettext 'Recovery key of the vault to bring back: ')" \
            --say-restored="$(gettext 'The vault is back. Choose how it opens now.')" \
            --say-codes-left-out="$(gettext 'Not kept: {dropped} too long or past what one text may hold, {repeats} already kept, {over_limit} past what one item holds.')" \
            --say-other-helper="$(gettext 'git already asks another credential helper, {helper}, and would hand it what the vault answers. Remove that one from git'\''s settings first, or keep the vault'\''s logins out of git.')" \
            || code=$?
        vault_says "$code"
    }
'''

# What polkit shows when the service asks you, each sentence as the service passes it, in the words it fills in itself
# N_ 'Set up Amethystora Vault. A program calling itself $(program) asks.'
# N_ 'Open Amethystora Vault. A program calling itself $(program) asks.'
# N_ 'Make a new vault. A program calling itself $(program) asks.'
# N_ 'Change how Amethystora Vault opens. A program calling itself $(program) asks.'
# N_ 'A program calling itself $(program) asks to use “$(item)” from Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to change “$(item)” in Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to remove “$(item)” from Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to change how often “$(item)” asks you in Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to remove the vault “$(item)” and everything in it.'
# N_ 'A program calling itself $(program) asks to remove every vault of this account and everything in them.'
# N_ 'A program calling itself $(program) asks to keep Amethystora Vault open for longer before it closes by itself.'
# N_ 'A program calling itself $(program) asks to take “$(item)” out of Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to show the key of “$(item)”. Anyone who sees it can make your codes.'
# N_ 'A program calling itself $(program) asks to show every backup code of “$(item)”.'
# N_ 'A program calling itself $(program) asks to take every authenticator account and backup code out of Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to replace your authenticator accounts in Amethystora Vault with an import.'
# N_ 'A program calling itself $(program) asks Amethystora Vault to issue the certificate “$(item)”.'
# N_ 'A program calling itself $(program) asks Amethystora Vault to issue the identity certificate of this machine, “$(item)”.'
# N_ 'A program calling itself $(program) asks to replace the root certificate of this machine. Everyone who trusted the old one has to stop by hand.'
# N_ 'A program calling itself $(program) asks to sign with “$(item)” from Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to sign a message for “$(detail)”, not a login, with “$(item)” from Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to sign something that is not a login with “$(item)” from Amethystora Vault.'
# N_ 'A program calling itself $(program), on a connection forwarded from another computer (host key $(detail)), asks to sign with “$(item)” from Amethystora Vault.'
# N_ 'A program calling itself $(program) asks to connect Amethystora Vault to the server and account “$(item)”. Whatever in the vault syncs is sent to that account, and whoever opens the vault can reach all of it.'
# N_ 'A program calling itself $(program) asks to connect Amethystora Vault to “$(item)”, trusting the certificate with that fingerprint for this connection. Whatever in the vault syncs is sent to that account, and whoever opens the vault can reach all of it.'
# N_ 'A program calling itself $(program) asks to stop syncing Amethystora Vault with its server.'
# N_ 'A program calling itself $(program) asks to send “$(item)” to the server Amethystora Vault syncs with.'
# N_ 'A program calling itself $(program) asks to stop syncing “$(item)”, which moves its copy on the server to the trash of the server.'
# N_ 'A program calling itself $(program) asks to let Amethystora Vault take, on its next sync, the items that went from the server or came from it.'

# Set up a vault on this account: password, approval (fingerprint or account password) or no-prompt. It ends with a recovery key that you type back
[positional-arguments]
setup METHOD="password":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    if [[ "$1" == no-prompt ]]; then
        gettext "No prompt stops a program that reads your files, a copied backup and a Flatpak with access to your home. It does not stop a program of this account that asks."; echo
        printf '%s ' "$(gettext 'To choose it, type: no prompt')"
        read -r answer
        [[ "$answer" == "no prompt" ]] || exit 1
    fi
    run setup "$1"
    gettext "The vault is set up and open."; echo

# Which vaults this account has, whether each is open, and how it opens. A closed vault's name is sealed with it
status:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    output=$(/usr/libexec/amethystora-vault-cli status) || vault_says $?
    while IFS=$'\t' read -r what id state first methods name; do
        case "$what" in
            set-up)
                if [[ "$id" == no ]]; then
                    gettext "There is no vault on this account yet. Set one up with: ame vault setup"; echo
                fi
                ;;
            vault)
                label=${name:-$(gettext 'A closed vault')}
                if [[ "$state" == open ]]; then
                    eval_gettext '${label}: open'; echo
                else
                    eval_gettext '${label}: closed'; echo
                fi
                ;;
        esac
    done <<<"$output"

# The same as status: every vault you can see from here
vaults:
    @ame vault status

# Open the vault: approval, no-prompt, security-key or password. Typing a password or a recovery key is also how a hidden vault opens
[positional-arguments]
open *METHOD:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run open "$@" >/dev/null
    gettext "Open."; echo

# Close every open vault of this account now
lock:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run lock

# How each vault opens, as JSON. add approval, no-prompt, security-key or password; remove SLOT; background on or off (a key for unattended work, on the TPM); idle MINUTES
[positional-arguments]
methods *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    if [[ "${1:-} ${2:-}" == "add no-prompt" ]] || [[ "${1:-} ${2:-}" == "background on" ]]; then
        gettext "This stops a program that reads your files, a copied backup and a Flatpak with access to your home. It does not stop a program of this account that asks."; echo
        printf '%s ' "$(gettext 'To choose it, type: no prompt')"
        read -r answer
        [[ "$answer" == "no prompt" ]] || exit 1
    fi
    run methods "$@"

# Change the password of the vault that is open
change-password:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run change-password
    gettext "The password is changed."; echo

# A new vault with its own password and recovery key, opened by its own password. It asks its name and whether it is hidden (in no list, opening only when its password is typed): typed, never part of the command
new:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run new
    gettext "The vault is made and open."; echo

# Call the vault that is open something else; the name is typed, never part of the command
rename:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run rename

# Remove the vault that is open, and everything in it. Not the first: forget removes that
remove-this:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run remove-this

# Remove every vault here, including any you cannot see, and everything in them, for good
forget:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    gettext "This removes every vault here, including any you cannot see, and everything in them, for good. Nothing can bring them back but a backup and its recovery key."; echo
    printf '%s ' "$(gettext 'To go on, type: FORGET')"
    read -r answer
    [[ "$answer" == "FORGET" ]] || exit 1
    run forget
    gettext "Every vault here is gone."; echo

# Items by kind and id: put KIND NAME [ID] (the value as JSON on standard input, or plain text with --text), get KIND ID, list [KIND], remove KIND ID, approval KIND ID ask|while-open|never
[positional-arguments]
item *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run item "$@"

# Passwords: generate [LENGTH|words], add NAME [USER] [ADDRESS] (asks for it), new NAME [USER] [ADDRESS] (makes one), list, get NAME (to standard output, after you approve), remove NAME
[positional-arguments]
password *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run password "$@"

# Backup codes: add SERVICE [FILE] (a text file or a PDF, or standard input), list, next SERVICE, used SERVICE CODE-NUMBER (with --undo to take it back), remove SERVICE
[no-cd]
[positional-arguments]
codes *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run codes "$@"

# Authenticator codes: add [FILE] (a file from another app, or asks for a link), list, code NAME (with --step for the next code of a counter-based account), qr NAME, remove NAME
[no-cd]
[positional-arguments]
auth *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    if [[ "${1:-}" == qr ]]; then
        gettext "Anyone who sees this code or the key under it can make your codes."; echo
    fi
    run auth "$@"

# Keys: new NAME [TYPE] (--kind=ssh, signing, exchange or symmetric, and --not-exportable, for good), import NAME FILE, list, public NAME, export NAME (--plain for no passphrase, --out=FILE), remove NAME
[no-cd]
[positional-arguments]
key *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    if [[ "${1:-}" == export ]]; then
        gettext "A key taken out of the vault lives outside it from now on."; echo
    fi
    run key "$@"
    [[ "${1:-}" == import ]] || exit 0
    file="${!#}"
    printf '%s ' "$(gettext 'The key is in the vault. Take the file out of your home as well? To do so, type: yes')"
    read -r answer
    [[ "$answer" == yes ]] || exit 0
    ame vault protect "$file"

# Certificates: new NAME pdf|code|mail|tls|device|login [HOST...] (--name=NAME, --email=ADDRESS, --web=ADDRESS, --org=ORGANIZATION, --country=CC, --key=TYPE, --digest=DIGEST), list, show NAME, chain NAME, export NAME [pem] (--out=FILE), renew NAME, ca [account|device|local|root], remove NAME
[no-cd]
[positional-arguments]
cert *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    if [[ "${1:-}" == new ]]; then
        gettext "Only computers that trust this machine's root, or your CA, accept this. Other computers call the signer unknown until then."; echo
    fi
    run cert "$@"

# Sign a PDF with a certificate of yours: pdf sign FILE [CERT]. The signed copy goes beside it, and the key never leaves the vault
[no-cd]
[positional-arguments]
pdf *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    gettext "Only computers that trust this machine's root, or your CA, accept this. Other computers call the signer unknown until then."; echo
    run pdf "$@"

# This machine's root certificate, to give to whoever should trust everything it issues. root rotate makes a new one, for a stolen key: everyone who trusted the old one has to stop by hand
[positional-arguments]
root *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run root "$@"

# The sealed backup: every vault of this account, hidden ones too, as they lie sealed, in ~/.local/share/amethystora/vault-backup (seven kept). It needs no open vault, and only the recovery keys open what is in it. The daily backup runs it first
backup:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run backup

# Bring a vault back from a sealed backup: it asks the vault's recovery key, then a new way to open it
[no-cd]
[positional-arguments]
restore FILE:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run restore "$1"
    gettext "The vault is open."; echo

# Whether the Vault looks up a logo for each authenticator account by asking its own website. logos on says what that tells the site, and asks first
[positional-arguments]
logos STATE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    marker="${XDG_CONFIG_HOME:-$HOME/.config}/amethystora/vault-logos"
    case "${1:-}" in
        on)
            gettext "A logo is fetched from the account's own website, which learns that someone at your address has an account there. Nothing else is sent, and no vault you cannot see is ever looked up."; echo
            printf '%s ' "$(gettext 'To turn it on, type: logos')"
            read -r answer
            [[ "$answer" == "logos" ]] || exit 1
            mkdir -p "$(dirname "$marker")"
            : >"$marker"
            gettext "Logos are on."; echo ;;
        off)
            rm -f "$marker"
            gettext "Logos are off."; echo ;;
        "")
            if [[ -e "$marker" ]]; then gettext "Logos are on."; else gettext "Logos are off."; fi; echo ;;
        *) echo "usage: ame vault logos on|off"; exit 64 ;;
    esac

# Let your browsers trust the certificates this machine makes for your local services (a printer, a NAS): tls trust browsers on adds the Local Services CA for websites, off takes it away. Firefox keeps its own store: import the CA there by hand
[positional-arguments]
tls *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    if [[ "${1:-}" == trust && "${2:-}" == browsers && "${3:-}" == on ]]; then
        gettext "Browsers that read your own certificate database will accept a certificate this machine made for a local name, such as printer.local, and nothing for a public website."; echo
    fi
    run tls "$@"

# The SSH agent: on points this account's programs at the keys in the vault from your next login, off takes that back, status says how it is
[positional-arguments]
ssh ARG="status":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    case "$1" in
        on)
            run ssh on
            gettext "Programs you start after your next login use the keys in the vault. The vault has to be open for them to."; echo
            ;;
        off)
            run ssh off
            gettext "From your next login, programs no longer use the keys in the vault."; echo
            ;;
        status)
            output=$(/usr/libexec/amethystora-vault-cli ssh status) || vault_says $?
            while IFS=$'\t' read -r what value; do
                case "$what:$value" in
                    ssh:on) gettext "The SSH agent is on for this account."; echo ;;
                    ssh:off) gettext "The SSH agent is off for this account."; echo ;;
                    keys:-) gettext "It does not answer now."; echo ;;
                    keys:*) count="$value"; eval_ngettext 'It lists $count key.' 'It lists $count keys.' "$count"; echo ;;
                    this-session:yes) gettext "This session uses it."; echo ;;
                    this-session:no) gettext "This session does not use it. A new login would."; echo ;;
                esac
            done <<<"$output"
            ;;
        *) echo "usage: ame vault ssh on|off|status"; exit 64 ;;
    esac

# A password on standard output, after approval, for a script or a tool's own settings: NAME, or NAME aws for AWS's credential_process, or NAME kube for kubectl's exec
[no-cd]
[positional-arguments]
get *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    /usr/libexec/amethystora-vault-cli get "$@" || vault_says $? >&2

# One command with a secret: NAME -- COMMAND. A protected file appears at its path for that command alone, and is sealed again when it ends. Anything else is a variable, NAME's in capitals or the one you give as VARIABLE=NAME
[no-cd]
[positional-arguments]
run *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    # The command's own exit status is this one's, so nothing comes after it. Words go first, since all that follows -- is the command's.
    exec /usr/libexec/amethystora-vault-cli \
        --say-inherit="$(gettext 'The secret is in {variable} for this command. Every program it starts inherits it, and other programs of this account can read it while it runs.')" \
        --say-exposed="$(gettext 'While this command runs, other programs of this account can read this file and the command'\''s memory.')" \
        --say-unsealable="$(gettext 'What the command left cannot be sealed again, so the file stays as it was before it ran.')" \
        --say-killed="$(gettext 'The command did not end well after changing the file. What it left is sealed beside the old copy, under its own name. See: ame vault protected')" \
        --say-emptied="$(gettext 'The command left the file empty, so the sealed copy was kept as it was.')" \
        --say-detached="$(gettext 'Something took the file'\''s place away while the command ran, so what it wrote went to your disk, not into the vault, and nothing was sealed. The old copy is still sealed.')" \
        --say-copy-left="$(gettext 'Not kept, since it may be a copy of the protected file, or half of one: its name has the file'\''s in it, it holds its content, or the command did not end well ({count} in all): {names}')" \
        --say-not-kept="$(gettext 'These could not be kept, and are gone with the command'\''s private space ({count} in all, and any with names that cannot be shown): {names}')" \
        --say-variable="$(gettext 'That variable changes how programs start, so a secret is never put in it. Name another: VARIABLE=NAME')" \
        --say-reopen="$(gettext 'The vault closed while the command ran, and what it left is only here. Open the vault, then press Enter to seal it, or type no to lose it: ')" \
        --say-again="$(gettext 'That was refused, and what the command left is only here. Press Enter to be asked again, or type no to lose it: ')" \
        --say-lost="$(gettext 'What the command left is gone. The file is as it was before it ran.')" \
        --say-refused="$(gettext 'No: that was refused, or it did not match.')" \
        --say-closed="$(gettext 'The vault is closed. Open it with: ame vault open')" \
        --say-not-set-up="$(gettext 'There is no vault on this account yet. Set one up with: ame vault setup')" \
        --say-file-there="$(gettext 'Something else is already there, and it was left as it is.')" \
        --say-in-use="$(gettext 'Another command has that file open.')" \
        --say-no-namespace="$(gettext 'This system gives a command no namespace of its own, so the file stays sealed.')" \
        run "$@"

# Take a file or folder out of your home and into the vault, sealed. It says first what that cannot do
[no-cd]
[positional-arguments]
protect PATH:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    gettext "Old copies survive this. The hourly snapshots keep the plain file for up to 14 days, backups for as long as they last, and nothing here deletes from them."; echo
    gettext "So replace this key or login once it is protected: whatever could read the old file may have kept it."; echo
    gettext "The file is unlinked, not wiped, since the disk copies on write. The disk's encryption covers what is left of it."; echo
    gettext "It stays sealed until a command is given it, and while that command runs, other programs of this account can read it."; echo
    printf '%s ' "$(gettext 'To go on, type: yes')"
    read -r answer
    [[ "$answer" == yes ]] || exit 1
    run protect "$1" >/dev/null
    gettext "It is out of your home and sealed. A command reaches it through: ame vault run"; echo

# What is protected: name, path, file or folder, and size. With usual, what is at the usual places for credentials and is not protected
[positional-arguments]
protected *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run protected "$@"

# Put a protected file back in your home as an ordinary file, and take it out of the vault
[no-cd]
[positional-arguments]
unprotect NAME:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    run unprotect "$1"
    gettext "It is back in your home, as an ordinary file."; echo

# Which tools ask the vault for their logins. git, docker, aws, kube or claude with on or off writes that tool's settings: aws on ITEM [PROFILE], kube on ITEM [USER], kube off USER, claude on ITEM
[positional-arguments]
helpers *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    if (( $# > 0 )); then
        run helpers "$@"
        exit 0
    fi
    output=$(/usr/libexec/amethystora-vault-cli helpers) || vault_says $?
    while IFS=$'\t' read -r tool installed state; do
        if [[ "$state" == on ]]; then
            eval_gettext '${tool}: asks the vault'; echo
        elif [[ "$installed" == installed ]]; then
            eval_gettext '${tool}: does not ask the vault'; echo
        else
            eval_gettext '${tool}: not installed'; echo
        fi
    done <<<"$output"

# Sync with a Vaultwarden or Bitwarden server. connect ADDRESS, now [accept], status, off, choose KIND ID on|off|sealed (a key or certificate goes only sealed, and only when chosen)
[positional-arguments]
sync *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    {{words}}
    options=(
        --say-sync-intro="$(gettext "What this vault holds that syncs (passwords, authenticator codes and backup codes) becomes items in that account, encrypted with the account's own key, and the account's changes come back. The server learns how many items there are, of which kind, how long each is and when it changed. The account's key is kept in this vault, so whoever opens this vault can reach that whole account. A dedicated account for the vault is the better choice. It works with Bitwarden and Vaultwarden accounts, and is not Bitwarden.")"
        --say-sync-email="$(gettext 'Email address of the account: ')"
        --say-sync-method="$(gettext 'Log in with the personal API key (it asks for no code) or the master password? Type key or password: ')"
        --say-sync-api-id="$(gettext 'API key client_id: ')"
        --say-sync-api-secret="$(gettext 'API key client_secret: ')"
        --say-sync-password="$(gettext "Master password (it opens the account's key here and is not kept): ")"
        --say-sync-trust="$(gettext "The server's certificate is not one this machine knows. To trust it for this connection only, give the fingerprint of it that the server's owner gave you:")"
        --say-sync-subject="$(gettext 'Subject:')"
        --say-sync-issuer="$(gettext 'Issuer:')"
        --say-sync-fingerprint="$(gettext 'SHA-256:')"
        --say-sync-trust-type="$(gettext "SHA-256 fingerprint from the server's owner: ")"
        --say-sync-trust-mismatch="$(gettext "That is not this certificate's fingerprint. Nothing was trusted.")"
        --say-sync-kdf-memory="$(n='{n}' eval_gettext "This account's key takes \${n} MiB of memory to make, which is a lot, and it is made in a program of its own that is gone afterwards. Type yes to go on: ")"
        --say-sync-kdf="$(n='{n}' eval_gettext "This account's key is made with only \${n} rounds, which is far fewer than Bitwarden's default, and makes the master password easier to guess. Type yes to go on: ")"
        --say-sync-provider="$(gettext 'Which second step? Type authenticator, email or yubikey: ')"
        --say-sync-code-app="$(gettext 'Code from your authenticator app: ')"
        --say-sync-code-email="$(gettext 'Code sent to your email: ')"
        --say-sync-code-yubikey="$(gettext 'Touch your YubiKey: ')"
        --say-sync-device="$(gettext "This server checks new devices, and sent a code to the account's email. Code, or type resend: ")"
        --say-sync-done="$(gettext 'Connected. While the vault is open, it and the account keep each other up to date.')"
        --say-problem-not-an-address="$(gettext 'That is not an address of a server.')"
        --say-problem-not-https="$(gettext 'Only a server on this machine may be reached without https.')"
        --say-problem-unsupported-server="$(gettext 'This kind of server is not supported.')"
        --say-problem-already-connected="$(gettext 'A server is connected already. Reconnect it, or turn sync off first.')"
        --say-problem-other-account="$(gettext 'That is another account than the one connected. Turn sync off first.')"
        --say-problem-not-connected="$(gettext 'No server is connected. Connect one with: ame vault sync connect ADDRESS')"
        --say-problem-not-syncable="$(gettext 'That kind of item does not sync.')"
        --say-problem-sealed-only="$(gettext 'A key or a certificate goes to the server only sealed.')"
        --say-problem-not-exportable="$(gettext 'That key was made so that it cannot leave the vault.')"
        --say-problem-bad-request="$(gettext 'That was not understood.')"
        --say-problem-unreachable="$(gettext 'The server could not be reached.')"
        --say-problem-untrusted="$(gettext "The server's certificate is not trusted.")"
        --say-problem-timeout="$(gettext 'The server took too long to answer.')"
        --say-problem-too-large="$(gettext 'The server sent more than the vault will take.')"
        --say-problem-not-allowed="$(gettext 'The vault would not go where the server pointed it.')"
        --say-problem-server-down="$(gettext 'The server is not answering right now. The vault goes on, and tries again later.')"
        --say-problem-rate-limited="$(gettext 'The server asks for fewer tries. Wait a while.')"
        --say-problem-server-refused="$(gettext 'The server refused the connection itself. The personal API key may get through where the password does not.')"
        --say-problem-server-said="$(gettext 'The server said:')"
        --say-problem-two-factor-unoffered="$(gettext 'This account asks for a second step the vault cannot do. The personal API key does not ask for one.')"
        --say-problem-kdf-unknown="$(gettext 'The server asked for a kind of key stretching the vault does not know.')"
        --say-problem-kdf-too-weak="$(gettext 'The server asks for far too little work to make the key. The vault will not log in.')"
        --say-problem-kdf-too-strong="$(gettext 'The server asks for more work than the vault will do.')"
        --say-problem-kdf-lowered="$(gettext 'The server now asks for less work to make the key than when this was connected. That is what a server would do to make a master password easier to guess, so nothing was sent. To accept it, turn sync off and connect again.')"
        --say-problem-salt-changed="$(gettext "The server gave another salt than the account's email. Something is wrong.")"
        --say-problem-unsupported="$(gettext 'This account logs in in a way the vault does not support (single sign-on, a key connector, or no master password).')"
        --say-problem-password-reset="$(gettext 'The account must change its master password first. Do that in its web vault.')"
        --say-problem-newer-encryption="$(gettext "This account uses Bitwarden's newer encryption, which the vault cannot read yet. Nothing was changed on either side.")"
        --say-problem-cannot-open="$(gettext "The account's key did not open. The master password may be wrong.")"
        --say-problem-no-key="$(gettext 'The server sent no usable key for this account. Nothing was changed.')"
        --say-problem-keys-changed="$(gettext "The account's key changed on the server, as it does when the master password is changed elsewhere. Connect again to prove the new one.")"
        --say-problem-identity-changed="$(gettext 'Another account than the connected one answered.')"
        --say-problem-login-expired="$(gettext 'The login ran out. Connect again.')"
        --say-problem-client-version="$(gettext 'The server will not talk to this version of the vault.')"
        --say-problem-bad-response="$(gettext 'The server answered in a way the protocol does not allow.')"
        --say-problem-many-gone="$(n='{n}' eval_gettext 'Items have gone from the server, or been put in its trash, at once or over the last days: ${n}. If you removed them there, accept it with: ame vault sync now accept')"
        --say-problem-many-new="$(n='{n}' eval_gettext 'Items have come from the server at once or over the last days: ${n}. If you added them there, accept it with: ame vault sync now accept')"
        --say-problem-local-unreadable="$(n='{n}' eval_gettext 'Items of the vault did not read: ${n}. Nothing is sent to the server while that is so, since an item that is not there looks removed. Restore the vault from a backup, or remove the damaged items.')"
        --say-problem-links-unreadable="$(gettext 'What the vault keeps of its sync did not read. Turn sync off and connect again.')"
    )
    case "${1:-}" in
        status)
            output=$(/usr/libexec/amethystora-vault-cli sync "$@" "${options[@]}") || vault_says $?
            while IFS=$'\t' read -r what a; do
                case "$what" in
                    connected) [[ "$a" == yes ]] || { gettext "No server is connected."; echo; } ;;
                    address) eval_gettext 'Server: ${a}'; echo ;;
                    email) eval_gettext 'Account: ${a}'; echo ;;
                    linked) eval_gettext 'Items linked with the server: ${a}'; echo ;;
                    running) [[ "$a" != yes ]] || { gettext "A sync is running now."; echo; } ;;
                    last) when=$(date -d "@${a}" '+%Y-%m-%d %H:%M'); eval_gettext 'Last synced: ${when}'; echo ;;
                    problem) eval_gettext 'Stopped: ${a}'; echo ;;
                esac
            done <<<"$output"
            ;;
        now)
            code=0
            output=$(/usr/libexec/amethystora-vault-cli sync "$@" "${options[@]}") || code=$?
            while IFS=$'\t' read -r what a; do
                case "$what" in
                    sent) eval_gettext 'Sent to the server: ${a}'; echo ;;
                    received) eval_gettext 'Taken from the server: ${a}'; echo ;;
                    conflicts) [[ "$a" == 0 ]] || { eval_gettext 'Changed on both sides, both kept: ${a}'; echo; } ;;
                    organization) [[ "$a" == 0 ]] || { eval_gettext 'Items of an organization, left alone: ${a}'; echo; } ;;
                    refused)
                        case "$a" in
                            too-large-to-seal) gettext "Not synced: an item is too large for the account to keep. A secure note holds a little over 5 KB, and the vault will not split an item across several."; echo ;;
                            too-long) gettext "Not synced: an item has a field longer than Bitwarden accepts."; echo ;;
                            no-link) gettext "Not synced: an authenticator account of a kind Bitwarden's apps cannot read."; echo ;;
                            fields-swapped) gettext "Not taken from the server: an item had its fields moved around, which a server can do to an item. The vault kept its own."; echo ;;
                            copy) gettext "Not taken from the server: an item had the same ciphertext as another, which only a server copying one item does."; echo ;;
                            too-big) gettext "Not synced: an item is larger or nested deeper than any of a person's, and was left alone."; echo ;;
                            future-date) gettext "Not taken from the server: an item is dated far ahead of this machine's clock, and was left alone."; echo ;;
                            unreadable) gettext "Not taken from the server: an item did not open, so it was left as it is."; echo ;;
                            rolled-back) gettext "Not taken from the server: an item came back older than the vault knows. It was left as it is."; echo ;;
                            key-removed | type-changed | moved-to-organization) gettext "Not taken from the server: an item changed in a way only a tampering server does. It was left as it is."; echo ;;
                            *) eval_gettext 'Not synced: ${a}'; echo ;;
                        esac
                        ;;
                esac
            done <<<"$output"
            [[ $code -eq 1 ]] && exit 1
            vault_says "$code"
            ;;
        connect | off | choose)
            code=0
            /usr/libexec/amethystora-vault-cli sync "$@" "${options[@]}" || code=$?
            [[ $code -eq 1 ]] && exit 1
            vault_says "$code"
            if [[ "$1" == off ]]; then gettext "Sync is off. The vault and the account keep what they have."; echo; fi
            ;;
        *)
            echo "usage: ame vault sync connect ADDRESS | now [accept] | status | off | choose KIND ID on|off|sealed"
            exit 64
            ;;
    esac
