#!/usr/bin/bash
# Bring everything from your old computer (R7-48): /usr/libexec/amethystora-bring-over against stand-ins, run from 20-tests.sh and by
# hand (`bash build_files/shared/test-bring-over.sh [HELPER]`). A disk of another computer is a folder tree here, lsblk and
# udisksctl are stubs that log what they were asked, and rsync is the real one. It checks what the helper promises:
#  - the disk is mounted read-only (always `-o ro`), nothing under it changes, and a mount the desktop made is unmounted first
#  - what is on it is only believed as far as plain folders and files: no link is followed or copied, a folder that is a link is passed over
#  - it adds and never replaces, and a second run carries on with what is missing
#  - no password is ever opened (the stand-ins' logins are unreadable and hold canaries) and the recovery key goes on a pipe only
set -uo pipefail

HELPER="${1:-$(dirname "$0")/../../system_files/shared/usr/libexec/amethystora-bring-over}"
T="$(mktemp -d)"
trap 'chmod -R u+rwX "${T}" 2>/dev/null; rm -rf "${T}"' EXIT
fail() {
    echo "test-bring-over: $*" >&2
    exit 1
}

OLD="${T}/old"
NEW="${T}/home"
mkdir -p "${NEW}/Documents" "${T}/stubs" "${T}/log"
export HOME="${NEW}" XDG_CONFIG_HOME="${NEW}/.config"
export PATH="${T}/stubs:${PATH}"

# --- A Windows disk's stand-in --------------------------------------------------------------------------------------------------
SAM="${OLD}/Users/Sam"
mkdir -p "${SAM}/Documents/letters" "${SAM}/Pictures" "${SAM}/Music" "${SAM}/Videos" "${OLD}/Users/Public/Documents" "${OLD}/Users/Default" \
    "${SAM}/AppData/Local/Google/Chrome/User Data/Default" "${SAM}/AppData/Roaming/Mozilla/Firefox/Profiles/x.default" "${OLD}/Windows"
echo "the report" >"${SAM}/Documents/report.docx"
echo "dear Sam" >"${SAM}/Documents/letters/a.txt"
echo hidden >"${SAM}/Documents/desktop.ini"
echo thumbs >"${SAM}/Documents/Thumbs.db"
echo "lock" >"${SAM}/Documents/~\$report.docx"
head -c 300000 /dev/zero >"${SAM}/Music/song.mp3"
echo "a picture" >"${SAM}/Pictures/p.jpg"
echo "a film" >"${SAM}/Videos/v.mp4"
echo "public" >"${OLD}/Users/Public/Documents/x.txt"
# Links out of the disk: a file, a folder inside Documents, and a whole folder that is a link
ln -s /etc/hostname "${SAM}/Documents/hostname-link"
ln -s /etc "${SAM}/Documents/etc-folder"
ln -s /etc "${SAM}/Downloads"
# What a browser keeps: bookmarks, which may be read, and logins, which never are (unreadable, with canaries inside)
cat >"${SAM}/AppData/Local/Google/Chrome/User Data/Default/Bookmarks" <<'EOF'
{"roots":{"bookmark_bar":{"name":"Bookmarks bar","type":"folder","children":[
  {"name":"Fedora <docs>","type":"url","url":"https://docs.fedoraproject.org/?a=1&b=2"},
  {"name":"A folder","type":"folder","children":[{"name":"Example","type":"url","url":"http://example.org/"}]},
  {"name":"evil","type":"url","url":"javascript:alert(1)"}]},"other":{"name":"Other","type":"folder","children":[]}}}
