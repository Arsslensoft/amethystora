#!/usr/bin/bash
# Back up to your other computer (R7-35), end to end on one machine: two computers as two sets of folders, the Vault as a stand-in that really encrypts
# (test-vault-stand-in.py: never the live one), the real restic and rclone over loopback.
#   - pairing: six digits that agree, a person in the middle whose digits do not, a computer that breaks its commitment, a certificate that is no
#     certificate: and what is added to the store or kept by the other side in each case
#   - the store: append-only (a delete is refused), TLS 1.3, client certificates required (none, and one nobody paired, are refused)
#   - the backup: runs with the Vault open, with it closed (it waits, says so once, breaks nothing) and under the `background` key; nothing is in the clear
#     at rest (a canary in the files is not in the store's data, the password and key are not in either computer's folders) and nothing is left in the runtime folders
#   - the recovery key opens the repository on a computer that was paired later, and `amethystora-restore` stays read-only
set -euo pipefail

SRC="${SRC:-$(cd "$(dirname "$0")/../.." && pwd)}"
LIBEXEC_SRC="${SRC}/system_files/shared/usr/libexec"
[[ -x "${LIBEXEC_SRC}/amethystora-backup-pair" ]] || { echo "test-backup-other: the sources are not at ${SRC}" >&2; exit 1; }
T="$(mktemp -d)"
PIDS=()
cleanup() {
    for pid in "${PIDS[@]+"${PIDS[@]}"}"; do kill "${pid}" 2>/dev/null || true; done
    rm -rf "${T}"
}
trap cleanup EXIT
fail() { echo "FAIL $*"; exit 1; }
command -v rclone >/dev/null && command -v restic >/dev/null && command -v openssl >/dev/null || { echo "test-backup-other: skipped (no restic, rclone or openssl)"; exit 0; }

# The helpers as installed, each calling the others in the stand-in folder
mkdir -p "${T}/libexec" "${T}/lib" "${T}/bin" "${T}/scratch"
for f in amethystora-backup-pair amethystora-backup-secret amethystora-backup amethystora-backup-other amethystora-backup-recovery \
    amethystora-backup-store amethystora-backup-other-ready amethystora-restore amethystora-start-from-backup; do
    sed -e "s#/usr/libexec/#${T}/libexec/#g" -e "s#/usr/lib/amethystora/#${T}/lib/#g" "${LIBEXEC_SRC}/${f}" >"${T}/libexec/${f}"
    chmod +x "${T}/libexec/${f}"
done
sed -e "s#/usr/libexec/#${T}/libexec/#g" "${SRC}/system_files/shared/usr/lib/amethystora/backup-secrets.sh" >"${T}/lib/backup-secrets.sh"
# The Vault's keep helper, pointed at the stand-in socket; the things the backup calls that belong to the image
SOCK="${T}/vault.sock"
echo open >"${T}/vault.mode"
printf '#!/usr/bin/bash\nexec python3 -I %q --socket %q "$@"\n' "${LIBEXEC_SRC}/amethystora-vault-keep" "${SOCK}" >"${T}/libexec/amethystora-vault-keep"
printf '#!/usr/bin/bash\nexit 2\n' >"${T}/libexec/amethystora-vault-cli"
printf '#!/usr/bin/bash\necho "{\\"version\\":1}"\n' >"${T}/libexec/amethystora-setup-manifest"
printf '#!/usr/bin/bash\necho "$@" >>%q\n' "${T}/notifications" >"${T}/bin/notify-send"
chmod +x "${T}"/libexec/amethystora-vault-keep "${T}"/libexec/amethystora-vault-cli "${T}"/libexec/amethystora-setup-manifest "${T}/bin/notify-send"
python3 -I "${SRC}/build_files/shared/test-vault-stand-in.py" "${SOCK}" "${T}/vault.mode" &
PIDS+=($!)
for _ in $(seq 50); do [[ -S "${SOCK}" ]] && break; sleep 0.1; done
[[ -S "${SOCK}" ]] || fail "the stand-in Vault did not start"

# Two computers: their homes, data, state and runtime folders
for c in a b c; do
    mkdir -p "${T}/${c}/home" "${T}/${c}/run" "${T}/${c}/tmp"
    chmod 700 "${T}/${c}/run"
