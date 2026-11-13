#!/usr/bin/bash
# The network switches on networks made here: a network namespace with dnsmasq (DHCPv4, DHCPv6, router
# advertisements, DNS for a search domain of its own) and a web server that answers every page with a login
# page, as a hotel's network does, and a second link standing for a VPN.
# - As the image ships: temporary IPv6 addresses, Wi-Fi's address per network name (Fedora's), and the
#   machine's name told to every network.
# - A different address on each network (ame security mac): a wired network sees an address of its own, the
#   same each time, another network another one, a default for one port in an earlier file does not win over
#   it (as Fedora's for Wi-Fi does not), no network is told the machine's name or its DHCPv6 identity, and
#   off gives everything back. Away from home only (`away`): the network named home keeps the port's own
#   address, the name and LLMNR lookups, any other network gets none of them, and a network no longer named
#   home, or off, gives the connection's defaults back.
# - Encrypted DNS (ame security dns): every name over DNS-over-TLS to the resolver chosen and to nobody else,
#   the network's own names and the connectivity check still on the network's servers, a VPN's settings kept
#   through every change.
# - A network that wants a login first: NetworkManager says so with encrypted DNS on, the session is offered
#   the pause in a notification that keeps its button on the screen, `ame security dns pause` lets the login
#   page load, and the pause ends by its timer. (Pressing the button cannot be done here: GNOME Shell's
#   buttons offer no AT-SPI action. Pressed with the lab's mouse, vm_mouse, it opens the command in a terminal.)
# - Firefox looks names up through the system's resolver as Fedora builds it, and with its own DNS-over-HTTPS
#   turned on asks its provider instead, without either breaking the other.
# vmlab-paths: @sf/usr/libexec/amethystora-dns* @sf/usr/libexec/amethystora-mac @sf/usr/share/amethystora/network/* @sf/usr/lib/systemd/user/amethystora-dns* @ui/modules/tools/reach*
source "$(dirname "$0")/lib.sh"

NS=vmlab-net
DIR=/run/vmlab-net
WAN=vmlab-wan
VPN=vmlab-vpn
LAN=vmlab-lan
LAN2=vmlab-lan-2
NAME=vmlab-net
UID_TEST="$(id -u "${TEST_USER}")"
HOME_DIR="$(realpath "$(getent passwd "${TEST_USER}" | cut -d: -f6)")"
RESOLVED_DROP_IN=/etc/systemd/resolved.conf.d/60-amethystora-dns.conf
NM_DNS_DROP_IN=/etc/NetworkManager/conf.d/60-amethystora-dns.conf
NM_MAC_DROP_IN=/etc/NetworkManager/conf.d/60-amethystora-mac.conf

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