EOF
echo "CANARY-CHROME-PASSWORD" >"${SAM}/AppData/Local/Google/Chrome/User Data/Default/Login Data"
echo "CANARY-CHROME-COOKIE" >"${SAM}/AppData/Local/Google/Chrome/User Data/Default/Cookies"
python3 - "${SAM}/AppData/Roaming/Mozilla/Firefox/Profiles/x.default/places.sqlite" <<'EOF'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.executescript("""
CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT);
CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, title TEXT, guid TEXT);
INSERT INTO moz_places VALUES (1, 'https://www.mozilla.org/'), (2, 'https://docs.fedoraproject.org/?a=1&b=2'), (3, 'place:type=6');
INSERT INTO moz_bookmarks VALUES (1, 2, NULL, 0, '', 'root________'), (2, 2, NULL, 1, 'menu', 'menu________'), (3, 2, NULL, 1, 'toolbar', 'toolbar_____'),
  (4, 2, NULL, 1, 'tags', 'tags________'), (10, 1, 1, 3, 'Mozilla', 'aaaaaaaaaaaa'), (11, 1, 2, 3, 'Fedora <docs>', 'bbbbbbbbbbbb'),
  (12, 1, 3, 3, 'smart', 'cccccccccccc'), (13, 1, 1, 4, 'tagged copy', 'dddddddddddd');
""")
db.commit()
EOF
echo "CANARY-FIREFOX-LOGINS" >"${SAM}/AppData/Roaming/Mozilla/Firefox/Profiles/x.default/logins.json"
echo "CANARY-FIREFOX-KEYS" >"${SAM}/AppData/Roaming/Mozilla/Firefox/Profiles/x.default/key4.db"
chmod 000 "${SAM}/AppData/Local/Google/Chrome/User Data/Default/Login Data" "${SAM}/AppData/Local/Google/Chrome/User Data/Default/Cookies" \
    "${SAM}/AppData/Roaming/Mozilla/Firefox/Profiles/x.default/logins.json" "${SAM}/AppData/Roaming/Mozilla/Firefox/Profiles/x.default/key4.db"
# Something already on the new computer under a name the disk has
echo "the new computer's own report" >"${NEW}/Documents/report.docx"

tree_state() { find "${OLD}" -printf '%p %y %s %m %T@\n' | sort; }
BEFORE="$(tree_state)"

# --- Scan ---------------------------------------------------------------------------------------------------------------------------
JSON="$("${HELPER}" scan "${OLD}" --json)" || fail "scan failed"
[[ "$(jq -r '[.[].name] | join(",")' <<<"${JSON}")" == "Sam" ]] || fail "the people of the disk: $(jq -c '[.[].name]' <<<"${JSON}")"
[[ "$(jq -r '.[0].folders | map(.id) | join(",")' <<<"${JSON}")" == "documents,pictures,music,videos" ]] ||
    fail "the folders: $(jq -c '.[0].folders | map(.id)' <<<"${JSON}") (a link is not a folder, and Downloads here is one)"
[[ "$(jq -r '.[0].folders[0].files' <<<"${JSON}")" == 2 ]] || fail "Documents has two files that count (a link, desktop.ini, Thumbs.db and a lock file do not): $(jq -c '.[0].folders[0]' <<<"${JSON}")"
[[ "$(jq -r '.[0].bookmarks' <<<"${JSON}")" == 3 ]] || fail "bookmarks counted: $(jq -r '.[0].bookmarks' <<<"${JSON}") (two in the first browser, one more in the second, a bookmark both have once, no script, no place:, no tag)"

# --- Bring ----------------------------------------------------------------------------------------------------------------------------
"${HELPER}" bring "${OLD}" Sam --folders documents,pictures,music,downloads,bogus --bookmarks >"${T}/log/bring.out" 2>&1 || fail "bring failed: $(cat "${T}/log/bring.out")"
[[ "$(cat "${NEW}/Documents/letters/a.txt")" == "dear Sam" ]] || fail "a file of Documents did not come"
[[ "$(cat "${NEW}/Documents/report.docx")" == "the new computer's own report" ]] || fail "a file that was there was replaced"
[[ -f "${NEW}/Pictures/p.jpg" && -f "${NEW}/Music/song.mp3" ]] || fail "Pictures and Music did not come"
[[ ! -e "${NEW}/Videos" ]] || fail "Videos came and was not chosen"
for left in desktop.ini Thumbs.db 'hostname-link' 'etc-folder' '~$report.docx'; do
    [[ ! -e "${NEW}/Documents/${left}" && ! -L "${NEW}/Documents/${left}" ]] || fail "${left} was copied"