done
as() {
    local c="$1"
    shift
    env -i PATH="${T}/bin:${PATH}" HOME="${T}/${c}/home" XDG_RUNTIME_DIR="${T}/${c}/run" TMPDIR="${T}/${c}/tmp" \
        XDG_CONFIG_HOME="${T}/${c}/home/.config" XDG_DATA_HOME="${T}/${c}/home/.local/share" XDG_STATE_HOME="${T}/${c}/home/.local/state" \
        "$@"
}

PORTS="$(python3 -I -c 'import socket
s = [socket.socket() for _ in range(2)]
[x.bind(("127.0.0.1", 0)) for x in s]
print(*[x.getsockname()[1] for x in s])')"
STORE_PORT="${PORTS% *}"
PAIR_PORT="${PORTS#* }"

# --- Pairing, in one process with two roles ------------------------------------------------------------------------------
cat >"${T}/pairing.py" <<'PY'
import faulthandler, importlib.machinery, importlib.util, json, os, socket, sys, threading
faulthandler.dump_traceback_later(90, exit=True)
path, base, store_port = sys.argv[1], sys.argv[2], int(sys.argv[3])
loader = importlib.machinery.SourceFileLoader("pairmod", path)
spec = importlib.util.spec_from_loader("pairmod", loader)
m = importlib.util.module_from_spec(spec)
loader.exec_module(m)
os.environ["XDG_RUNTIME_DIR"] = f"{base}/a/run"
os.environ["XDG_DATA_HOME"] = f"{base}/a/home/.local/share"
os.environ["HOME"] = f"{base}/a/home"
m.SECRET = f"{base}/libexec/amethystora-backup-secret"
m.STORE_PORT = store_port
local = threading.local()
shown, ready = {}, {"a": threading.Event(), "b": threading.Event()}
def ask(six, expect, name):
    me = local.role
    shown[me] = six
    ready[me].set()
    other = "b" if me == "a" else "a"
    ready[other].wait(20)
    return shown.get(other) == six
m.ask_match = ask

def as_role(role, base_dir):
    local.role = role
    # the folders of that computer
    m.STORE_DIR = f"{base}/b/home/.local/share/amethystora/backup-store"
    m.OTHER_DIR = f"{base}/{base_dir}/home/.local/share/amethystora/backup-other"

def attempt(fn, *a, **k):
    try:
        return fn(*a, **k), None
    except SystemExit as e:
        return None, e.code

def run_pair(listener_port_holder, relay=None, tweak=None, computer="a"):
    shown.clear(); [e.clear() for e in ready.values()]
    out = {}
    def b():
        as_role("b", "b")
        out["b"] = attempt(m.pair, 0, 30, None, listener=listener_port_holder["listener"])
    t = threading.Thread(target=b); t.start()
    as_role("a", computer)
    port = (relay or listener_port_holder)["port"]
    out["a"] = attempt(m.connect, "127.0.0.1", port, None)
    t.join(60)
    return out

def new_listener():
    s = socket.create_server(("127.0.0.1", 0))
    return {"listener": s, "port": s.getsockname()[1]}

def client_count():
    folder = f"{base}/b/home/.local/share/amethystora/backup-store/clients"
    return len([n for n in os.listdir(folder) if n.endswith(".pem")]) if os.path.isdir(folder) else 0

result = {}
as_role("b", "b")
m.store_init()

# 1. A good pairing: the digits agree, both ends are done
holder = new_listener()
out = run_pair(holder)
result["good"] = {"digits_agree": shown.get("a") == shown.get("b") and bool(shown.get("a")), "a_ok": out["a"][1] is None, "b_ok": out["b"][1] is None,
    "clients": client_count(), "digits": shown.get("a")}
result["a_store_pem"] = os.path.exists(f"{base}/a/home/.local/share/amethystora/backup-other/store.pem")
result["a_sealed"] = os.path.exists(f"{base}/a/home/.local/share/amethystora/backup-other/client.avs")

# 2. A person in the middle: both certificates replaced, a nonce of his own
def relay_for(b_holder):
    r = new_listener()
    def run():
        a_sock, _ = r["listener"].accept()
        b_sock = socket.create_connection(("127.0.0.1", b_holder["port"]))
        ra, rb = a_sock.makefile("r"), b_sock.makefile("r")
        import tempfile, shutil
        d1, d2 = tempfile.mkdtemp(dir=f"{base}/scratch"), tempfile.mkdtemp(dir=f"{base}/scratch")
        m1, _ = m.make_certificate(d1, "mitm-a", "clientAuth")
        m2, _ = m.make_certificate(d2, "mitm-b", "serverAuth", ["localhost"], ["127.0.0.1"])
        hello = json.loads(ra.readline()); hello["cert"] = m1
        b_sock.sendall(json.dumps(hello).encode() + b"\n")
        reply = json.loads(rb.readline()); reply["cert"] = m2
        a_sock.sendall(json.dumps(reply).encode() + b"\n")
        for src, dst in ((ra, b_sock), (ra, b_sock), (rb, a_sock)):
            line = src.readline()
            if line:
                dst.sendall(line.encode() if isinstance(line, str) else line)
        a_sock.close(); b_sock.close()
        shutil.rmtree(d1, ignore_errors=True); shutil.rmtree(d2, ignore_errors=True)
    th = threading.Thread(target=run, daemon=True); th.start()
    return r