# A terminal in the session, as Ptyxis opens one, for what asks for the password and a yes
session_terminal() {
    local steps=()
    while [[ $# -gt 0 && "$1" != -- ]]; do steps+=("$1"); shift; done
    shift
    in_terminal "${steps[@]}" -- runuser -u "${TEST_USER}" -- env XDG_RUNTIME_DIR="/run/user/${UID_TEST}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${UID_TEST}/bus" \
        systemd-run --user --pty --wait --quiet --collect -- /usr/bin/bash -lc "$*"
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
mac_of() { cat "/sys/class/net/$1/address"; }
# A unicast address the machine made up: the locally administered bit set, the group bit not
made_up() { (((16#${1:0:2} & 3) == 2)); }
# NetworkManager's verdict, after it checked again (the check itself can time out behind a login page, on the
# IPv6 side that page does not answer), as the offer reads it
connectivity() { nmcli -t networking connectivity check >/dev/null 2>&1; nmcli -t -f CONNECTIVITY general 2>/dev/null; }
up() { nmcli --wait 90 connection up "$1" >/dev/null 2>&1; }
has_temporary() { ip -6 addr show dev "$1" scope global | grep -q 'temporary'; }
# What the network was told, from dnsmasq's lease file (DHCPv4 lines, then a "duid" line and the DHCPv6 ones):
# the name for a hardware address, "*" for none, and for the IPv6 address this connection has now, the name and
# the DHCPv6 identity (DUID)
lease4_name() { awk -v mac="$1" '/^duid / {exit} $2 == mac {name = $4} END {print name}' "${DIR}/leases"; }
v6_address() { nmcli -g DHCP6.OPTION device show "${WAN}" 2>/dev/null | tr '|' '\n' | sed -n 's/^ *ip6_address = //p' | tr -d '\\ '; }
lease6() { local a; a="$(v6_address)"; [[ -n "${a}" ]] && awk -v a="${a}" -v f="$1" '/^duid / {v6 = 1; next} v6 && $3 == a {print $f}' "${DIR}/leases"; }
lease6_name() { lease6 4; }
lease6_duid() { lease6 5; }
leased() { [[ -n "$(lease4_name "$(mac_of "${WAN}")")" && -n "$(lease6_name)" ]]; }
queried() { grep -q "query\[[A-Z]*\] $1 from" "${DIR}/dnsmasq.log"; }
query() { timeout 40 resolvectl query --cache=no "$@" 2>&1; }
# The DNS and DNS-over-TLS packets on a link (tcpdump prints a plain query's name), from start to stop
capture_start() {
    tcpdump -nn -l -i "$1" 'port 53 or port 853' >"${DIR}/capture" 2>/dev/null &
    CAPTURE=$!
    sleep 2
}
capture_stop() {
    sleep 2
    kill -INT "${CAPTURE}" 2>/dev/null
    wait "${CAPTURE}" 2>/dev/null
    cat "${DIR}/capture"
}
report_json() { /usr/libexec/amethystora-security-status --json 2>/dev/null; }
record() { jq -c --arg id "$1" '[.. | objects | select(.id? == $id)][0]' <<<"$2"; }

# dnsmasq anew, with no lease, so that a lease shows what this connection told it. A copy of the program,
# for the policy's dnsmasq_t keeps the real one out of this folder
start_dnsmasq() {
    systemctl stop vmlab-dnsmasq.service 2>/dev/null
    : >"${DIR}/leases"
    : >"${DIR}/dnsmasq.log"
    systemd-run --quiet --unit=vmlab-dnsmasq --collect -p NetworkNamespacePath="/run/netns/${NS}" -- \
        "${DIR}/dnsmasq" --keep-in-foreground --user=root --group=root --conf-file=/dev/null --no-resolv --no-hosts \
        --dhcp-authoritative --bind-interfaces --interface=lan0 --interface=vpn0 --except-interface=lo \
        --log-queries --log-dhcp --log-facility="${DIR}/dnsmasq.log" --dhcp-leasefile="${DIR}/leases" \
        --dhcp-range=10.99.0.50,10.99.0.150,255.255.255.0,1h --dhcp-option=option:router,10.99.0.1 \
        --dhcp-option=option:dns-server,10.99.0.1 --domain=portal.lan \
        --dhcp-option=option:domain-search,portal.lan,example.org \
        --enable-ra --dhcp-range=fd99::100,fd99::1ff,slaac,64,1h --dhcp-option=option6:dns-server,[fd99::1] \
        --address=/printer.portal.lan/10.99.0.9 --address=/corp.test/10.98.0.7 --address=/#/10.99.0.1
    wait_for 20 grep -q started "${DIR}/dnsmasq.log"
}

make_networks() {
    mkdir -p "${DIR}"
    install -m 0755 /usr/sbin/dnsmasq "${DIR}/dnsmasq" && chcon -t bin_t "${DIR}/dnsmasq"
    cat >"${DIR}/portal.py" <<'EOF'
import http.server

class Portal(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with open("/run/vmlab-net/http.log", "a") as log:
            log.write(f"{self.headers.get('Host')} {self.path}\n")
        if self.path.startswith("/login"):
            body = b"<html><body>Log in to the lab's network</body></html>"
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
        else:
            body = b""
            self.send_response(302)
            self.send_header("Location", "http://10.99.0.1/login")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass

http.server.ThreadingHTTPServer(("10.99.0.1", 80), Portal).serve_forever()
EOF
    ip netns add "${NS}"
    ip link add "${WAN}" type veth peer name lan0 netns "${NS}"
    ip link add "${VPN}" type veth peer name vpn0 netns "${NS}"
    ip -n "${NS}" link set lo up
    ip -n "${NS}" addr add 10.99.0.1/24 dev lan0
    ip -n "${NS}" addr add fd99::1/64 dev lan0
    ip -n "${NS}" addr add 10.98.0.1/24 dev vpn0
    ip -n "${NS}" link set lan0 up
    ip -n "${NS}" link set vpn0 up
    # The VPN's end has an address of its own, as a VPN client gives its interface, and is not NetworkManager's
    ip addr add 10.98.0.2/24 dev "${VPN}"
    ip link set "${VPN}" up
    start_dnsmasq
    systemd-run --quiet --unit=vmlab-portal --collect -p NetworkNamespacePath="/run/netns/${NS}" -- python3 "${DIR}/portal.py"
    # NetworkManager leaves veth devices to whoever made them (85-nm-unmanaged.rules): this one is the network
    wait_for 30 nmcli -t device show "${WAN}"
    nmcli device set "${WAN}" managed yes
    # ...and connects only as the test says, not with a wired connection NetworkManager makes up for it
    nmcli device set "${WAN}" autoconnect no
    for profile in "${LAN}" "${LAN2}"; do
        nmcli connection add type ethernet ifname "${WAN}" con-name "${profile}" autoconnect no \
            ipv4.route-metric 2000 ipv6.route-metric 2000 >/dev/null
    done
}

cleanup() {
    nmcli connection delete "${LAN}" "${LAN2}" >/dev/null 2>&1
    systemctl stop vmlab-dnsmasq.service vmlab-portal.service 2>/dev/null
    ip netns delete "${NS}" 2>/dev/null
    rm -f /run/NetworkManager/conf.d/22-vmlab-early.conf
    nmcli general reload conf 2>/dev/null
    [[ -n "${MAIN:-}" ]] && nmcli device connect "${MAIN}" >/dev/null 2>&1
    [[ -e "${DIR}/hostname" ]] && hostnamectl set-hostname --static "$(cat "${DIR}/hostname")"
    rm -rf "${DIR}" "${HOME_DIR}/vmlab-ff-system" "${HOME_DIR}/vmlab-ff-doh"
}

MAIN="$(nmcli -t -f DEVICE,TYPE,STATE device | awk -F: '$2 == "ethernet" && $3 == "connected" {print $1; exit}')"
MAIN_UUID="$(nmcli -t -f DEVICE,CON-UUID device | awk -F: -v d="${MAIN}" '$1 == d {print $2}')"
if [[ -z "${MAIN}" ]]; then
    not_ok "no wired connection to start from" "$(nmcli device 2>&1)"
    finish
fi
cleanup
mkdir -p "${DIR}"
hostnamectl --static >"${DIR}/hostname"
hostnamectl set-hostname --static "${NAME}"

# --- As the image ships --------------------------------------------------------------------------------------
same "encrypted DNS is off as the image ships" "$(/usr/libexec/amethystora-dns status)" off
same "a different address on each network is off as the image ships" "$(/usr/libexec/amethystora-mac status)" off
CONFIG="$(/usr/sbin/NetworkManager --print-config 2>/dev/null)"
check "NetworkManager's connectivity check is on (Fedora's)" grep -qx 'enabled=true' <<<"$(sed -n '/^\[connectivity\]/,/^\[/p' <<<"${CONFIG}")"
check "Wi-Fi gets an address of its own for each network name as the image ships (Fedora's stable-ssid)" \
    grep -qx 'wifi.cloned-mac-address=stable-ssid' <<<"${CONFIG}"
same "temporary IPv6 addresses as the image ships (ame security ipv6-privacy), on ${MAIN}" \
    "$(cat "/proc/sys/net/ipv6/conf/${MAIN}/use_tempaddr")" 2
same "the machine reaches the internet" "$(connectivity)" full

make_networks
MAC0="$(mac_of "${WAN}")"
if up "${LAN}" && wait_for 60 leased; then
    ok "a network made here: DHCPv4, DHCPv6 and router advertisements from dnsmasq"
else
    not_ok "the network made here did not connect" "$(nmcli device show "${WAN}" 2>&1 | head -n 20)" "$(tail -n 20 "${DIR}/dnsmasq.log")"
    cleanup
    finish
fi
same "without the switch, a wired network sees the port's own address" "$(mac_of "${WAN}")" "${MAC0}"
same "without the switch, the network is told the machine's name (DHCPv4)" "$(lease4_name "${MAC0}")" "${NAME}"
echo "# without the switch, DHCPv6 tells the network the name: $(lease6_name), as DUID $(lease6_duid)"
check "a temporary IPv6 address on the network made here, as the image ships" wait_for 30 has_temporary "${WAN}"
DUID0="$(lease6_duid)"
start_dnsmasq
up "${LAN2}" && wait_for 60 leased
DUID0_2="$(lease6_duid)"
echo "# without the switch, the DHCPv6 identity on a second network: ${DUID0_2} ($([[ "${DUID0_2}" == "${DUID0}" ]] && echo the same || echo another))"

# --- A different address on each network (ame security mac) ---------------------------------------------------
out="$(session_terminal 120 'Use a different address on each network\?' y '[Pp]assword' "${TEST_USER}"$'\r' -- "ame security mac on")"
status=$?
same "ame security mac turns it on, after a yes and the password (exit status, then the switch)" \
    "${status} $(/usr/libexec/amethystora-mac status)" "0 on"
[[ "${status}" == 0 ]] || not_ok "ame security mac said" "$(tail -n 15 <<<"${out}")"
start_dnsmasq
up "${LAN}" && wait_for 60 leased
MAC1="$(mac_of "${WAN}")"
if [[ "${MAC1}" != "${MAC0}" ]] && made_up "${MAC1}"; then
    ok "the wired network sees an address of its own, made up for it (${MAC1}, the port's is ${MAC0})"
else
    not_ok "the wired network still sees ${MAC1}" "the port's: ${MAC0}"
fi
same "and is not told the machine's name (DHCPv4)" "$(lease4_name "${MAC1}")" "*"
same "nor over DHCPv6" "$(lease6_name)" "*"
DUID1="$(lease6_duid)"
up "${LAN}" && wait_for 60 leased
same "the same address each time it connects to that network" "$(mac_of "${WAN}")" "${MAC1}"
start_dnsmasq
up "${LAN2}" && wait_for 60 leased
MAC2="$(mac_of "${WAN}")"
DUID2="$(lease6_duid)"
if [[ "${MAC2}" != "${MAC1}" && "${MAC2}" != "${MAC0}" ]] && made_up "${MAC2}"; then
    ok "another network on the same port sees another address (${MAC2})"
else
    not_ok "another network sees ${MAC2}" "the first: ${MAC1}, the port's: ${MAC0}"
fi
if [[ -n "${DUID1}" && "${DUID2}" != "${DUID1}" && "${DUID1}" != "${DUID0}" ]]; then
    ok "nor can DHCPv6 tell them apart: each network gets an identity (DUID) of its own"
else
    not_ok "DHCPv6 gives the networks one identity" "first: ${DUID1}" "second: ${DUID2}" "without the switch: ${DUID0}"
fi
check "a temporary IPv6 address with it on" wait_for 30 has_temporary "${WAN}"
# A default for this one port in a file read before the switch's, as Fedora's 22-wifi-mac-addr.conf is for
# Wi-Fi: the switch's default still applies
mkdir -p /run/NetworkManager/conf.d
printf '[connection.22-vmlab-early]\nmatch-device=interface-name:%s\nethernet.cloned-mac-address=permanent\n' "${WAN}" \
    >/run/NetworkManager/conf.d/22-vmlab-early.conf
nmcli general reload conf
up "${LAN}"
same "a default for one port in an earlier file, as Fedora's for Wi-Fi is, does not win over the switch" "$(mac_of "${WAN}")" "${MAC1}"
rm -f /run/NetworkManager/conf.d/22-vmlab-early.conf
nmcli general reload conf
up "${MAIN_UUID}"
permanent="$(ip -j link show "${MAIN}" | jq -r '.[0].permaddr // empty')"
if [[ -n "${permanent}" && "$(mac_of "${MAIN}")" != "${permanent}" ]]; then
    ok "the machine's own wired connection sees another address too ($(mac_of "${MAIN}"), its own ${permanent})"
else
    not_ok "${MAIN} still sees its own address" "$(ip link show "${MAIN}")"
fi
same "and still reaches the internet" "$(connectivity)" full
out="$(session_terminal 120 '[Pp]assword' "${TEST_USER}"$'\r' -- "ame security mac off")"
same "ame security mac off turns it off" "$? $(/usr/libexec/amethystora-mac status)" "0 off"
start_dnsmasq
up "${LAN}" && wait_for 60 leased
same "off, the network sees the port's own address again" "$(mac_of "${WAN}")" "${MAC0}"
same "and is told the machine's name again" "$(lease4_name "${MAC0}")" "${NAME}"
up "${MAIN_UUID}"
same "${MAIN} has its own address again" "$(ip -j link show "${MAIN}" | jq -r '.[0].permaddr // "its own"')" "its own"

# --- The same, except at home (ame security mac away, ame security home-networks) -----------------------------
# systemd-resolved asks the link for a single-label name by LLMNR (UDP 5355) unless the connection says no; the
# network made here answers every DNS name, so a lookup shows whether the question went out as well
llmnr_asked() {
    tcpdump -nn -l -i "${WAN}" 'udp port 5355' >"${DIR}/llmnr" 2>/dev/null &
    local pid=$!
    sleep 2
    query vmlab-single >/dev/null
    sleep 2
    kill -INT "${pid}" 2>/dev/null
    wait "${pid}" 2>/dev/null
    # A packet line names the port; round 7 found the file non-empty with nothing in it that cat showed
    grep -q '\.5355' "${DIR}/llmnr"
}
LAN_UUID="$(nmcli -g connection.uuid connection show "${LAN}")"
prop_of() { nmcli -g "$2" connection show "$1" 2>/dev/null; }
default_props() {
    [[ -z "$(prop_of "$1" 802-3-ethernet.cloned-mac-address)" && "$(prop_of "$1" ipv4.dhcp-send-hostname)" == -1 &&
        "$(prop_of "$1" ipv6.dhcp-send-hostname)" == -1 && "$(prop_of "$1" connection.llmnr)" == -1 ]]
}
up "${LAN}" && wait_for 60 leased
if llmnr_asked; then
    ok "off, a single-label name is looked up over LLMNR on the network as well (the control for the next check)"
    LLMNR_CONTROL=true
else
    skip "LLMNR lookups do not go out on this link even off, so the next check shows nothing"
    LLMNR_CONTROL=false
fi
named="$(/usr/libexec/amethystora-hardening home "${LAN_UUID}" 2>&1)" || not_ok "naming ${LAN} home" "${named}"
out="$(session_terminal 120 'except at home\?' y '[Pp]assword' "${TEST_USER}"$'\r' -- "ame security mac away")"
status=$?
same "ame security mac away turns it on, after a yes and the password (exit status, then the switch)" \
    "${status} $(/usr/libexec/amethystora-mac status)" "0 away"
[[ "${status}" == 0 ]] || not_ok "ame security mac away said" "$(tail -n 15 <<<"${out}")"
same "the report calls it on" "$(report_json | jq -r '[.checks[] | select(.id == "network-address")][0].state')" on
same "the network named home keeps the port's own address setting (permanent)" \
    "$(prop_of "${LAN_UUID}" 802-3-ethernet.cloned-mac-address)" permanent
start_dnsmasq
up "${LAN}" && wait_for 60 leased
same "the network named home sees the port's own address" "$(mac_of "${WAN}")" "${MAC0}"
same "and is told the machine's name" "$(lease4_name "${MAC0}")" "${NAME}"
start_dnsmasq
up "${LAN2}" && wait_for 60 leased
MAC3="$(mac_of "${WAN}")"
if [[ "${MAC3}" != "${MAC0}" ]] && made_up "${MAC3}"; then
    ok "a network not named home sees an address made up for it (${MAC3})"
else
    not_ok "the network not named home sees ${MAC3}" "the port's: ${MAC0}"
fi
same "and is not told the machine's name (DHCPv4)" "$(lease4_name "${MAC3}")" "*"
same "nor over DHCPv6" "$(lease6_name)" "*"
if [[ "${LLMNR_CONTROL}" == true ]]; then
    # The setting NetworkManager gave resolved for the link is what the connection's default asks for; the packets
    # are what resolved did with it
    same "resolved's LLMNR setting on a network not named home is off" "$(resolvectl llmnr "${WAN}" 2>&1 | sed 's/.*: //')" no
    if llmnr_asked; then
        not_ok "a single-label name was looked up over LLMNR on a network not named home" "$(od -c "${DIR}/llmnr" | head -n 5)" \
            "$(resolvectl status "${WAN}" 2>&1)" "$(nmcli -g connection.llmnr connection show "${LAN2}")" \
            "$(/usr/sbin/NetworkManager --print-config 2>&1 | grep -i llmnr)"
    else
        ok "no LLMNR lookup goes out on a network not named home"
    fi
fi
start_dnsmasq
up "${LAN}" && wait_for 60 leased
if [[ "${LLMNR_CONTROL}" == true ]]; then
    if llmnr_asked; then ok "the network named home keeps LLMNR lookups"; else not_ok "no LLMNR lookup on the network named home"; fi
fi
/usr/libexec/amethystora-hardening home
check "naming no network home gives the connection back its defaults" default_props "${LAN_UUID}"
start_dnsmasq
up "${LAN}" && wait_for 60 leased
if [[ "$(mac_of "${WAN}")" != "${MAC0}" ]] && [[ -z "$(lease4_name "$(mac_of "${WAN}")")" || "$(lease4_name "$(mac_of "${WAN}")")" == "*" ]]; then
    ok "and away it is another network like the rest: another address, no name"
else
    not_ok "the network no longer named home still sees $(mac_of "${WAN}")" "$(lease4_name "$(mac_of "${WAN}")")"
fi
/usr/libexec/amethystora-hardening home "${LAN_UUID}"
/usr/libexec/amethystora-mac off
check "off, the network that was named home has its defaults back" default_props "${LAN_UUID}"
/usr/libexec/amethystora-hardening home
up "${MAIN_UUID}"

# --- Encrypted DNS ---------------------------------------------------------------------------------------------
# A VPN's DNS, set on its own link over D-Bus as Tailscale sets it: its domain, its server, no DNS-over-TLS
resolvectl dns "${VPN}" 10.98.0.1
resolvectl domain "${VPN}" '~corp.test'
resolvectl dnsovertls "${VPN}" no
vpn_kept() { [[ "$(resolvectl dns "${VPN}")" == *10.98.0.1* && "$(resolvectl domain "${VPN}")" == *'~corp.test'* ]]; }
out="$(session_terminal 120 'Send every lookup to Quad9\?' y '[Pp]assword' "${TEST_USER}"$'\r' -- "ame security dns quad9")"
same "ame security dns quad9 turns it on, after a yes and the password (exit status, then the resolver)" \
    "$? $(/usr/libexec/amethystora-dns status) $(/usr/libexec/amethystora-dns name)" "0 on Quad9"
check "resolved's drop-in: Quad9's servers, DNS-over-TLS, every name" grep -qzP \
    'DNS=9\.9\.9\.9#dns\.quad9\.net[^\n]*\nDNSOverTLS=yes\nDomains=~\.' "${RESOLVED_DROP_IN}"
check "NetworkManager's drop-in: each network's own servers in plain text" grep -qx 'connection.dns-over-tls=0' "${NM_DNS_DROP_IN}"
# The network's lease names two search domains: its own (portal.lan, under a name only a network has) stays, and one it
# could take public names back with (example.org) is taken off by amethystora-dns-domains.service
domains_taken() { local d; d="$(resolvectl domain "${WAN}")"; [[ "${d}" == *portal.lan* && "${d}" != *example.org* ]]; }
check "a search domain the network hands out is taken off, the network's own name kept" wait_for 20 domains_taken
same "DNS-over-TLS everywhere but on the networks' own links" \
    "$(resolvectl dnsovertls | sed -n "s/^Global: //p;s/^Link [0-9]* (${MAIN}): //p;s/^Link [0-9]* (${WAN}): //p" | paste -sd ' ')" "yes no no"
# What crosses the machine's own link while a name is looked up (resolvectl names the link an answer came in on,
# encrypted or not)
name=www.wikipedia.org
capture_start "${MAIN}"
out="$(query "${name}")"
seen="$(capture_stop)"
if [[ "${out}" == *"encrypted transport: yes"* ]] && grep -qE '\.853[: ]' <<<"${seen}" && ! grep -qF "${name}" <<<"${seen}"; then
    ok "a name goes to Quad9 over DNS-over-TLS, and to no network's servers in plain text"
else
    not_ok "${name} was not looked up over DNS-over-TLS alone" "${out}" "on ${MAIN}:" "$(head -n 20 <<<"${seen}")"
fi
: >"${DIR}/dnsmasq.log"
query www.example.org >/dev/null
if queried www.example.org; then not_ok "the network made here was asked for www.example.org" "$(cat "${DIR}/dnsmasq.log")"; else ok "the network was not asked for it"; fi
same "the connectivity check still finds the internet (it asks each network's own servers)" "$(connectivity)" full
out="$(query printer.portal.lan)"
if [[ "${out}" == *10.99.0.9* && "${out}" == *"link: ${WAN}"* ]] && queried printer.portal.lan; then
    ok "a name on the network's own domain (its printer) is asked of the network's own servers"
else
    not_ok "printer.portal.lan did not go to the network made here" "${out}"
fi
out="$(query git.corp.test)"
if [[ "${out}" == *10.98.0.7* && "${out}" == *"link: ${VPN}"* ]]; then ok "a VPN's names still go to its own server"; else not_ok "git.corp.test did not go to the VPN's server" "${out}"; fi
if vpn_kept; then ok "turning it on kept the VPN's settings (resolved reloaded, not restarted)"; else not_ok "the VPN's settings are gone" "$(resolvectl status "${VPN}")"; fi
check "the offer of a pause runs in the session" user_systemctl is-active --quiet amethystora-dns-portal.service
record="$(record encrypted-dns "$(report_json)")"
if jq -e '.state == "on" and (.text | test("Quad9"))' <<<"${record}" >/dev/null; then ok "the security report says it is on, with Quad9"; else not_ok "the report's encrypted-dns" "${record}"; fi
/usr/libexec/amethystora-dns on cloudflare
out="$(query www.fedoraproject.org)"
if [[ "$(/usr/libexec/amethystora-dns name)" == Cloudflare && "${out}" == *"encrypted transport: yes"* ]] && vpn_kept; then
    ok "another resolver chosen (Cloudflare) answers, and the VPN keeps its settings again"
else
    not_ok "switching to Cloudflare" "$(/usr/libexec/amethystora-dns name)" "${out}" "$(resolvectl status "${VPN}")"
fi

# --- Firefox ---------------------------------------------------------------------------------------------------
check "Fedora builds Firefox with its own DNS-over-HTTPS off (network.trr.mode 5)" grep -qE \
    '^pref\("network\.trr\.mode", *5\);' /usr/lib64/firefox/browser/defaults/preferences/firefox-redhat-default-prefs.js
# What Firefox asks resolved, from resolved's own log at debug level, which names each name it looks up
firefox_shot() {
    local profile="${HOME_DIR}/$1"
    rm -f "${profile}/shot.png"
    as_user "timeout 180 firefox --headless --no-remote --profile ${profile} --screenshot ${profile}/shot.png $2" >/dev/null 2>&1
    [[ -s "${profile}/shot.png" ]]
}
looked_up() { journalctl -u systemd-resolved --since "@$1" --no-pager -o cat | grep -F "$2"; }
as_user "mkdir -p ~/vmlab-ff-system ~/vmlab-ff-doh"
resolvectl log-level debug
since="$(date +%s)"
if firefox_shot vmlab-ff-system https://www.iana.org/ && looked_up "${since}" www.iana.org >/dev/null; then
    ok "Firefox loads a page with its name looked up through the system's resolver, encrypted DNS on"
else
    not_ok "Firefox, as Fedora builds it" "$(looked_up "${since}" iana | head -n 5)"
fi
# Its own DNS-over-HTTPS turned on in a profile of the test's, as Settings, Privacy & Security does
as_user "printf '%s\n' 'user_pref(\"network.trr.mode\", 3);' 'user_pref(\"network.trr.uri\", \"https://mozilla.cloudflare-dns.com/dns-query\");' >~/vmlab-ff-doh/user.js"
since="$(date +%s)"
if firefox_shot vmlab-ff-doh https://www.rfc-editor.org/ && ! looked_up "${since}" www.rfc-editor.org >/dev/null &&
    looked_up "${since}" mozilla.cloudflare-dns.com >/dev/null; then
    ok "with its own DNS-over-HTTPS on, Firefox asks its provider instead (found through the system's resolver), and the page still loads"
else
    not_ok "Firefox with DNS-over-HTTPS" "$(looked_up "${since}" rfc-editor | head -n 3)" "$(looked_up "${since}" cloudflare-dns | head -n 3)" \
        "screenshot: $(ls -la "${HOME_DIR}/vmlab-ff-doh/shot.png" 2>&1)"
fi
resolvectl log-level info

# --- A network that wants a login first --------------------------------------------------------------------------
offered() { pgrep -u "${TEST_USER}" -f 'notify-send.*log in first' >/dev/null; }
nmcli device disconnect "${MAIN}" >/dev/null
: >"${DIR}/dnsmasq.log"
behind_portal() { [[ "$(connectivity)" == portal ]]; }
if wait_for 120 behind_portal; then
    ok "behind the login page, with encrypted DNS on, NetworkManager finds a network that wants a login (portal)"
else
    not_ok "NetworkManager says $(connectivity) behind the login page" "$(resolvectl dnsovertls)" "$(tail -n 5 "${DIR}/http.log" 2>&1)"
fi
if wait_for 90 offered; then ok "the session is offered the pause, in a notification"; else not_ok "no notification offers the pause" \
    "$(journalctl -b --user-unit=amethystora-dns-portal.service --no-pager -o cat 2>&1 | tail -n 10)"; fi
# A critical one, which GNOME Shell keeps on the screen with its button until answered
check "it stays on the screen with its button (critical), Pause for ten minutes…" \
    pgrep -u "${TEST_USER}" -f 'notify-send .*--urgency=critical .*--action=pause=Pause for ten minutes'
skip "pressing the button: GNOME Shell's buttons offer no AT-SPI action; pressed with the lab's mouse (vm_mouse), it opens" \
    "ame security dns pause in a terminal, which asks for the password, as typed below"
pkill -u "${TEST_USER}" -f 'notify-send .*log in first'
out="$(query www.example.net)"
if [[ "${out}" != *10.99.0.1* ]] && ! queried www.example.net; then
    ok "until then, a name is neither seen nor answered by the network"
else
    not_ok "the network answered www.example.net" "${out}"
fi
out="$(session_terminal 120 '[Pp]assword' "${TEST_USER}"$'\r' -- "ame security dns pause")"
status=$?
if [[ "${status}" == 0 ]] && /usr/libexec/amethystora-dns paused; then ok "ame security dns pause pauses it, after the password"; else
    not_ok "ame security dns pause" "exit status ${status}" "$(tail -n 10 <<<"${out}")"; fi
out="$(query www.example.net)"
if [[ "${out}" == *10.99.0.1* ]] && queried www.example.net; then ok "paused, the network's own servers answer"; else not_ok "paused, www.example.net" "${out}"; fi
check "and its login page loads" sh -c 'curl -sL --max-time 20 http://www.example.net/ | grep -q "Log in"'
next="$(systemctl show amethystora-dns-resume.timer -P NextElapseUSecMonotonic)"
usec="$(systemd-analyze timespan "${next}" 2>/dev/null | sed -n 's/.*s: \([0-9][0-9]*\)$/\1/p' | head -n 1)"
left=$((${usec:-0} / 1000000 - $(cut -d. -f1 /proc/uptime)))
if ((left > 540 && left <= 600)); then ok "the pause ends by itself in ten minutes (${left} s left)"; else not_ok "the pause's timer" "${left} s left" \
    "$(systemctl status amethystora-dns-resume.timer --no-pager 2>&1 | head -n 6)"; fi
record="$(record encrypted-dns-paused "$(report_json)")"
if jq -e '.state == "check"' <<<"${record}" >/dev/null; then ok "the security report says it is paused"; else not_ok "the report's encrypted-dns-paused" "${record}"; fi
# What the timer starts when its ten minutes are up
systemctl start amethystora-dns-resume.service
out="$(query www.example.net)"
if ! /usr/libexec/amethystora-dns paused && [[ "${out}" != *10.99.0.1* ]]; then ok "at the end of the pause, names go to the resolver again"; else
    not_ok "after the pause" "$(ls /run/systemd/resolved.conf.d/ 2>&1)" "${out}"; fi
# Off the network with the login page, whose server answers any name first, and back on the machine's own
nmcli device disconnect "${WAN}" >/dev/null
nmcli device connect "${MAIN}" >/dev/null
same "on its own network again, the machine reaches the internet" "$(connectivity)" full

# --- Off ------------------------------------------------------------------------------------------------------
out="$(session_terminal 120 '[Pp]assword' "${TEST_USER}"$'\r' -- "ame security dns off")"
same "ame security dns off turns it off" "$? $(/usr/libexec/amethystora-dns status)" "0 off"
if [[ ! -e "${RESOLVED_DROP_IN}" && ! -e "${NM_DNS_DROP_IN}" && ! -e "${NM_MAC_DROP_IN}" ]]; then ok "and leaves none of its files"; else
    not_ok "files left" "$(ls "${RESOLVED_DROP_IN}" "${NM_DNS_DROP_IN}" "${NM_MAC_DROP_IN}" 2>&1)"; fi
same "resolved has no DNS-over-TLS left" "$(resolvectl dnsovertls | sed -n 's/^Global: //p')" no
name=www.wikipedia.org
capture_start "${MAIN}"
out="$(query "${name}")"
seen="$(capture_stop)"
# ...with nothing sent over DNS-over-TLS: what the closing of its last connection carries has no data
if grep -qF "${name}" <<<"${seen}" && ! grep -qE '> [0-9a-f.:]+\.853: .*length [1-9]' <<<"${seen}"; then ok "names go to the network's own servers again"; else
    not_ok "after off, ${name}" "${out}" "on ${MAIN}:" "$(head -n 20 <<<"${seen}")"; fi
same "the saved connection was left as it was" "$(nmcli -g connection.dns-over-tls connection show "${MAIN_UUID}")" -1
if vpn_kept; then ok "and the VPN keeps its settings"; else not_ok "the VPN's settings are gone" "$(resolvectl status "${VPN}")"; fi
offer_stopped() { ! user_systemctl is-active --quiet amethystora-dns-portal.service; }
check "the offer of a pause stops with it" wait_for 30 offer_stopped
same "the machine reaches the internet" "$(connectivity)" full

cleanup
finish