done
[[ ! -e "${NEW}/Downloads" || -z "$(ls -A "${NEW}/Downloads")" ]] || fail "the Downloads that is a link to /etc was followed"
[[ "$(stat -c %a "${NEW}/Documents/letters/a.txt")" == 644 ]] || fail "a copied file's mode is not plain"
[[ ! -e "${NEW}/Documents/.amethystora-partial" ]] || fail "rsync's partial folder is left behind"
[[ "$(tree_state)" == "${BEFORE}" ]] || fail "the disk changed"
BOOKMARKS="$(ls "${NEW}/Documents/"Bookmarks*.html 2>/dev/null)"
[[ -n "${BOOKMARKS}" ]] || fail "no bookmarks file: $(ls -A "${NEW}/Documents")"
grep -q 'HREF="https://docs.fedoraproject.org/?a=1&amp;b=2"' "${BOOKMARKS}" || fail "a bookmark is missing or not escaped"
grep -q 'HREF="http://example.org/"' "${BOOKMARKS}" && grep -q 'HREF="https://www.mozilla.org/"' "${BOOKMARKS}" || fail "bookmarks of both browsers"
grep -qi 'javascript:\|place:' "${BOOKMARKS}" && fail "a script or a place made it into the bookmarks"
[[ "$(grep -c 'docs.fedoraproject' "${BOOKMARKS}")" == 1 ]] || fail "a bookmark that both browsers have is there twice"
[[ "$(stat -c %a "${BOOKMARKS}")" == 600 ]] || fail "the bookmarks file is not private"
# No canary of a password anywhere in the new home
if grep -rq 'CANARY' "${NEW}"; then fail "a password's canary reached the new computer: $(grep -rl CANARY "${NEW}")"; fi
# A second bookmarks file never replaces the first
"${HELPER}" bring "${OLD}" Sam --bookmarks >/dev/null 2>&1 || fail "second bring failed"
[[ "$(ls "${NEW}/Documents/"Bookmarks*.html | wc -l)" == 2 ]] || fail "the second run did not make a file of its own"

# --- Carrying on ---------------------------------------------------------------------------------------------------------------------
# A file that came whole is not copied again, and one that was not there yet is
echo "a late one" >"${SAM}/Pictures/late.jpg"
rm -f "${SAM}/Pictures/late.jpg.pre"
BEFORE="$(tree_state)"
echo "changed on the disk" >"${SAM}/Pictures/p.jpg"
BEFORE="$(tree_state)"
"${HELPER}" bring "${OLD}" Sam --folders pictures >/dev/null 2>&1 || fail "third bring failed"
[[ "$(cat "${NEW}/Pictures/late.jpg")" == "a late one" ]] || fail "a new file was not picked up on the next run"
[[ "$(cat "${NEW}/Pictures/p.jpg")" == "a picture" ]] || fail "a file that is here was replaced by the disk's changed one"
[[ "$(tree_state)" == "${BEFORE}" ]] || fail "the disk changed on the next run"

# --- A person that is not there, and nonsense ------------------------------------------------------------------------------------------
"${HELPER}" bring "${OLD}" Nobody --folders documents >/dev/null 2>&1 && fail "bring for a person the disk does not have"
"${HELPER}" bogus >/dev/null 2>&1
[[ $? == 64 ]] || fail "usage exits 64"
"${HELPER}" >/dev/null 2>&1 </dev/null
[[ $? == 1 ]] || fail "without a terminal it must refuse"

