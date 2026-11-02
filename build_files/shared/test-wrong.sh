#!/usr/bin/bash
# Something's wrong (R7-36): /usr/libexec/amethystora-wrong against stand-ins for what it reads, for 20-tests.sh. Each topic
# says what it found and names one thing to press: a fix is always an `ame` command and never a security one, anything to do
# with security only opens Security, what it read is never written, and what is quoted is clean text.
#   bash test-wrong.sh [HELPER]

set -uo pipefail

HELPER="${1:-/usr/libexec/amethystora-wrong}"
ROOT="$(mktemp -d)"
trap 'rm -rf "${ROOT}"' EXIT
export AMETHYSTORA_WRONG_ROOT="${ROOT}"
export HOME="${ROOT}/home"
mkdir -p "${HOME}" "${ROOT}/bin" "${ROOT}/says" "${ROOT}/usr/libexec" "${ROOT}/etc/amethystora" "${ROOT}/proc/pressure" \
    "${ROOT}/proc/asound" "${ROOT}/var/lib/amethystora/drivers"
failed=0
fail() {
    echo "test-wrong: $*" >&2
    failed=1
}

# One stand-in under every name: what it prints is the file of its name and arguments in says/, or of its name alone; with
# neither it fails, as a program that is not there does
cat >"${ROOT}/bin/stand-in" <<'EOF'
#!/usr/bin/bash
name="$(basename "$0")"
echo "${name} $*" >>"${CALLS}"
key="$(printf '%s_' "${name}" "$@" | tr -c 'A-Za-z0-9_.\n-' '_')"
key="${key%_}"
for said in "${SAYS}/${key}" "${SAYS}/${name}_${*: -1}" "${SAYS}/${name}"; do
    if [[ -e "${said}" ]]; then
        cat "${said}"
        exit 0
    fi
done
exit 1
EOF
chmod +x "${ROOT}/bin/stand-in"
for name in wpctl systemctl nmcli ip ping resolvectl busctl lpstat df powerprofilesctl; do
    ln -s stand-in "${ROOT}/bin/${name}"
done
for name in amethystora-dns amethystora-mac amethystora-security-config amethystora-doctor; do
    ln -s "${ROOT}/bin/stand-in" "${ROOT}/usr/libexec/${name}"
done
export SAYS="${ROOT}/says" CALLS="${ROOT}/calls"
: >"${CALLS}"
export PATH="${ROOT}/bin:${PATH}"