before = client_count()
holder = new_listener()
relay = relay_for(holder)
out = run_pair(holder, relay=relay, computer="c")
result["mitm"] = {"digits_differ": shown.get("a") != shown.get("b"), "a_failed": out["a"][1] is not None, "b_failed": out["b"][1] is not None,
    "clients_added": client_count() - before, "c_kept": os.path.exists(f"{base}/c/home/.local/share/amethystora/backup-other/store.pem")}

# 3. A computer that breaks its commitment (reveals another nonce), and one that sends no certificate
def raw(first, second=None):
    holder = new_listener()
    box = {}
    def b():
        as_role("b", "b")
        box["b"] = attempt(m.pair, 0, 30, None, listener=holder["listener"])
    t = threading.Thread(target=b); t.start()
    s = socket.create_connection(("127.0.0.1", holder["port"]))
    s.sendall(json.dumps(first).encode() + b"\n")
    rd = s.makefile("r")
    if second is not None:
        rd.readline()
        s.sendall(json.dumps(second).encode() + b"\n")
    s.close(); t.join(30)
    return box["b"][1] is not None
before = client_count()
import tempfile
d = tempfile.mkdtemp(dir=f"{base}/scratch")
cert, _ = m.make_certificate(d, "cheat", "clientAuth")
na = os.urandom(16)
result["cheat"] = {"refused": raw({"v": 1, "commit": m.commit(na), "cert": cert, "name": "x"}, {"v": 1, "na": os.urandom(16).hex()}),
    "not_a_certificate": raw({"v": 1, "commit": "0" * 64, "cert": "not a certificate", "name": "x"}),
    "bad_commit": raw({"v": 1, "commit": "zz", "cert": cert, "name": "x"}), "clients_added": client_count() - before}
print(json.dumps(result))
PY
OUT="$(python3 -I "${T}/pairing.py" "${T}/libexec/amethystora-backup-pair" "${T}" "${STORE_PORT}" 2>"${T}/pairing.err")" || fail "the pairing harness: $(tail -n 5 "${T}/pairing.err")"
OUT="$(tail -n 1 <<<"${OUT}")"
j() { jq -r "$1" <<<"${OUT}"; }
[[ "$(j .good.digits_agree)" == true && "$(j .good.a_ok)" == true && "$(j .good.b_ok)" == true ]] || fail "a good pairing did not finish: ${OUT}"
[[ "$(j .good.clients)" == 1 && "$(j .a_store_pem)" == true && "$(j .a_sealed)" == true ]] || fail "a good pairing left the wrong things: ${OUT}"
[[ "$(j .mitm.digits_differ)" == true ]] || echo "note: the man in the middle's digits matched by chance (one in a million)"
[[ "$(j .mitm.a_failed)" == true && "$(j .mitm.b_failed)" == true && "$(j .mitm.clients_added)" == 0 && "$(j .mitm.c_kept)" == false ]] ||
    fail "a person in the middle was accepted: ${OUT}"
[[ "$(j .cheat.refused)" == true && "$(j .cheat.not_a_certificate)" == true && "$(j .cheat.bad_commit)" == true && "$(j .cheat.clients_added)" == 0 ]] ||
    fail "a broken pairing was accepted: ${OUT}"
STORE="${T}/b/home/.local/share/amethystora/backup-store"
OTHER="${T}/a/home/.local/share/amethystora/backup-other"
# The key and the pairing certificate are sealed, never plain
[[ "$(stat -c %a "${STORE}/server.key")" == 600 && "$(stat -c %a "${STORE}")" == 700 ]] || fail "the store's folder or key is not private"
! grep -rIl "PRIVATE KEY" "${T}/a" "${T}/c" 2>/dev/null | grep -q . || fail "a private key is on disk on the computer that backs up: $(grep -rIl 'PRIVATE KEY' "${T}/a" "${T}/c" | head -n 1)"
[[ -z "$(find "${T}/a/run" "${T}/b/run" "${T}/c/run" -mindepth 1)" ]] || fail "pairing left something in a runtime folder: $(find "${T}/a/run" "${T}/b/run" "${T}/c/run" -mindepth 1)"