# --- Disks: lsblk and udisksctl as stand-ins -------------------------------------------------------------------------------------------------
cat >"${T}/stubs/lsblk" <<EOF
#!/bin/sh
cat <<'JSON'
{"blockdevices":[
 {"name":"nvme0n1","path":"/dev/nvme0n1","type":"disk","fstype":null,"size":512000000000,"model":"Own SSD","rm":false,"mountpoints":[null],"children":[
   {"name":"nvme0n1p1","path":"/dev/nvme0n1p1","type":"part","fstype":"vfat","size":600000000,"mountpoints":["/boot/efi"],"pkname":"nvme0n1","rm":false},
   {"name":"nvme0n1p3","path":"/dev/nvme0n1p3","type":"part","fstype":"btrfs","size":500000000000,"mountpoints":["/var","/var/home","/sysroot"],"pkname":"nvme0n1","rm":false}]},
 {"name":"sda","path":"/dev/sda","type":"disk","fstype":null,"size":1000000000000,"model":"Old Laptop HDD","rm":false,"mountpoints":[null],"children":[
   {"name":"sda1","path":"/dev/sda1","type":"part","fstype":"ntfs","size":999000000000,"label":"Windows\u001b[31m","mountpoints":["/run/media/me/Windows"],"pkname":"sda","rm":false}]},
 {"name":"sdb","path":"/dev/sdb","type":"disk","fstype":null,"size":500000000000,"model":"Locked","rm":true,"mountpoints":[null],"children":[
   {"name":"sdb1","path":"/dev/sdb1","type":"part","fstype":"BitLocker","size":499000000000,"mountpoints":[null],"pkname":"sdb","rm":true}]},
 {"name":"sdc","path":"/dev/sdc","type":"disk","fstype":null,"size":500000000000,"model":"Linux","rm":false,"mountpoints":[null],"children":[
   {"name":"sdc1","path":"/dev/sdc1","type":"part","fstype":"crypto_LUKS","size":499000000000,"mountpoints":[null],"pkname":"sdc","rm":false},
   {"name":"sdc2","path":"/dev/sdc2","type":"part","fstype":"swap","size":8000000000,"mountpoints":["[SWAP]"],"pkname":"sdc","rm":false}]},
 {"name":"zram0","path":"/dev/zram0","type":"disk","fstype":"swap","size":8000000000,"mountpoints":["[SWAP]"],"rm":false}]}
JSON
EOF
chmod +x "${T}/stubs/lsblk"
LISTED="$("${HELPER}" disks --json)" || fail "disks failed"
[[ "$(jq -r 'map(.device) | join(",")' <<<"${LISTED}")" == "/dev/sda1,/dev/sdb1,/dev/sdc1" ]] || fail "the disks offered: $(jq -c 'map(.device)' <<<"${LISTED}") (not this system's, not swap, not zram)"
[[ "$(jq -r '.[0].label' <<<"${LISTED}")" == "Windows[31m" ]] || fail "a label is shown as text: $(jq -r '.[0].label' <<<"${LISTED}" | od -c | head -2)"
[[ "$(jq -r '.[1].encrypted' <<<"${LISTED}")" == true && "$(jq -r '.[2].supported' <<<"${LISTED}")" == false ]] || fail "BitLocker is asked for a key, LUKS is said to be unsupported"

# udisksctl logs its arguments, its standard input and whether the key is in its environment; it plays a disk that unlocks and mounts
cat >"${T}/stubs/udisksctl" <<EOF
#!/bin/bash
echo "ARGS \$*" >>"${T}/log/udisks.log"
if [[ "\$1" == unlock ]]; then
    echo "STDIN \$(cat)" >>"${T}/log/udisks.log"
    echo "Unlocked /dev/sdb1 as /dev/dm-0."
elif [[ "\$1" == mount ]]; then
    echo "Mounted \$3 at ${OLD}."
fi
env | grep -c "1111-2222" >>"${T}/log/udisks.env" || true
EOF
chmod +x "${T}/stubs/udisksctl"
# A pty plays the terminal: DRIVE HELPER EXPECT SEND... answers each prompt in order, and every later [Y/n] with the last answer
cat >"${T}/drive.py" <<'PY'
import os, pty, select, sys, time
helper, steps, again = sys.argv[1], list(zip(sys.argv[2::2], sys.argv[3::2])), sys.argv[-1]
pid, fd = pty.fork()
if pid == 0:
    os.execv(helper, [helper])
