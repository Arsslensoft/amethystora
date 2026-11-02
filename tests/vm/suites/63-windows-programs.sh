#!/usr/bin/bash
# Windows programs (amethystora-windows-program, plans/PLAN-windows-programs.md), as a person opening one meets them.
# Two phases in one boot:
#
#   1. the types and their default, a program opened from Files, and what the handler says and offers for each kind:
#      signed, changed after signing, unsigned, a known program, Steam, one Wine cannot run, a driver, a part of
#      Windows, known malware, and the image's own app opened in place of one
#   2. an unknown program run with Bottles: X11 granted to Bottles alone, a first bottle made, and the file reaching
#      Bottles through the document portal
# vmlab-paths: @sf/usr/libexec/amethystora-windows-program @sf/usr/share/amethystora/windows-programs.json @sf/usr/share/applications/zz-amethystora-open-anything.desktop build_files/shared/open-anything-types.py @ui/modules/tools/opener.js
source "$(dirname "$0")/lib.sh"

HOME_DIR="/var/home/${TEST_USER}"
HANDLER=/usr/libexec/amethystora-windows-program
FOLDER=Downloads/vmlab-windows
BOTTLES=com.usebottles.bottles
TYPES=(vnd.microsoft.portable-executable x-msdownload x-ms-dos-executable x-dosexec x-ms-ne-executable x-msi
    x-ms-shortcut msix appx msixbundle appxbundle x-bat)

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
TUID="$(id -u "${TEST_USER}")"
# A terminal in the account's session, as the one Files opens: term.py types the answers (in_terminal)
AS_TESTER=(runuser -u "${TEST_USER}" -- env "XDG_RUNTIME_DIR=/run/user/${TUID}" "WAYLAND_DISPLAY=wayland-0"
    "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/${TUID}/bus" bash -lc)
# put NAME <CONTENT: a file in the test folder, written as the account
put() { runuser -u "${TEST_USER}" -- sh -c 'mkdir -p "${1%/*}" && cat >"$1"' put "${HOME_DIR}/${FOLDER}/$1"; }
# opened NAME [EXPECT SEND]...: the handler on NAME in a terminal, the menu left with Ctrl+C when it shows
opened() {
    local name="$1"
    shift
    in_terminal 300 "$@" -- "${AS_TESTER[@]}" "cd ~/${FOLDER} && ${HANDLER} '${name}'"
}
# line_of TEXT STRING: the number of the first line of TEXT holding STRING
line_of() { grep -n -m1 -F -- "$2" <<<"$1" | cut -d: -f1; }
# make_pe FILE: a Windows program of the smallest kind, a PE32+ header and one section that returns at once. The
# image's own signed PE files (shim, the kernel) are signed in a way osslsigncode cannot read, so the suite makes and
# signs one as a Windows maker's signtool would
make_pe() {
    python3 - "$1" <<'PY'
import struct
import sys

dos = b'MZ' + bytes(0x3a) + struct.pack('<I', 0x40)
coff = struct.pack('<HHIIIHH', 0x8664, 1, 0, 0, 0, 240, 0x0022)
optional = (struct.pack('<HBBIIIII', 0x20b, 14, 0, 0x200, 0, 0, 0x1000, 0x1000) + struct.pack('<Q', 0x140000000)
            + struct.pack('<II', 0x1000, 0x200) + struct.pack('<HHHHHH', 6, 0, 0, 0, 6, 0) + struct.pack('<I', 0)
            + struct.pack('<III', 0x2000, 0x200, 0) + struct.pack('<HH', 3, 0x8160)
            + struct.pack('<QQQQ', 0x100000, 0x1000, 0x100000, 0x1000) + struct.pack('<II', 0, 16) + bytes(16 * 8))
section = b'.text\0\0\0' + struct.pack('<IIIIIIHHI', 0x10, 0x1000, 0x200, 0x200, 0, 0, 0, 0, 0x60000020)
headers = dos + b'PE\0\0' + coff + optional + section
with open(sys.argv[1], 'wb') as out:
    out.write(headers + bytes(0x200 - len(headers)) + b'\xc3' + b'vmlab windows program'.ljust(0x1ff, b'\0'))
PY
}