says() { printf '%s\n' "$2" >"${SAYS}/$(printf '%s' "$1" | tr -c 'A-Za-z0-9_.\n-' '_')"; }
clear_says() {
    rm -f "${SAYS}"/*
    rm -f "${ROOT}/etc/amethystora/hardening-on" "${ROOT}/var/lib/amethystora/drivers/status.json"
    : >"${CALLS}"
}
look() { "${HELPER}" look "$1" --json; }
# TOPIC JQ-TEST WHY: the answer holds to the test
holds() {
    local answer
    answer="$(look "$1")"
    jq -e "$2" <<<"${answer}" >/dev/null || fail "$3: ${answer}"
    # Whatever it found: at most one thing to press, an ame command that is not a security one, and one that can interrupt
    # something (every connection, every sound) opens in a terminal
    jq -e '(.fix == null or .security == null)
        and (.fix == null or ((.fix.command | join(" ")) | test("network reconnect|sound restart") | not) or .fix.terminal == true)
        and (.state != "fine" or .fix == null or (.fix.command | join(" ") | test("network reconnect|sound restart") | not))
        and (.fix == null or (.fix.command[0] == "ame" and .fix.command[1] != "security" and ([.fix.command[] | test("^[A-Za-z0-9_.:-]+$")] | all)))
        and (.security == null or (.security.page | test("^(events|settings(/[a-z]+)?)$")))' <<<"${answer}" >/dev/null ||
        fail "$3: an answer that breaks the rules: ${answer}"
}
sound_ok() {
    says systemctl_--user_is-active_wireplumber.service_pipewire.service $'active\nactive'
    says wpctl_get-volume_@DEFAULT_AUDIO_SINK@ 'Volume: 0.40'
    says wpctl_inspect_@DEFAULT_AUDIO_SINK@ 'id 52'
    says wpctl_status 'PipeWire
Audio
 ├─ Devices:
 │      49. Built-in Audio                      [alsa]
 │
 ├─ Sinks:
 │  *   52. Built-in Audio Analog Stereo        [vol: 0.40]
 │
 ├─ Sources:
 │      53. Built-in Audio Analog Stereo        [vol: 1.00]
 │

Video
 ├─ Sinks:
 │      99. Not a sound output'
}

# --- The list -----------------------------------------------------------------------------------------------------------
[[ "$("${HELPER}" topics --json | jq -r 'map(.id) | join(" ")')" == "sound internet printer slow space device other" ]] || fail "the topics are not the seven"
"${HELPER}" look nonsense --json >/dev/null 2>&1 && fail "a topic it does not know was taken"
[[ "$("${HELPER}" 2>/dev/null; echo $?)" == 64 ]] || fail "no arguments did not exit 64"

# --- No sound -----------------------------------------------------------------------------------------------------------
clear_says
says systemctl_--user_is-active_wireplumber.service_pipewire.service $'inactive\nactive'
holds sound '.state == "found" and .fix.command == ["ame","system","sound","restart"] and .fix.terminal == true' "the sound programs stopped"
clear_says; sound_ok
says wpctl_get-volume_@DEFAULT_AUDIO_SINK@ 'Volume: 0.40 [MUTED]'
holds sound '.fix.command == ["ame","system","sound","unmute"] and (.fix.label | contains("half") | not)' "muted"
clear_says; sound_ok
says wpctl_get-volume_@DEFAULT_AUDIO_SINK@ 'Volume: 0.00 [MUTED]'
holds sound '.fix.command == ["ame","system","sound","unmute"] and (.fix.label | contains("half")) and (.words | contains("zero"))' "muted and at zero says that it also turns the volume up"
clear_says; sound_ok
says wpctl_get-volume_@DEFAULT_AUDIO_SINK@ 'Volume: 0.00'
holds sound '.fix.command == ["ame","system","sound","unmute"] and (.fix.label | contains("half"))' "at zero"
clear_says; sound_ok
says wpctl_status 'Audio
 ├─ Sinks:
 │      52. Built-in Audio Analog Stereo        [vol: 0.40]
 │  *   61. HDMI / CANARY-TV   [vol: 1.00]
 │
 ├─ Sources:'
holds sound '.fix.command == ["ame","system","sound","use","52"] and (.words | contains("HDMI / CANARY-TV"))' "the wrong output"
clear_says; sound_ok
says wpctl_status 'Audio
 ├─ Sinks:
 │  *   52. Dummy Output   [vol: 1.00]
 │
 ├─ Sources:'
echo block-bluetooth >"${ROOT}/etc/amethystora/hardening-on"
holds sound '.security.page == "settings/devices" and .fix == null' "Bluetooth blocked goes to Security, and is not fixed here"
clear_says; sound_ok
holds sound '.state == "fine" and .fix == null' "sound that is fine offers nothing that would stop what plays"

# --- No internet --------------------------------------------------------------------------------------------------------
net() { # general-status [names] [check: on (default) | off | unreadable]
    clear_says
    says nmcli_-t_-f_STATE_CONNECTIVITY_WIFI_general_status "$1"
    says nmcli_networking enabled
    says ip_-4_route_show_default 'default via 192.0.2.1 dev wlan0'
    says ping_-n_-c_1_-W_2_192.0.2.1 ok
    says amethystora-dns_status off
    says amethystora-mac_status off
    says amethystora-security-config_NETWORK off
    case "${3:-on}" in
        on)
            says busctl_ConnectivityCheckEnabled 'b true'
            says busctl_ConnectivityCheckUri 's "http://nmcheck.example.org/check_network_status.txt"' ;;
        off) says busctl_ConnectivityCheckEnabled 'b false' ;;
    esac
    [[ "${2:-}" == names ]] && says resolvectl_query_--legend_no_--cache_no_nmcheck.example.org 'address'
    return 0
}
called() { grep -q "$1" "${CALLS}"; }
net 'disconnected:none:disabled'
holds internet '.fix.command == ["ame","system","network","on"]' "Wi-Fi off"
net 'disconnected:none:enabled'
holds internet '.fix.command == ["ame","system","network","reconnect"] and .fix.terminal == true' "not connected"
net 'connecting:none:enabled'
holds internet '.state == "found" and .fix == null and (.words | contains("still connecting"))' "still connecting is not called not connected"
net 'connected (site only):portal:enabled'
holds internet '.state == "found" and .fix == null and .security == null' "a login page"
net 'connected:portal:enabled'; says amethystora-dns_status on
holds internet '.security.page == "settings/network"' "a login page behind encrypted DNS goes to Security"

# NetworkManager's own check decides first: when it says the internet works, nothing is asked of the network
net 'connected:full:enabled'; rm -f "${SAYS}/ping_-n_-c_1_-W_2_192.0.2.1"
holds internet '.state == "fine" and .fix == null and (.words | contains("NetworkManager says"))' "a router that drops pings is not blamed while the check says it works"
called '^ping\|^resolvectl' && fail "the network was asked something while NetworkManager's check said full: $(cat "${CALLS}")"
net 'connected:full:enabled' names
holds internet '.state == "fine" and .security == null' "the internet is fine"
net 'connected:full:enabled'; says amethystora-security-config_NETWORK block
holds internet '.state == "fine" and .security.page == "settings/network"' "network protection blocking goes to Security"

# No way out is a connection that has no default route at all; a route that goes out of an interface is a way out
net 'connected:limited:enabled'; rm -f "${SAYS}/ip_-4_route_show_default"
holds internet '.fix.command == ["ame","system","network","reconnect"] and .fix.terminal == true and (.words | contains("every connection"))' "no default route"
net 'connected:limited:enabled'; says ip_-4_route_show_default 'default dev wwan0 scope link'
holds internet '.fix == null and (.words | contains("a way out")) and (.words | contains("no way out") | not)' "a route with no gateway is a way out"
net 'connected:limited:enabled'; rm -f "${SAYS}/ip_-4_route_show_default"; says ip_-6_route_show_default 'default via fe80::1 dev wlan0 proto ra'
holds internet '.fix == null and (.words | contains("no way out") | not)' "an IPv6-only network has a way out"

# The check failed: what was seen is said as seen, and the one name asked is the check's own
net 'connected:limited:enabled'
holds internet '.state == "found" and .fix == null and .security == null and (.words | contains("may")) and (.words | contains("answered a test message")) and (.words | contains("cannot reach") | not)' "the check failed, the router answers"
called 'resolvectl query --legend=no --cache=no nmcheck.example.org' || fail "the lookup is not of the check's own name, from outside the cache: $(cat "${CALLS}")"
called 'amethystora.org' && fail "a lookup named the project's own address"
net 'connected:limited:enabled' names; rm -f "${SAYS}/ping_-n_-c_1_-W_2_192.0.2.1"
holds internet '.fix == null and (.words | contains("did not answer a test message")) and (.words | contains("could be found")) and (.words | contains("cannot reach") | not)' "a router that is silent is said to be silent, not blamed"
net 'connected:limited:enabled'; says amethystora-dns_status on
holds internet '.security.page == "settings/network" and .fix == null' "encrypted DNS that may be blocked goes to Security"
net 'connected:limited:enabled'; says amethystora-mac_status away
holds internet '.security.page == "settings/network"' "the changing address goes to Security"
net 'connected:limited:enabled' x off
holds internet '.fix == null' "the check is off and failed"
called '^resolvectl' && fail "a name was looked up while NetworkManager's check is off: $(cat "${CALLS}")"
net 'connected:unknown:enabled' x off; rm -f "${SAYS}/ping_-n_-c_1_-W_2_192.0.2.1"
holds internet '.state == "found" and .fix == null and (.words | contains("is off")) and (.words | contains("did not answer a test message"))' "the check is off: it says it cannot tell"
called '^resolvectl' && fail "a name was looked up while NetworkManager's check is off: $(cat "${CALLS}")"
net 'connected:unknown:enabled' x off
holds internet '.state == "fine" and (.words | contains("cannot tell"))' "the check is off and the router answers"
clear_says
holds internet '.fix.command == ["ame","system","network","restart"] and .fix.terminal == true' "no NetworkManager"

# --- The printer --------------------------------------------------------------------------------------------------------
clear_says
says lpstat_-p ''
holds printer '.fix.command == ["ame","system","printer","settings"]' "no printer"
echo firewall-away >"${ROOT}/etc/amethystora/hardening-on"
holds printer '.security.page == "settings/network" and (.words | contains("not named as home"))' "the away firewall goes to Security, and is said to hide printers only off the home networks"
clear_says
says lpstat_-p 'printer HP_Office disabled since Mon 02 Nov 2026 - paused'
says lpstat_-d 'system default destination: HP_Office'
holds printer '.fix.command == ["ame","system","printer","resume","HP_Office"] and .fix.terminal == true' "a paused printer"
clear_says
says lpstat_-p 'printer HP$(id) disabled since Mon'
holds printer '.fix == null and (.words | length > 0)' "a printer name a command could not take is not offered, and the card still says something"
clear_says
says lpstat_-p 'printer -x disabled since Mon'
holds printer '.fix == null and (.words | contains("Settings"))' "a printer name that starts with a dash is not offered, and the card says where to go"
clear_says
says lpstat_-p 'printer HP_Office is idle.  enabled since Mon'
says lpstat_-o 'HP_Office-1 me 100 Mon'
holds printer '.state == "found" and .fix == null and (.words | startswith("1 job is"))' "a job waiting"
says lpstat_-o $'HP_Office-1 me 100 Mon\nHP_Office-2 me 100 Mon'
holds printer '.words | startswith("2 jobs are")' "jobs waiting"
clear_says
says lpstat_-p 'printer HP_Office is idle.  enabled since Mon'
holds printer '.state == "fine"' "a printer that is fine"

# --- It feels slow, and room ----------------------------------------------------------------------------------------------
CLK="$(getconf CLK_TCK)"
PAGE="$(getconf PAGESIZE)"
# A program in /proc, and where it is a second later: the share of the processor it used in between, and of the memory it holds
proc() { # PID NAME CPU-PERCENT MEMORY-PERCENT
    local pages=$(($4 * 8000000 * 1024 / 100 / PAGE)) one two
    for one in proc proc.later; do
        mkdir -p "${ROOT}/${one}/$1"
        two=0
        [[ "${one}" == proc.later ]] && two=$(($3 * CLK / 100))
        printf '%s (%s) S 0 0 0 0 0 0 0 0 0 0 %s 0 0 0 0 0 0 0 0 0 %s\n' "$1" "$2" "${two}" "${pages}" >"${ROOT}/${one}/$1/stat"
    done
}
quiet() {
    clear_says
    rm -rf "${ROOT}/proc" "${ROOT}/proc.later"
    mkdir -p "${ROOT}/proc/pressure"
    says df_-h_--output_pcent_avail_/var/home $'Use% Avail\n 40% 100G'
    says systemctl_is-active_uupd.service_uupd-manual.service_amethystora-integrity.service_amethystora-clamav-scan.service_flatpak-preinstall.service $'inactive\ninactive\ninactive\ninactive\ninactive'
    says systemctl_--user_is-active_amethystora-backup.service_amethystora-pkg-upgrade.service $'inactive\ninactive'
    proc 4242 kitty 1 2
    printf 'some avg10=0.00 avg60=0.00 avg300=0.00 total=0\n' >"${ROOT}/proc/pressure/memory"
    printf 'MemTotal: 8000000 kB\nSwapTotal:  100 kB\nSwapFree:  100 kB\n' >"${ROOT}/proc/meminfo"
}
quiet; says df_-h_--output_pcent_avail_/var/home $'Use% Avail\n 96% 1G'
holds slow '.fix.command == ["ame","system","clean"] and .fix.terminal == true' "a full disk"
quiet; proc 4243 falco 90 2
holds slow '.security.page == "settings/detection"' "the behaviour sensor goes to Security"
quiet; says systemctl_--user_is-active_amethystora-backup.service_amethystora-pkg-upgrade.service $'active\ninactive'
holds slow '.state == "found" and .fix == null and (.words | contains("backup"))' "a backup running"
quiet; proc 4244 CANARY-PROGRAM 150 2
holds slow '.state == "found" and (.words | contains("CANARY-PROGRAM"))' "a busy program"
# What it used over its life is not what it uses now: a program with a long past and no use in the last second is not busy
quiet; mkdir -p "${ROOT}/proc/4245" "${ROOT}/proc.later/4245"
for one in proc proc.later; do
    printf '4245 (old-worker) S 0 0 0 0 0 0 0 0 0 0 900000 0 0 0 0 0 0 0 0 0 100\n' >"${ROOT}/${one}/4245/stat"
done
holds slow '.state == "fine" and (.words | contains("old-worker") | not)' "a program that was busy and is idle now is not named"
quiet; proc 4246 'we) ird (' 1 1; proc 4247 spinner 80 1
holds slow '.words | contains("spinner")' "a program busy now is named, and a name that holds brackets does not confuse the reading"
quiet; printf 'some avg10=30.00 avg60=30.00 avg300=9.00 total=1\n' >"${ROOT}/proc/pressure/memory"; proc 4248 CANARY-HEAVY 1 40; proc 4249 CANARY-BUSY 30 20
holds slow '.state == "found" and (.words | contains("Memory")) and (.words | contains("CANARY-HEAVY"))' "memory short, named by memory and not by the processor"
quiet; says powerprofilesctl_get power-saver
holds slow '.fix.command == ["ame","system","power-mode","balanced"]' "Power Saver"
quiet
holds slow '.state == "fine" and (.words | contains("Nothing is keeping") | not)' "nothing busy"
quiet; says df_-h_--output_pcent_avail_/var/home $'Use% Avail\n 90% 9G'
holds space '.fix.command == ["ame","system","clean"]' "a full disk's room"
quiet
holds space '.state == "fine"' "room left"
quiet; rm -f "${SAYS}"/df_*
holds space '.state == "fine" and .fix == null and (.words | contains("0%") | not) and (.words | contains("could not read"))' "a disk that could not be read is not 0% full"

# --- A device does nothing ------------------------------------------------------------------------------------------------
clear_says
echo '{"findings":[{"device":"Adapter","text":"has no firmware","fix":"ame apps xbox-adapter on"}]}' >"${ROOT}/var/lib/amethystora/drivers/status.json"
holds device '.fix.command == ["ame","apps","xbox-adapter","on"] and .fix.terminal == true' "a driver finding"
echo '{"findings":[{"device":"Dock","text":"needs its key","fix":"ame security secure-boot"}]}' >"${ROOT}/var/lib/amethystora/drivers/status.json"
holds device '.security.page == "settings/devices" and .fix == null' "a security fix opens Security and runs nothing"
clear_says
says systemctl_is-enabled_usbguard.service enabled
says systemctl_is-enabled_usbguard-dbus.service masked
holds device '.security.page == "settings/devices" and (.words | contains("may be keeping"))' "USB protection goes to Security, and is said as a possible cause"
clear_says
says systemctl_is-enabled_usbguard.service enabled
says systemctl_is-enabled_usbguard-dbus.service enabled
holds device '.security.page == "settings/devices" and (.words | contains("locked screen")) and (.words | contains("keeps new devices out") | not)' "USB protection of the locked screen is not said to keep devices out all the time"
clear_says
echo block-webcam >"${ROOT}/etc/amethystora/hardening-on"
holds device '.security.page == "settings/devices"' "a blocked webcam goes to Security"
clear_says
holds device '.state == "fine"' "nothing keeping a device out"

# --- Something else ---------------------------------------------------------------------------------------------------------
clear_says
says amethystora-doctor_--json '{"findings":[{"id":"selinux","level":"info","evidence":"x"},{"id":"update-failed","level":"problem","evidence":"uupd"}]}'
holds other '.fix.command == ["ame","update"] and .fix.terminal == true' "a failed update"
says amethystora-doctor_--json '{"findings":[{"id":"security","level":"warning","evidence":"x"}]}'
holds other '.security.page == "events"' "the watcher's finding goes to Security"
says amethystora-doctor_--json '{"findings":[]}'
holds other '.state == "fine"' "nothing the doctor finds"

# --- It keeps nothing, and quotes clean text ----------------------------------------------------------------------------------
clear_says
says wpctl_status $'Audio\n ├─ Sinks:\n │  *   52. HDMI\x1b[31m\xe2\x80\xaeCANARY-TV [vol: 1.00]\n │\n ├─ Sources:'
sound_answer="$(look sound)"
[[ "${sound_answer}" != *$'\x1b'* && "${sound_answer}" != *$'\xe2\x80\xae'* ]] || fail "a device's name was not cleaned: ${sound_answer}"
[[ -z "$(find "${HOME}" -mindepth 1 -print -quit)" ]] || fail "it wrote into the home: $(find "${HOME}" -mindepth 1)"
grep -rl CANARY "${HOME}" /tmp --include='*' 2>/dev/null | grep -v "^${ROOT}" | grep -q . && fail "a canary reached the disk"

if [[ "${failed}" != 0 ]]; then
    exit 1
fi
echo "test-wrong: every topic as it should be"