out, mark, deadline = b"", 0, time.time() + 60
while time.time() < deadline:
    ready, _, _ = select.select([fd], [], [], 10.0)
    if not ready:
        break
    try:
        data = os.read(fd, 4096)
    except OSError:
        break
    if not data:
        break
    out += data
    while True:
        if steps:
            expect, send = steps[0]
        elif out[mark:].rstrip().endswith(b"[Y/n]"):
            expect, send = "[Y/n]", again
        else:
            break
        if expect.encode() not in out[mark:]:
            break
        mark = len(out)
        if steps:
            steps.pop(0)
        os.write(fd, send.encode() + b"\r")
try:
    os.kill(pid, 9)
except OSError:
    pass
os.waitpid(pid, 0)
sys.stdout.write(out.decode("utf-8", "replace"))
PY
KEY="111111-222222-333333-444444-555555-666666-777777-888888"
rm -rf "${NEW}/Documents" "${NEW}/Pictures" "${NEW}/Music"
TEXT="$(python3 "${T}/drive.py" "${HELPER}" "Its number" 2 "> " "${KEY}" y)" || fail "the terminal run failed"
[[ "${TEXT}" == *"Done."* ]] || fail "the run in a terminal did not finish: ${TEXT: -800}"
[[ "${TEXT}" != *"${KEY}"* ]] || fail "the recovery key was echoed to the terminal"
grep -q "^ARGS unlock -b /dev/sdb1 --key-file /dev/stdin$" "${T}/log/udisks.log" || fail "the disk was not unlocked by a key on stdin: $(cat "${T}/log/udisks.log")"
grep -q "^STDIN ${KEY}$" "${T}/log/udisks.log" || fail "the key did not reach udisks on its standard input"
grep -q "^ARGS mount -b /dev/dm-0 -o ro$" "${T}/log/udisks.log" || fail "the unlocked disk was not mounted read-only: $(cat "${T}/log/udisks.log")"
grep -q "^ARGS mount" "${T}/log/udisks.log" && [[ "$(grep -c '^ARGS mount' "${T}/log/udisks.log")" == "$(grep '^ARGS mount' "${T}/log/udisks.log" | grep -c -- ' -o ro$')" ]] || fail "a mount was not read-only"
LAST="$(grep '^ARGS' "${T}/log/udisks.log" | tail -2 | tr '\n' '|')"
[[ "${LAST}" == "ARGS unmount -b /dev/dm-0|ARGS lock -b /dev/sdb1|" ]] || fail "the disk is not unmounted, then locked, at the end: ${LAST}"
[[ "$(grep -v '^STDIN' "${T}/log/udisks.log" | grep -c "${KEY}")" == 0 ]] || fail "the key is in an argument"
[[ ! -s "${T}/log/udisks.env" || "$(sort -u "${T}/log/udisks.env")" == 0 ]] || fail "the key is in the environment of udisksctl"
if grep -rqF "${KEY}" "${T}" --exclude-dir=log --exclude-dir=old 2>/dev/null; then fail "the recovery key was written to a file"; fi
[[ -f "${NEW}/Documents/letters/a.txt" && -f "${NEW}/Pictures/p.jpg" ]] || fail "the run in a terminal brought nothing"

# A disk the desktop mounted (writable) is unmounted first and mounted again read-only
: >"${T}/log/udisks.log"
python3 "${T}/drive.py" "${HELPER}" "Its number" 1 n >/dev/null || fail "the terminal run with a mounted disk failed"
[[ "$(grep '^ARGS' "${T}/log/udisks.log" | head -2 | tr '\n' '|')" == "ARGS unmount -b /dev/sda1|ARGS mount -b /dev/sda1 -o ro|" ]] ||
    fail "a mount the desktop made was not unmounted before the read-only mount: $(cat "${T}/log/udisks.log")"

echo "test-bring-over: ok"