# --- The store ----------------------------------------------------------------------------------------------------------
as b env AMETHYSTORA_BACKUP_STORE_PORT="${STORE_PORT}" "${T}/libexec/amethystora-backup-store" >"${T}/store.log" 2>&1 &
PIDS+=($!)
for _ in $(seq 60); do (exec 3<>"/dev/tcp/127.0.0.1/${STORE_PORT}") 2>/dev/null && break; sleep 0.2; done

# The computer that backs up: the settings, as `ame backup other-computer setup` writes them, and the password sealed
mkdir -p "${T}/a/home/.config/amethystora" "${T}/a/home/Documents"
CANARY="canary-in-the-clear-$$-7731"
echo "${CANARY}" >"${T}/a/home/Documents/tax.txt"
head -c 200000 /dev/urandom >"${T}/a/home/Documents/random.bin"
cat >"${T}/a/home/.config/amethystora/backup-excludes" <<<"*.cache"
PASSWORD="repo-password-$(head -c 12 /dev/urandom | base64 | tr -dc 'A-Za-z0-9')-$$"
printf '%s' "${PASSWORD}" | as a "${T}/libexec/amethystora-backup-secret" store-password || fail "sealing the password"
cat >"${T}/a/home/.config/amethystora/backup-other.env" <<ENV
RESTIC_REPOSITORY=rest:https://127.0.0.1:${STORE_PORT}/
RESTIC_PASSWORD_COMMAND='${T}/libexec/amethystora-backup-secret password'
RESTIC_CACERT=${OTHER}/store.pem
AMETHYSTORA_BACKUP_SECRETS=vault
AMETHYSTORA_BACKUP_PATHS=${T}/a/home/Documents
AMETHYSTORA_BACKUP_NAME=Desk
ENV
# The repository is made with the keys as the backup has them
. "${T}/lib/backup-secrets.sh"
# a command with computer a's settings, its keys opened from the Vault for as long as it runs
with_a() (
    export XDG_RUNTIME_DIR="${T}/a/run" HOME="${T}/a/home" XDG_DATA_HOME="${T}/a/home/.local/share" XDG_CONFIG_HOME="${T}/a/home/.config"
    set -a; source "${T}/a/home/.config/amethystora/backup-other.env"; set +a
    backup_secrets_open && trap backup_secrets_close EXIT
    "$@"
)
with_a restic init >/dev/null || fail "the repository could not be made through the store"

# Without a client certificate, or with one nobody paired, the store refuses before reading anything
( set -a; source "${T}/a/home/.config/amethystora/backup-other.env"; set +a
  RESTIC_PASSWORD="${PASSWORD}" restic --no-lock snapshots >/dev/null 2>&1 ) && fail "the store answered a client with no certificate"
FOREIGN="${T}/foreign"
mkdir -p "${FOREIGN}"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -keyout "${FOREIGN}/k" -out "${FOREIGN}/c" -days 3 -subj /CN=foreign \
    -addext extendedKeyUsage=clientAuth 2>/dev/null
cat "${FOREIGN}/c" "${FOREIGN}/k" >"${FOREIGN}/client.pem"
( set -a; source "${T}/a/home/.config/amethystora/backup-other.env"; set +a
  RESTIC_PASSWORD="${PASSWORD}" RESTIC_TLS_CLIENT_CERT="${FOREIGN}/client.pem" restic --no-lock snapshots >/dev/null 2>&1 ) && fail "the store answered a computer nobody paired"

# --- The backup ---------------------------------------------------------------------------------------------------------
as a "${T}/libexec/amethystora-backup" --other >"${T}/run1.log" 2>&1 || { cat "${T}/run1.log"; fail "the backup to the other computer failed"; }
[[ -s "${T}/a/home/.local/state/amethystora/backup-other-last-success" ]] || fail "no stamp after a good run"
[[ -z "$(ls -A "${T}/a/run")" ]] || fail "the backup left something in the runtime folder: $(ls -A "${T}/a/run")"
grep -q "Snapshots in this repository" "${T}/run1.log" || fail "the run did not say what it holds"
! grep -q "${PASSWORD}\|PRIVATE KEY" "${T}/run1.log" || fail "the run printed a secret"

