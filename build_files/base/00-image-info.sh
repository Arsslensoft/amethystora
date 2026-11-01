#!/usr/bin/env bash

echo "::group:: ===$(basename "$0")==="

set -xeuo pipefail

IMAGE_PRETTY_NAME="Amethystora"
IMAGE_LIKE="fedora"
HOME_URL="https://github.com/arsslensoft/amethystora"
DOCUMENTATION_URL="https://github.com/arsslensoft/amethystora#readme"
SUPPORT_URL="https://github.com/arsslensoft/amethystora/issues/"
BUG_SUPPORT_URL="https://github.com/arsslensoft/amethystora/issues/"
CODE_NAME="Amethystora"
VERSION="${VERSION:-00.00000000}"

IMAGE_INFO="/usr/share/amethystora/image-info.json"
IMAGE_REF="ostree-image-signed:docker://ghcr.io/$IMAGE_VENDOR/$IMAGE_NAME"

# Image Flavor: the Justfile's flavor, which ends the image's name (amethystora-dx-nvidia-lts)
image_flavor="main"
if [[ "${IMAGE_NAME}" =~ -(nvidia-open|nvidia-lts)$ ]]; then
  image_flavor="${BASH_REMATCH[1]}"
fi

# When the base image was built, in seconds since the epoch: the created date of its image configuration,
# which the Justfile reads with skopeo (Universal Blue's images carry no org.opencontainers.image.created
# label). The report's image-age check reads it beside the image's own date, since packages that come
# with the base move only when image-versions.yml does, and 20-tests.sh warns of a base gone stale.
[[ "${BASE_IMAGE_CREATED:-}" =~ ^[0-9]+$ ]]

cat >$IMAGE_INFO <<EOF
{
  "image-name": "$IMAGE_NAME",
  "image-flavor": "$image_flavor",
  "image-vendor": "$IMAGE_VENDOR",
  "image-ref": "$IMAGE_REF",
  "image-tag":"$UBLUE_IMAGE_TAG",
  "base-image-name": "$BASE_IMAGE_NAME",
  "base-created": $BASE_IMAGE_CREATED,
  "fedora-version": "$FEDORA_MAJOR_VERSION"
}
EOF

# OS Release File
sed -i "s|^VARIANT_ID=.*|VARIANT_ID=$IMAGE_NAME|" /usr/lib/os-release
sed -i "s|^PRETTY_NAME=.*|PRETTY_NAME=\"${IMAGE_PRETTY_NAME} (Version: ${VERSION})\"|" /usr/lib/os-release
sed -i "s|^NAME=.*|NAME=\"$IMAGE_PRETTY_NAME\"|" /usr/lib/os-release
sed -i "s|^HOME_URL=.*|HOME_URL=\"$HOME_URL\"|" /usr/lib/os-release
sed -i "s|^DOCUMENTATION_URL=.*|DOCUMENTATION_URL=\"$DOCUMENTATION_URL\"|" /usr/lib/os-release
sed -i "s|^SUPPORT_URL=.*|SUPPORT_URL=\"$SUPPORT_URL\"|" /usr/lib/os-release
sed -i "s|^BUG_REPORT_URL=.*|BUG_REPORT_URL=\"$BUG_SUPPORT_URL\"|" /usr/lib/os-release
sed -i "s|^CPE_NAME=\"cpe:/o:fedoraproject:fedora|CPE_NAME=\"cpe:/o:${IMAGE_VENDOR}:${IMAGE_PRETTY_NAME,}|" /usr/lib/os-release
sed -i "s|^DEFAULT_HOSTNAME=.*|DEFAULT_HOSTNAME=\"${IMAGE_PRETTY_NAME,}\"|" /usr/lib/os-release
sed -i "s|^ID=fedora|ID=${IMAGE_PRETTY_NAME,}\nID_LIKE=\"${IMAGE_LIKE}\"|" /usr/lib/os-release
sed -i "/^REDHAT_BUGZILLA_PRODUCT=/d; /^REDHAT_BUGZILLA_PRODUCT_VERSION=/d; /^REDHAT_SUPPORT_PRODUCT=/d; /^REDHAT_SUPPORT_PRODUCT_VERSION=/d" /usr/lib/os-release
if grep -q "^LOGO=" /usr/lib/os-release; then
  sed -i "s|^LOGO=.*|LOGO=amethystora-logo|" /usr/lib/os-release
else
  echo "LOGO=amethystora-logo" >> /usr/lib/os-release
fi
sed -i "s|^VERSION_CODENAME=.*|VERSION_CODENAME=\"$CODE_NAME\"|" /usr/lib/os-release
sed -i "s|^VERSION=.*|VERSION=\"${VERSION}\"|" /usr/lib/os-release
sed -i "s|^OSTREE_VERSION=.*|OSTREE_VERSION=\'${VERSION}\'|" /usr/lib/os-release

if [[ -n "${SHA_HEAD_SHORT:-}" ]]; then
  echo "BUILD_ID=\"$SHA_HEAD_SHORT\"" >>/usr/lib/os-release
fi

# Added in systemd 249.
# https://www.freedesktop.org/software/systemd/man/latest/os-release.html#IMAGE_ID=
echo "IMAGE_ID=\"${IMAGE_NAME}\"" >> /usr/lib/os-release
echo "IMAGE_VERSION=\"${VERSION}\"" >> /usr/lib/os-release

# The rest of what Fedora's release packages wrote under Fedora's name (build.sh hands their place in
# the package database to generic-release): the line /etc/system-release and /etc/redhat-release show,
# the CPE beside it, and the software identification tags that call this system Fedora
echo "${IMAGE_PRETTY_NAME} release ${FEDORA_MAJOR_VERSION} (${CODE_NAME})" >/usr/lib/fedora-release
echo "cpe:/o:${IMAGE_VENDOR}:${IMAGE_PRETTY_NAME,}:${FEDORA_MAJOR_VERSION}" >/usr/lib/system-release-cpe
rm -rf /usr/lib/swidtag/fedoraproject.org /etc/swid/swidtags.d/fedoraproject.org

# Fix issues caused by ID no longer being fedora
sed -i "s|^EFIDIR=.*|EFIDIR=\"fedora\"|" /usr/sbin/grub2-switch-to-blscfg


# bazaar weekly downloads used for fastfetch
curl -X 'GET' \
'https://flathub.org/api/v2/stats/io.github.kolunmi.Bazaar?all=false&days=1' \
-H 'accept: application/json' | jq -r ".installs_last_7_days" | numfmt --to=si --round=nearest > /usr/share/amethystora/bazaar-install-count

echo "::endgroup::"
