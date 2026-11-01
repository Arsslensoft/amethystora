#!/usr/bin/bash
# amethystora-image-policy (/usr/libexec/amethystora-image-policy, or the copy named first) against a stand-in for
# the image made of its own schemas and dconf files, for 20-tests.sh: agent.conf and the security switches reach
# /etc/amethystora as the machine's own commands write them, desktop.json reaches GNOME's defaults and, locked, dconf's
# database with its lock, an image extension turned off leaves the list every account starts with, and a folder with
# anything else, a line the agent or Security cannot read, a hardening switch a build cannot set, a setting the
# image does not have or a value out of its range is refused before anything is written. Hardening and the updates'
# schedule need a build, which the image template's own run tries.

set -euo pipefail

POLICY="${1:-/usr/libexec/amethystora-image-policy}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
ROOT="${T}/root"
DIR="${T}/policy"
UI=amethystora-ui@arsslensoft.github.io
CLIPBOARD=amethystora-clipboard@arsslensoft.github.io

stand_in() {
    rm -rf "${ROOT}" "${DIR}"
    mkdir -p "${ROOT}/usr/share/glib-2.0" "${ROOT}/etc/dconf/db" "${ROOT}/etc/amethystora" "${DIR}" \
        "${ROOT}/usr/share/gnome-shell/extensions/${UI}" "${ROOT}/usr/share/gnome-shell/extensions/${CLIPBOARD}"
    cp -r /usr/share/glib-2.0/schemas "${ROOT}/usr/share/glib-2.0/"
    cp -r /etc/dconf/db/distro.d "${ROOT}/etc/dconf/db/"
    cp /etc/amethystora/security.conf "${ROOT}/etc/amethystora/"
}
image() { GSETTINGS_SCHEMA_DIR="${ROOT}/usr/share/glib-2.0/schemas" GSETTINGS_BACKEND=memory /usr/bin/gsettings get "$@"; }
locked() { DCONF_PROFILE="${T}/profile" /usr/bin/dconf read "$1"; }
printf 'file-db:%s\n' "${ROOT}/etc/dconf/db/distro" >"${T}/profile"

stand_in
printf '# The company'"'"'s\nALLOW=know\nALLOW_FOR=admin:act\nROOT=each\n' >"${DIR}/agent.conf"
printf 'balanced\nPROCESSES=on\nNOTIFY=critical\n' >"${DIR}/security"
cat >"${DIR}/desktop.json" <<'EOF'
{
  "version": 1,
  "set": {
    "idle-delay": 600, "screen-lock": true, "lock-delay": 0, "camera-off": true, "clock-format": "24h",
    "keyboard-layouts": ["us", "fr+azerty"], "login-user-list-off": true,
    "login-banner": "Property of Acme.\nAuthorised use only.", "sleep-on-mains": 3600
  },
  "lock": ["idle-delay", "screen-lock", "camera-off"],
  "extensions-off": ["amethystora-clipboard@arsslensoft.github.io"]
}
EOF
echo "# Policy" >"${DIR}/README.md"
out="$("${POLICY}" "${DIR}" --root "${ROOT}")"
grep -q "desktop settings, 3 of them locked" <<<"${out}"
cmp -s "${DIR}/agent.conf" "${ROOT}/etc/amethystora/agent.conf"
grep -qx "PROCESSES=on" "${ROOT}/etc/amethystora/security.conf"
grep -qx "NOTIFY=critical" "${ROOT}/etc/amethystora/security.conf"
[[ "$(AMETHYSTORA_SECURITY_CONF="${ROOT}/etc/amethystora/security.conf" /usr/libexec/amethystora-security-config DECOY)" == off ]]
[[ "$(image org.gnome.desktop.session idle-delay)" == "uint32 600" ]]
[[ "$(image org.gnome.desktop.screensaver lock-enabled)" == true ]]
[[ "$(image org.gnome.desktop.privacy disable-camera)" == true ]]
[[ "$(image org.gnome.desktop.interface clock-format)" == "'24h'" ]]
[[ "$(image org.gnome.desktop.input-sources sources)" == "[('xkb', 'us'), ('xkb', 'fr+azerty')]" ]]
[[ "$(image org.gnome.login-screen disable-user-list)" == true ]]
[[ "$(image org.gnome.login-screen banner-message-enable)" == true ]]
[[ "$(image org.gnome.login-screen banner-message-text)" == "'Property of Acme.\\nAuthorised use only.'" ]]
[[ "$(image org.gnome.settings-daemon.plugins.power sleep-inactive-ac-timeout)" == 3600 ]]
extensions="$(image org.gnome.shell enabled-extensions)"
grep -qF "'${UI}'" <<<"${extensions}"
grep -qF "${CLIPBOARD}" <<<"${extensions}" && false
# ...the locked ones in dconf's database too, value and lock
[[ "$(locked /org/gnome/desktop/session/idle-delay)" == "uint32 600" ]]
[[ "$(locked /org/gnome/desktop/privacy/disable-camera)" == true ]]
grep -qx /org/gnome/desktop/session/idle-delay "${ROOT}/etc/dconf/db/distro.d/locks/zz4-image-policy"
grep -q clock-format "${ROOT}/etc/dconf/db/distro.d/locks/zz4-image-policy" && false
# Run again with nothing to set, the desktop's settings go
rm "${DIR}/desktop.json"
"${POLICY}" "${DIR}" --root "${ROOT}" >/dev/null
[[ ! -e "${ROOT}/usr/share/glib-2.0/schemas/zz4-image-policy.gschema.override" ]]
[[ ! -e "${ROOT}/etc/dconf/db/distro.d/locks/zz4-image-policy" ]]
[[ "$(image org.gnome.desktop.session idle-delay)" != "uint32 600" ]]

