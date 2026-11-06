#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# The login screen's background: the default dark theme's wallpaper (Facets), so that the desktop starts from
# the picture the person logged in on. branding/generate.mjs draws it. GNOME reads it from inside
# GNOME Shell's own theme bundle and nowhere else, which the helper rebuilds with it; an image of one's own
# built on this one puts its picture there with the same helper (amethystora-brand). The helper also gives the lock
# screen its look, which no extension reaches: the large clock and date in Quicksand, and its cards of glass. It checks
# both in the rebuilt bundle, and Quicksand is installed by then (09-fonts.sh).
/usr/libexec/amethystora-login-background /usr/share/backgrounds/amethystora/amethystora-login.png
test -s /usr/share/fonts/amethystora/quicksand/QuicksandVariable.ttf

echo "::endgroup::"
