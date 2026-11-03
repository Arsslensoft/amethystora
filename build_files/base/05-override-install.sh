#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# shellcheck source=build_files/shared/copr-helpers.sh
source /ctx/build_files/shared/copr-helpers.sh

# We do not need anything here at all
rm -rf /usr/src
# Except the licences. Some 120 of Fedora's packages keep their licence, copyright or NOTICE file among
# their documentation and nowhere else (libX11, pixman, zsh, the Apache NOTICE of httpd and apr), and
# the MIT, BSD and Apache licences ask for that text to travel with the program. So those files move
# beside the other licences, under the same folder name, before the documentation goes.
(
    cd /usr/share/doc
    find . -xtype f \( -iname '*licen[cs]e*' -o -iname '*copying*' -o -iname '*copyright*' -o -iname 'notice*' \
        -o -iname 'patents*' -o -iname 'authors*' -o -iname 'legal*' -o -iname 'third[-_]party*' \) \
        -exec sh -ec 'for file; do
            [ -e "/usr/share/licenses/${file}" ] || install -Dpm0644 "${file}" "/usr/share/licenses/${file}"
        done' sh {} +
)
rm -rf /usr/share/doc
# Remove kernel-devel from rpmdb because all package files are removed from /usr/src
rpm --erase --nodeps kernel-devel

# Multimedia: FFmpeg from Fedora, with RPM Fusion's libavcodec-freeworld for the codecs Fedora leaves
# out, in place of the negativo17 build the base image installs. That build is configured with
# --enable-nonfree, linking GPL encoders with CUDA and the Fraunhofer libraries, and FFmpeg itself
# labels it "nonfree and unredistributable"; Fedora's and RPM Fusion's are GPL-3.0-or-later.
# negativo17's pipewire-libs-extra goes too: its LC3plus plugin links liblc3plus, whose code comes with
# no licence to redistribute it. aptX comes from RPM Fusion instead, and Fedora's own PipeWire carries
# the other Bluetooth codecs (AAC, LDAC, LC3, Opus). negativo17's packages claim Fedora's names at the
# same or higher versions, so they are taken out first, and the replacements come only from the
# repositories named, never from negativo17's (fedora-multimedia, which outranks Fedora's). From 45 its
# libheif, which links x264, x265 and kvazaar, takes the place of Fedora's as well.
NEGATIVO17_MULTIMEDIA=(
    ffmpeg ffmpeg-libs libavcodec libavdevice libavfilter libavformat libavutil libswresample libswscale
    libfdk-aac mpeghdec liblc3plus libfreeaptx pipewire-libs-extra
    davs2-libs kvazaar-libs LCEVCdec libquirc librtmp libxavs svt-jpeg-xs uavs3d-libs vvenc-libs
    x264-libs x265-libs xavs2-libs xevd-libs xeve-libs libheif libde265 uvg266-libs vvdec-libs
)
readarray -t NEGATIVO17_INSTALLED < <(rpm -qa --queryformat '%{NAME}\t%{VENDOR}\n' "${NEGATIVO17_MULTIMEDIA[@]}" |
    awk -F'\t' '$2 == "negativo17.org" { print $1 }')