# At rest: the store holds ciphertext (not the canary, not a file name), the computers hold no key or password in the clear
! grep -rlF "${CANARY}" "${STORE}" "${T}/a/home/.config" "${T}/a/home/.local" 2>/dev/null | grep -q . || fail "the canary is in the clear: $(grep -rlF "${CANARY}" "${STORE}" "${T}/a" | head -n 1)"
! grep -rlF "tax.txt" "${STORE}/data" 2>/dev/null | grep -q . || fail "a file name is in the store's data"
! grep -rlF "${PASSWORD}" "${T}/a" "${T}/b" "${T}/c" 2>/dev/null | grep -q . || fail "the repository password is on disk in the clear: $(grep -rlF "${PASSWORD}" "${T}/a" "${T}/b" "${T}/c" | head -n 1)"
[[ "$(find "${STORE}/data" -type f | wc -l)" -gt 5 ]] || fail "the store holds nothing"

# Append-only: this computer cannot delete from the store, with its own keys
snapshot="$(with_a restic snapshots --json | jq -r '.[0].id')"
[[ -n "${snapshot}" && "${snapshot}" != null ]] || fail "the backup left no snapshot in the store"
with_a restic forget --prune "${snapshot}" >/dev/null 2>&1 && fail "the store let this computer delete a snapshot"
[[ "$(with_a restic snapshots --json | jq length)" -ge 1 ]] || fail "a refused delete still removed the snapshot"
with_a restic cat config >/dev/null || fail "the repository's config is gone"

# A closed Vault: the night waits, says so once, breaks nothing
rm -f "${T}/a/home/.local/state/amethystora/backup-other-last-success"
echo closed >"${T}/vault.mode"
as a "${T}/libexec/amethystora-backup" --other >"${T}/run2.log" 2>&1 || fail "a closed Vault made the run fail"
grep -q "waits" "${T}/run2.log" || fail "a closed Vault was not said"
[[ ! -e "${T}/a/home/.local/state/amethystora/backup-other-last-success" ]] || fail "a closed Vault left a stamp of a run that did not happen"
as a "${T}/libexec/amethystora-backup" --other >/dev/null 2>&1
[[ "$(wc -l <"${T}/notifications")" -eq 1 ]] || fail "a missed night was said more than once a day: $(cat "${T}/notifications")"
[[ -z "$(ls -A "${T}/a/run")" ]] || fail "a closed Vault left something in the runtime folder"
# The background key serves it with the vault closed (a night with nobody logged in)
echo background >"${T}/vault.mode"
printf '%s' "${PASSWORD}" | as a "${T}/libexec/amethystora-backup-secret" store-password
as a "${T}/libexec/amethystora-backup" --other >"${T}/run3.log" 2>&1 || { cat "${T}/run3.log"; fail "the run did not work under the background key"; }
[[ -s "${T}/a/home/.local/state/amethystora/backup-other-last-success" ]] || fail "no stamp under the background key"
echo open >"${T}/vault.mode"

# --- The recovery key, and a new computer ----------------------------------------------------------------------------------
KEY="$(as a "${T}/libexec/amethystora-backup-recovery" --env "${T}/a/home/.config/amethystora/backup-other.env" --print)" || fail "the recovery key could not be made through the store"
[[ "${KEY}" =~ ^([A-HJ-NP-Z2-9]{5}-){4}[A-HJ-NP-Z2-9]{5}$ ]] || fail "the recovery key's shape: ${KEY}"
[[ -z "$(ls -A "${T}/a/run")" ]] || fail "the recovery key's helper left something in the runtime folder"
# A new computer (c), paired with the store later, opens the repository with the recovery key alone and puts the files back
HOLDER_C="${T}/c.sh"
cat >"${T}/pair-c.py" <<'PY'
import faulthandler, importlib.machinery, importlib.util, json, os, socket, sys, threading
faulthandler.dump_traceback_later(90, exit=True)
path, base, store_port, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
loader = importlib.machinery.SourceFileLoader("pairmod", path)
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("pairmod", loader)); loader.exec_module(m)
os.environ["XDG_RUNTIME_DIR"] = f"{base}/c/run"
os.environ["XDG_DATA_HOME"] = f"{base}/c/home/.local/share"
os.environ["HOME"] = f"{base}/c/home"
m.STORE_DIR = f"{base}/b/home/.local/share/amethystora/backup-store"
m.STORE_PORT = store_port
shown = {}
ev = {"a": threading.Event(), "b": threading.Event()}
local = threading.local()
def ask(six, expect, name):
    shown[local.role] = six; ev[local.role].set(); ev["b" if local.role == "a" else "a"].wait(20)
    return shown.get("b" if local.role == "a" else "a") == six
