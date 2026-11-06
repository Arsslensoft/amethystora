# shellcheck shell=bash
# For amethystora-backup, amethystora-restore and the recovery key's helper when the backup is to another computer (AMETHYSTORA_BACKUP_SECRETS=vault in
# its settings): the client key the Vault holds, as a file in a folder of $XDG_RUNTIME_DIR for as long as the program runs, named for restic. The repository's
# password comes through RESTIC_PASSWORD_COMMAND (amethystora-backup-secret password). Neither is ever in a file of this account's own: the caller
# calls backup_secrets_close when it ends, however it ends.

backup_secrets_open() {
    local code=0
    [[ -n "${XDG_RUNTIME_DIR:-}" && -d "${XDG_RUNTIME_DIR}" ]] || return 1
    BACKUP_SECRETS_DIR="$(umask 077 && mktemp -d "${XDG_RUNTIME_DIR}/amethystora-backup.XXXXXX")" || return 1
    /usr/libexec/amethystora-backup-secret client "${BACKUP_SECRETS_DIR}" || code=$?
    if [[ "${code}" -ne 0 ]]; then
        backup_secrets_close
        return "${code}"
    fi
    export RESTIC_TLS_CLIENT_CERT="${BACKUP_SECRETS_DIR}/client.pem"
}

backup_secrets_close() {
    if [[ -n "${BACKUP_SECRETS_DIR:-}" ]]; then
        rm -rf -- "${BACKUP_SECRETS_DIR}"
    fi
    BACKUP_SECRETS_DIR=""
    unset RESTIC_TLS_CLIENT_CERT
    return 0
}