if [[ "${SUITE_PHASE}" == 1 ]]; then
    # Every type shared-mime-info gives a Windows program opens in the handler
    wrong=""
    for type in "${TYPES[@]}"; do
        default="$(in_session "xdg-mime query default application/${type}" 2>/dev/null)"
        [[ "${default}" == amethystora-windows-program.desktop ]] || wrong+=" ${type}=${default:-none}"
    done
    if [[ -z "${wrong}" ]]; then ok "all ${#TYPES[@]} Windows program types open in amethystora-windows-program.desktop"; else not_ok "types with another default:${wrong}"; fi

    # A program unsigned, and the same signed with a certificate of its own, under an installer's name
    WORK="$(mktemp -d)"
    make_pe "${WORK}/vmlab-tool.exe"
    openssl req -x509 -newkey rsa:2048 -nodes -days 2 -subj "/O=VMLab Publisher/CN=VMLab Publisher" \
        -keyout "${WORK}/key.pem" -out "${WORK}/cert.pem" >/dev/null 2>&1
    check "a test program is made and signed" osslsigncode sign -certs "${WORK}/cert.pem" -key "${WORK}/key.pem" -n vmlab \
        -in "${WORK}/vmlab-tool.exe" -out "${WORK}/signed.exe"
    SIGNED="${WORK}/signed.exe"
    put vmlab-tool.exe <"${WORK}/vmlab-tool.exe"
    put SpotifySetup.exe <"${SIGNED}"
    type="$(in_session "gio info -a standard::content-type ~/${FOLDER}/SpotifySetup.exe" 2>/dev/null | sed -n 's/^ *standard::content-type: //p')"
    check "a modern .exe is application/vnd.microsoft.portable-executable (${type})" test "${type}" = application/vnd.microsoft.portable-executable

    began=${SECONDS}
    out="$(opened SpotifySetup.exe 'What would you like to do' $'\x03')"
    echo "# SpotifySetup.exe, from opening to the choice (with clamscan's load): $((SECONDS - began)) s"
    scanned="$(line_of "${out}" 'Viruses: no known malware in it.')"
    signed="$(line_of "${out}" 'Signed in the name of VMLab Publisher, which this machine cannot confirm')"
    known="$(line_of "${out}" 'This is Spotify.')"
    if [[ -n "${scanned}" && -n "${signed}" && -n "${known}" ]] && ((scanned < signed && signed < known)); then
        ok "it is scanned, then its signer is said, then what it is: $(grep -m1 -o 'Signed [^.]*' <<<"${out}")"
    else
        not_ok "scan, signer and name, in that order" "$(head -n 20 <<<"${out}")"
    fi
    if grep -qF 'Install Spotify for Linux, from Flathub' <<<"${out}" && grep -qF 'Use Spotify on the web, as a web app (https://open.spotify.com)' <<<"${out}" &&
        grep -qF 'Run the Windows program, with Bottles' <<<"${out}" &&
        (($(line_of "${out}" 'Install Spotify for Linux') < $(line_of "${out}" 'Run the Windows program'))); then
        ok "Spotify's app from Flathub and its web version are offered before the Windows program"
    else
        not_ok "the choices for Spotify" "$(tail -n 12 <<<"${out}")"
    fi

    # Changed after it was signed: refused before anything is offered
    put tampered.exe <"${SIGNED}"
    printf 'vmlab' | runuser -u "${TEST_USER}" -- dd of="${HOME_DIR}/${FOLDER}/tampered.exe" bs=1 seek=$((0x210)) conv=notrunc status=none
    out="$(opened tampered.exe)"
    status=$?
    if [[ ${status} -ne 0 ]] && grep -qF 'Signature: it does NOT match the program' <<<"${out}" && ! grep -qF 'What would you like to do' <<<"${out}"; then
        ok "a program changed after it was signed is refused, and nothing is offered"
    else
        not_ok "the changed program (exit ${status})" "$(tail -n 10 <<<"${out}")"
    fi

    # Unsigned and unknown: only Bottles
    out="$(opened vmlab-tool.exe 'What would you like to do' $'\x03')"
    if grep -qF 'Signature: none. Nothing says who made it.' <<<"${out}" && grep -qF 'Run the Windows program, with Bottles' <<<"${out}" &&
        ! grep -qF 'Flathub' <<<"${out}"; then
        ok "an unsigned, unknown program says nobody signed it and offers Bottles alone"
    else
        not_ok "the unsigned program" "$(tail -n 12 <<<"${out}")"
    fi

    # Steam is set up by its ame command; Office never runs with Wine; a driver and a part of Windows are not programs
    put SteamSetup.exe <"${HOME_DIR}/${FOLDER}/vmlab-tool.exe"
    out="$(opened SteamSetup.exe 'What would you like to do' $'\x03')"
    check "Steam's installer offers ame apps gaming" grep -qF 'Set it up with: ame apps gaming' <<<"${out}"
    put OfficeSetup.exe <"${HOME_DIR}/${FOLDER}/vmlab-tool.exe"
    out="$(opened OfficeSetup.exe 'What would you like to do' $'\x03')"
    if grep -qF 'known not to run with Wine' <<<"${out}" && grep -qF 'Install LibreOffice from Flathub, which does the same job' <<<"${out}" &&
        ! grep -qF 'with Bottles' <<<"${out}"; then
        ok "Office is said not to run with Wine, LibreOffice is offered, and Bottles is not"
    else
        not_ok "the choices for Office" "$(tail -n 12 <<<"${out}")"
    fi
    put 581.57-desktop-win10-win11-64bit-international-dch-whql.exe <"${HOME_DIR}/${FOLDER}/vmlab-tool.exe"
    out="$(opened 581.57-desktop-win10-win11-64bit-international-dch-whql.exe)"
    if grep -qF 'This is the NVIDIA driver. Linux brings its own drivers' <<<"${out}" && ! grep -qF 'What would you like to do' <<<"${out}"; then
        ok "NVIDIA's Windows driver is said not to work here, and nothing is offered"
    else
        not_ok "the NVIDIA driver" "$(tail -n 8 <<<"${out}")"
    fi
    put driver.sys <"${HOME_DIR}/${FOLDER}/vmlab-tool.exe"
    out="$(opened driver.sys)"
    if grep -qF 'driver.sys is not a program you can run' <<<"${out}" && ! grep -qF 'Viruses:' <<<"${out}"; then
        ok "a .sys file is said to be no program"
    else
        not_ok "the .sys file" "$(tail -n 8 <<<"${out}")"
    fi

    # Known malware stops it before anything is read from the file
    printf '%s%s' 'X5O!P%@AP[4\PZX54(P^)7CC)7}$EICAR' '-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*' | put vmlab-eicar.exe
    out="$(opened vmlab-eicar.exe)"
    status=$?
    if [[ ${status} -ne 0 ]] && grep -qF 'Viruses: it matches known malware' <<<"${out}" && ! grep -q '^Sign' <<<"${out}"; then
        ok "a file that matches known malware is refused, before its signature is read"
    else
        not_ok "the EICAR file (exit ${status})" "$(tail -n 8 <<<"${out}")"
    fi

    # An antivirus installer opens the Security app, its launcher's name on the choice
    put mbsetup.exe <"${HOME_DIR}/${FOLDER}/vmlab-tool.exe"
    out="$(opened mbsetup.exe 'What would you like to do' $'\r')"
    if grep -qF 'Open Security, which is part of this system' <<<"${out}" && wait_for 60 pgrep -u "${TEST_USER}" -x amethystora-sec; then
        ok "an antivirus's installer offers the Security app, and opens it"
    else
        not_ok "Security from an antivirus's installer" "$(tail -n 8 <<<"${out}")"
    fi
    pkill -u "${TEST_USER}" -x amethystora-sec

    # Opened from Files: the default application starts the handler in a terminal of the session. In a unit that
    # leaves it running when gio ends (in_session's would stop the terminal with it)
    user_bus systemd-run --user --collect --quiet -p KillMode=process -- gio open "${HOME_DIR}/${FOLDER}/vmlab-tool.exe"
    if wait_for 60 pgrep -u "${TEST_USER}" -f "${HANDLER} .*vmlab-tool.exe"; then
        ok "opening an .exe in the session starts the handler in a terminal"
    else
        not_ok "gio open of an .exe started no handler" "$(pgrep -a -u "${TEST_USER}" | tail -n 15)"
    fi
    pkill -u "${TEST_USER}" -f "${HANDLER} .*vmlab-tool.exe"
    rm -rf "${WORK}"
    continue_phase