# The company's network: its proxy, a Flatpak remote with its key pinned, and a VPN with its CA and no secret
stand_in
mkdir -p "${ROOT}/usr/lib/NetworkManager" "${DIR}/flatpak-remotes" "${DIR}/vpn"
cp -r /usr/lib/NetworkManager/VPN "${ROOT}/usr/lib/NetworkManager/"
SERVICE="$(sed -n 's/^service=//p' "$(ls /usr/lib/NetworkManager/VPN/*.name | head -n1)")"
printf 'http=proxy.acme.example:3128\nhttps=[fd00::1]:3129\nignore=localhost,*.acme.example,10.0.0.0/8\nlock=yes\n' >"${DIR}/proxy"
export GNUPGHOME="${T}/gnupg"
mkdir -m 0700 "${GNUPGHOME}"
gpg --batch --passphrase '' --quick-generate-key "Acme apps <apps@acme.example>" ed25519 sign never 2>/dev/null
KEY="$(gpg --export apps@acme.example | base64 -w0)"
FPR="$(gpg --with-colons --fingerprint apps@acme.example | awk -F: '$1 == "fpr" { print $10; exit }')"
remote() { printf '[Flatpak Repo]\nTitle=Acme apps\nUrl=%s\nGPGKey=%s\n' "$1" "${KEY}" >"${DIR}/flatpak-remotes/acme.flatpakrepo"; }
remote https://flatpak.acme.example/repo/
echo "${FPR}" >"${DIR}/flatpak-remotes/acme.fingerprint"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout "${T}/ca.key" -out "${DIR}/vpn/acme-ca.crt" \
    -subj /CN=Acme -days 2 2>/dev/null
vpn() {
    cat >"${DIR}/vpn/acme.nmconnection" <<EOF
[connection]
id=Acme VPN
uuid=3c2b6f2e-2a0d-4d9e-9f4e-6a1f3f0f2c11
type=vpn
autoconnect=false

[vpn]
service-type=${SERVICE}
remote=vpn.acme.example
password-flags=2
ca=/etc/pki/amethystora-vpn/acme-ca.crt
$*

[ipv4]
method=auto
EOF
}
vpn
"${POLICY}" "${DIR}" --root "${ROOT}" >/dev/null
[[ "$(image org.gnome.system.proxy mode)" == "'manual'" ]]
[[ "$(image org.gnome.system.proxy.http host)|$(image org.gnome.system.proxy.http port)" == "'proxy.acme.example'|3128" ]]
[[ "$(image org.gnome.system.proxy.https host)" == "'[fd00::1]'" ]]
[[ "$(image org.gnome.system.proxy ignore-hosts)" == "['localhost', '*.acme.example', '10.0.0.0/8']" ]]
[[ "$(locked /system/proxy/http/host)" == "'proxy.acme.example'" ]]
grep -qx /system/proxy/mode "${ROOT}/etc/dconf/db/distro.d/locks/zz4-image-policy"
grep -qx "http_proxy=http://proxy.acme.example:3128" "${ROOT}/etc/environment.d/60-image-proxy.conf"
grep -qx "no_proxy=localhost,\*.acme.example,10.0.0.0/8" "${ROOT}/etc/environment.d/60-image-proxy.conf"
cmp -s "${DIR}/flatpak-remotes/acme.flatpakrepo" "${ROOT}/etc/flatpak/remotes.d/acme.flatpakrepo"
cmp -s "${DIR}/vpn/acme-ca.crt" "${ROOT}/etc/pki/amethystora-vpn/acme-ca.crt"
[[ "$(stat -c %a "${ROOT}/etc/NetworkManager/system-connections/acme.nmconnection")" == 600 ]]
# ...the remote alone before bake, which then takes its apps
rm -rf "${ROOT}/etc/flatpak/remotes.d"
"${POLICY}" "${DIR}" --root "${ROOT}" --only flatpak-remotes >/dev/null
[[ -f "${ROOT}/etc/flatpak/remotes.d/acme.flatpakrepo" ]]
cp "${DIR}/vpn/acme-ca.crt" "${T}/ca.crt"

refused() {
    local why="$1"
    if "${POLICY}" "${DIR}" --root "${ROOT}" >/dev/null 2>&1; then
        echo "amethystora-image-policy took ${why}"
        exit 1
    fi
    [[ ! -e "${ROOT}/usr/share/glib-2.0/schemas/zz4-image-policy.gschema.override" && ! -e "${ROOT}/etc/amethystora/agent.conf" &&
        ! -e "${ROOT}/etc/flatpak/remotes.d" && ! -e "${ROOT}/etc/NetworkManager/system-connections" &&
        ! -e "${ROOT}/etc/environment.d/60-image-proxy.conf" ]] ||
        { echo "amethystora-image-policy wrote for ${why}"; exit 1; }
    cmp -s /etc/amethystora/security.conf "${ROOT}/etc/amethystora/security.conf" ||
        { echo "amethystora-image-policy changed security.conf for ${why}"; exit 1; }
}
desktop() { stand_in; echo "ALLOW=know" >"${DIR}/agent.conf"; printf '%s\n' "$1" >"${DIR}/desktop.json"; refused "$2"; }
stand_in; echo "ALLOW=everything" >"${DIR}/agent.conf"; refused "an agent level that does not exist"
stand_in; echo "ALLOW_FOR=a b:act" >"${DIR}/agent.conf"; refused "an account name with a space"
stand_in; printf 'balanced\nDECOY=maybe\n' >"${DIR}/security"; refused "a security value a switch does not take"
stand_in; printf 'NO_SUCH_SWITCH=on\n' >"${DIR}/security"; refused "a security switch that does not exist"
stand_in; printf 'PROCESSES=on\nbalanced\n' >"${DIR}/security"; refused "a profile after a switch"
stand_in; echo "firewall-away on" >"${DIR}/hardening"; refused "a hardening switch a build cannot set"
stand_in; echo "lockout off" >"${DIR}/hardening"; refused "the image's own protection turned off"
stand_in; echo "block-webcam on" >"${DIR}/hardening"; refused "hardening outside a build"
stand_in; echo "automatic=off" >"${DIR}/updates"; refused "the updates outside a build"
stand_in; echo "x" >"${DIR}/build.sh"; refused "a script"
desktop '{"version": 1, "set": {"idle-delay": -1}}' "a negative delay"
desktop '{"version": 1, "set": {"idle-delay": 99999}}' "a delay out of range"
desktop '{"version": 1, "set": {"clock-format": "25h"}}' "a clock format that does not exist"
desktop '{"version": 1, "set": {"keyboard-layouts": ["us; rm -rf /"]}}' "a layout that is a command"
desktop '{"version": 1, "set": {"org.gnome.desktop.background picture-uri": "x"}}' "a setting not in the table"
desktop '{"version": 1, "set": {"login-banner": "a\u0007b"}}' "a banner with a control character"
desktop '{"version": 1, "lock": ["idle-delay"]}' "a lock without a value"
desktop '{"version": 1, "set": {"login-user-list-off": true}, "lock": ["login-user-list-off"]}' "a lock on the login screen"
desktop '{"version": 1, "extensions-off": ["amethystora-ui@arsslensoft.github.io"]}' "Amethystora UI turned off"
desktop '{"version": 1, "extensions-off": ["someone@example.com"]}' "an extension the image lacks"
desktop '{"version": 1, "command": "reboot"}' "a key desktop.json does not have"

# What the company's network may not hold, each beside an otherwise good remote and VPN
network() {
    stand_in
    mkdir -p "${ROOT}/usr/lib/NetworkManager" "${DIR}/flatpak-remotes" "${DIR}/vpn"
    cp -r /usr/lib/NetworkManager/VPN "${ROOT}/usr/lib/NetworkManager/"
    remote https://flatpak.acme.example/repo/
    echo "${FPR}" >"${DIR}/flatpak-remotes/acme.fingerprint"
    cp "${T}/ca.crt" "${DIR}/vpn/acme-ca.crt"
    vpn
}
network; "${POLICY}" "${DIR}" --root "${ROOT}" >/dev/null || { echo "the good remote and VPN were refused"; exit 1; }
network; echo "http=alice:secret@proxy.acme.example:3128" >"${DIR}/proxy"; refused "a proxy with a login"
network; echo "http=proxy.acme.example:99999" >"${DIR}/proxy"; refused "a proxy's port out of range"
network; printf 'http=proxy.acme.example:3128\nignore=localhost;reboot\n' >"${DIR}/proxy"; refused "a proxy's ignore list with a command"
network; echo "servers=10.0.0.53" >"${DIR}/dns"; refused "a DNS server without the name its certificate is for"
network; echo "servers=10.0.0.53#dns.acme.example" >"${DIR}/dns"; refused "DNS outside a build"
network; echo "0000000000000000000000000000000000000000" >"${DIR}/flatpak-remotes/acme.fingerprint"; refused "a remote whose key is not the one pinned"
network; rm "${DIR}/flatpak-remotes/acme.fingerprint"; refused "a remote without a fingerprint"
network; remote http://flatpak.acme.example/repo/; refused "a remote over http"
network; remote https://flatpak.acme.example/repo/; echo "Filter=/etc/x" >>"${DIR}/flatpak-remotes/acme.flatpakrepo"; refused "a remote with a key Flatpak would read a file for"
network; cp "${DIR}/flatpak-remotes/acme.flatpakrepo" "${DIR}/flatpak-remotes/flathub.flatpakrepo"; refused "a remote standing in for Flathub"
network; vpn "password=hunter2"; refused "a VPN with a password"
network; vpn "static-key=/etc/pki/amethystora-vpn/acme-ca.crt"; refused "a VPN with a key's file"
network; vpn "cert=/home/alice/alice.crt"; refused "a VPN naming a file outside its certificates"
network; printf '\n[vpn-secrets]\npassword=hunter2\n' >>"${DIR}/vpn/acme.nmconnection"; refused "a VPN with its secrets section"
network; cat "${T}/ca.key" >>"${DIR}/vpn/acme-ca.crt"; refused "a certificate with its private key"
network; sed -i "s/^service-type=.*/service-type=org.freedesktop.NetworkManager.nonexistent/" "${DIR}/vpn/acme.nmconnection"; refused "a VPN plugin the image lacks"
network; sed -i "s/^type=vpn/type=ethernet/" "${DIR}/vpn/acme.nmconnection"; refused "a connection that is no VPN"
network
cat >"${DIR}/vpn/wg.nmconnection" <<'EOF'
[connection]
id=Acme WireGuard
uuid=6f0f4f2a-9a5e-4d64-9b36-0d1f6e7a0b42
type=wireguard

[wireguard]
private-key=yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=
EOF
refused "WireGuard with its private key"
echo "amethystora-image-policy: ok"