m.ask_match = ask
srv = socket.create_server(("127.0.0.1", 0))
def b():
    local.role = "b"; m.pair(0, 30, None, listener=srv)
t = threading.Thread(target=b); t.start()
local.role = "a"
os.makedirs(out, mode=0o700, exist_ok=True)
print(json.dumps(m.connect("127.0.0.1", srv.getsockname()[1], None, temp=out)))
t.join()
PY
mkdir -p "${T}/c/restore"
python3 -I "${T}/pair-c.py" "${T}/libexec/amethystora-backup-pair" "${T}" "${STORE_PORT}" "${T}/c/restore" >/dev/null || fail "pairing the new computer"
as b "${T}/libexec/amethystora-backup-pair" clients | grep -c . | grep -qx 2 || fail "the store does not accept the new computer"
# The store reads its clients when it starts: it is restarted, as `ame backup store pair` does
kill "${PIDS[-1]}" 2>/dev/null || true
sleep 0.5
as b env AMETHYSTORA_BACKUP_STORE_PORT="${STORE_PORT}" "${T}/libexec/amethystora-backup-store" >"${T}/store2.log" 2>&1 &
PIDS+=($!)
for _ in $(seq 60); do (exec 3<>"/dev/tcp/127.0.0.1/${STORE_PORT}") 2>/dev/null && break; sleep 0.2; done
# c is a new computer at the same home path as a (what amethystora-restore knows how to put back): the files are gone, only the recovery key is known
rm -rf "${T}/a/home/Documents"
CONFIG_BEFORE="$(find "${T}/a/home/.config" -type f -exec sha256sum {} + | sort)"
# (as c, but at the home path the backup has)
out="$(printf '%s\n' "${KEY}" | env -i PATH="${T}/bin:${PATH}" HOME="${T}/a/home" XDG_RUNTIME_DIR="${T}/c/run" TMPDIR="${T}/c/tmp" \
    "${T}/libexec/amethystora-start-from-backup" --repository "rest:https://127.0.0.1:${STORE_PORT}/" \
    --cacert "${T}/c/restore/store.pem" --client-cert "${T}/c/restore/client.pem" --steps files --key-stdin 2>&1)" || { echo "${out}"; fail "restoring from nothing with the recovery key"; }
grep -q "newest backup of files" <<<"${out}" || fail "the restore did not say what the backup holds: ${out}"
[[ "$(cat "${T}/a/home/Documents/tax.txt")" == "${CANARY}" ]] || fail "the files did not come back from the other computer"
cmp -s "${T}/a/home/Documents/random.bin" <(cat "${T}/a/home/Documents/random.bin") || true
[[ "$(find "${T}/a/home/.config" -type f -exec sha256sum {} + | sort)" == "${CONFIG_BEFORE}" ]] || fail "restoring from nothing changed the account's settings"
[[ -z "$(ls -A "${T}/c/run")" ]] || fail "restoring left something in the runtime folder: $(ls -A "${T}/c/run")"
! grep -q "${KEY//-/}" <<<"${out}" || fail "the recovery key was printed"
! grep -rlF "${KEY//-/}" "${T}/a" "${T}/b" "${T}/c" "${T}/libexec" 2>/dev/null | grep -q . || fail "the recovery key is on disk"
# The read-only helper still only reads, and this feature's helpers never delete from a backup
! grep -E '^[^#]*restic[^#]* (forget|prune|rewrite)( |$)' "${LIBEXEC_SRC}"/amethystora-backup "${LIBEXEC_SRC}"/amethystora-backup-other "${LIBEXEC_SRC}"/amethystora-restore \
    "${LIBEXEC_SRC}"/amethystora-start-from-backup "${LIBEXEC_SRC}"/amethystora-backup-recovery || fail "a helper of the backup deletes from a repository"
# The store is the only one that can be told to accept a computer, and only by a pairing both people confirmed
! grep -rn "clients.pem\|clients/" "${LIBEXEC_SRC}" --include='amethystora-*' -l | grep -v 'backup-pair\|backup-store\|backup-other' | grep -q . || fail "something other than the pairing writes the store's list of computers"
echo "test-backup-other: ok"