fi

if [[ "${SUITE_PHASE}" == 2 ]]; then
    if flatpak info "${BOTTLES}" >/dev/null 2>&1; then
        skip "Bottles was installed before this suite: its first run cannot be checked"
    else
        # Installed as a person at the machine would, through the handler; the account here is outside a polkit session,
        # so Bottles is installed first as root, and the handler finds it there
        check "Bottles installs from Flathub" flatpak install --system -y --noninteractive flathub "${BOTTLES}"
        began=${SECONDS}
        out="$(in_terminal 1500 'What would you like to do' $'\r' 'Run it with Bottles\?' 'y' -- "${AS_TESTER[@]}" \
            "cd ~/${FOLDER} && ${HANDLER} vmlab-tool.exe")"
        status=$?
        echo "# the first bottle, made and handed the program: $((SECONDS - began)) s"
        [[ ${status} -eq 0 ]] && ok "the handler ran Bottles" || not_ok "the handler with Bottles (exit ${status})" "$(tail -n 20 <<<"${out}")"
        overrides="$(in_session "flatpak override --user --show ${BOTTLES}" 2>/dev/null)"
        if grep -q '^sockets=.*\bx11\b' <<<"${overrides}" && ! grep -q 'x11' /var/lib/flatpak/overrides/"${BOTTLES}" 2>/dev/null; then
            ok "X11 is granted to Bottles for the account alone"
        else
            not_ok "Bottles' X11" "${overrides}"
        fi
        check "a bottle named Windows was made" test -f "${HOME_DIR}/.var/app/${BOTTLES}/data/bottles/bottles/Windows/bottle.yml"
        if wait_for 120 pgrep -u "${TEST_USER}" -f "\-e /run/user/${TUID}/doc/[^ ]*/vmlab-tool.exe"; then
            ok "Bottles was handed the program through the document portal, not the home folder"
        else
            not_ok "no Bottles with the portal's path" "$(pgrep -a -u "${TEST_USER}" -f bottles | tail -n 5)"
        fi
        pkill -u "${TEST_USER}" -f "${BOTTLES}"
        sleep 3
        in_session "flatpak override --user --reset ${BOTTLES}" >/dev/null 2>&1
        flatpak uninstall --system -y --noninteractive --delete-data "${BOTTLES}" >/dev/null 2>&1
        flatpak uninstall --system -y --noninteractive --unused >/dev/null 2>&1
        rm -rf "${HOME_DIR}/.var/app/${BOTTLES}"
    fi
    rm -rf "${HOME_DIR:?}/${FOLDER}"
    if [[ ! -e "${HOME_DIR}/${FOLDER}" && ! -e "${HOME_DIR}/.var/app/${BOTTLES}" ]] && ! flatpak info "${BOTTLES}" >/dev/null 2>&1; then
        ok "nothing of this suite is left"
    else
        not_ok "this suite left Bottles or its files"
    fi
    finish
fi
finish