MULTIMEDIA_DEPENDENTS=()
if ((${#NEGATIVO17_INSTALLED[@]})); then
    # Whatever needs them now has to be satisfied by the replacements, which is checked below
    readarray -t MULTIMEDIA_DEPENDENTS < <(rpm -q --provides "${NEGATIVO17_INSTALLED[@]}" | awk '{ print $1 }' |
        sort -u | xargs -d '\n' rpm -q --whatrequires --queryformat '%{NAME}\n' | grep -v '^no package requires' |
        sort -u | grep -vxF -f <(printf '%s\n' "${NEGATIVO17_INSTALLED[@]}"))
    rpm --erase --nodeps "${NEGATIVO17_INSTALLED[@]}"
fi
dnf -y install --repo=fedora --repo=updates \
    ffmpeg-free fdk-aac-free libavcodec-free libavfilter-free libavformat-free libavutil-free \
    libswresample-free libswscale-free libheif libheif-ffmpeg
rpmfusion_release_install free
# Its repository files are in /etc/yum.repos.d before 45 and /usr/share/dnf5/repos.d from 45-1
dnf5 repo list --enabled --json | jq -r '.[].id | select(startswith("rpmfusion-free"))' |
    xargs -r -I{} dnf5 config-manager setopt "{}.enabled=0"
readarray -t RPMFUSION_MASTER < <(rpmfusion_master)
dnf -y install "${RPMFUSION_MASTER[@]}" --repo=fedora --repo=updates --repo=rpmfusion-free --repo=rpmfusion-free-updates \
    libavcodec-freeworld pipewire-codec-aptx
if ((${#MULTIMEDIA_DEPENDENTS[@]})); then
    rpm --verify --nofiles --noscripts "${MULTIMEDIA_DEPENDENTS[@]}"
fi

# Starship Shell Prompt, from a pinned release checked against the SHA-256 pinned beside it, not one
# fetched from the same release. Renovate bumps both together (.github/renovate.json5)
STARSHIP_TAG="v1.27.0"
STARSHIP_SHA256="3b94758850901717eb36aa1b53f8492c1322506ec6d41273bc6151e79ed8e4a1"
STARSHIP_URL="https://github.com/starship/starship/releases/download/${STARSHIP_TAG}/starship-x86_64-unknown-linux-gnu.tar.gz"
ghcurl "${STARSHIP_URL}" --fail --retry 3 -o /tmp/starship.tar.gz
echo "${STARSHIP_SHA256}  /tmp/starship.tar.gz" | sha256sum -c -
tar -xzf /tmp/starship.tar.gz -C /tmp
install -c -m 0755 /tmp/starship /usr/bin
# Its setup for each shell, as `starship init` prints it, written once here rather than by two runs of
# starship in every new terminal (profile.d/90-amethystora-starship.sh, /etc/zsh/zshrc,
# fish/vendor_conf.d/amethystora-starship.fish)
install -d /usr/share/amethystora/starship
for shell in bash zsh fish; do
    /usr/bin/starship init "${shell}" --print-full-init >"/usr/share/amethystora/starship/init.${shell}" 2>/dev/null
    grep -q "starship prompt" "/usr/share/amethystora/starship/init.${shell}"
done
# Fedora's zsh reads /etc/zshrc and never /etc/zsh, where the image's configuration is. Only zshrc is taken: the
# folder's zprofile would set LC_ALL to English in every login
echo 'source /etc/zsh/zshrc' >>/etc/zshrc

# fprintd with the duress finger (packaging/fprintd), which the Containerfile's fprintd stage built, in place
# of Fedora's. Its release begins with the highest of Fedora's it was made from, so a base image whose
# fprintd is another version or a later release carries a change of Fedora's this build lacks: the build
# stops until packaging/fprintd takes it (its README). The source package travels in the image.
FPRINTD_OURS="$(rpm -qp --queryformat '%{VERSION} %{RELEASE}' /fprintd/fprintd-pam-[0-9]*.rpm)"
FPRINTD_FEDORA="$(rpm -q --queryformat '%{VERSION} %{RELEASE}' fprintd-pam)"
FPRINTD_OURS_RELEASE="$(cut -d' ' -f2 <<<"${FPRINTD_OURS}" | cut -d. -f1)"
FPRINTD_FEDORA_RELEASE="$(cut -d' ' -f2 <<<"${FPRINTD_FEDORA}" | cut -d. -f1)"
if [[ "${FPRINTD_FEDORA%% *}" != "${FPRINTD_OURS%% *}" || ${FPRINTD_FEDORA_RELEASE} -gt ${FPRINTD_OURS_RELEASE} ]]; then
    echo "::error::Fedora's fprintd is ${FPRINTD_FEDORA}, past packaging/fprintd's ${FPRINTD_OURS}: take Fedora's change there"
    exit 1
fi
dnf5 -y install /fprintd/fprintd-[0-9]*."$(uname -m)".rpm /fprintd/fprintd-pam-[0-9]*."$(uname -m)".rpm
[[ "$(rpm -q --queryformat '%{RELEASE}\n' fprintd fprintd-pam | grep -c '\.amethystora\.')" == 2 ]]
install -D -m 0644 -t /usr/src/amethystora /fprintd/fprintd-*.src.rpm

# Remove desktop entries
if [[ -f /usr/share/applications/gnome-system-monitor.desktop ]]; then
    sed -i 's@\[Desktop Entry\]@\[Desktop Entry\]\nHidden=true@g' /usr/share/applications/gnome-system-monitor.desktop
fi
if [[ -f /usr/share/applications/org.gnome.SystemMonitor.desktop ]]; then
    sed -i 's@\[Desktop Entry\]@\[Desktop Entry\]\nHidden=true@g' /usr/share/applications/org.gnome.SystemMonitor.desktop
fi

echo "::endgroup::"
