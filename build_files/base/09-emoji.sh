#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# The emoji and symbols picker's lists and names (Super+., Amethystora UI's emoji module), made from Unicode's own
# files: Emoji 16.0's emoji-test.txt (the emoji, their order and skin tones), the Unicode Character Database's
# UnicodeData.txt (the names of the symbols CLDR leaves unnamed) and the Unicode CLDR's annotations, release 47 (the
# names and search words in English, French, German, Spanish and Arabic). Each download is checked against the
# SHA-256 pinned beside it; a new release is pinned by hand, with every sum again. All of them are under the Unicode
# License v3, which ships in /usr/share/licenses/unicode/LICENSE. The colour emoji font is the base image's: an emoji
# it cannot draw is left out (emoji-data.py says how many). The first two come from Unicode's unicodetools repository
# at the tag of the 17.0 release, which holds 16.0's files byte for byte as www.unicode.org publishes them: that
# server timed out every build of 2026-10-31 (Cloudflare 522).
CLDR_URL="https://raw.githubusercontent.com/unicode-org/cldr/release-47"
UNICODETOOLS_URL="https://raw.githubusercontent.com/unicode-org/unicodetools/final-17.0-20250910/unicodetools/data"

# name in the work folder, URL, SHA-256
FILES=(
    "emoji-test.txt ${UNICODETOOLS_URL}/emoji/16.0/emoji-test.txt 24f0c534e86cf142e2496953e8f0e46a3e702392911eddcd29c6cced85139697"
    "UnicodeData.txt ${UNICODETOOLS_URL}/ucd/16.0.0/UnicodeData.txt ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f"
    "ann-en.xml ${CLDR_URL}/common/annotations/en.xml b33e2e88ed2fb8c438c1efa9747b9d845e8d7d74ef0c32342a805c0f46fdd7ec"
    "ann-fr.xml ${CLDR_URL}/common/annotations/fr.xml 1c8215d15275a38197af51198a10a0ce84942da003700885f6f0a8bddb0b8641"
    "ann-de.xml ${CLDR_URL}/common/annotations/de.xml a36c3d4e0d21457035b4950ad325b678ec9116ca97ac71813eb18d82d696b895"
    "ann-es.xml ${CLDR_URL}/common/annotations/es.xml b703dcc863d7972d4168d680f73981c6fee7925acb583bcf0b266fb88d039776"
    "ann-ar.xml ${CLDR_URL}/common/annotations/ar.xml 0d402ff77c8947381b9f474548635264a8ee474896e857112b7b5e71b3f8a01d"
    "der-en.xml ${CLDR_URL}/common/annotationsDerived/en.xml 4f93a6780bcf83f85bf727af80ca18525b290910c378da169d5cb2ce938d765d"
    "der-fr.xml ${CLDR_URL}/common/annotationsDerived/fr.xml 283427cdc765424d3172e390aeabf153d97140e6665af18b240c013db3132c42"
    "der-de.xml ${CLDR_URL}/common/annotationsDerived/de.xml d66c8252841ce9f78221738c36ea76661b19306fd7289ccabcb99fdd5f14c42a"
    "der-es.xml ${CLDR_URL}/common/annotationsDerived/es.xml 356f546cb4dde519b017309b4a797369c802230a3ab7a4d826eae5f67c0bb2d5"
    "der-ar.xml ${CLDR_URL}/common/annotationsDerived/ar.xml 46f5c7f777c122a0d9f8863d90f115481f0b839c3d7055727c43da489e82fc94"
    "LICENSE ${CLDR_URL}/LICENSE b4c0ae8ef04f7059f96ce5bbe0467f9fe6f6d81bbe13517701dfeb961fb4d0b6"
)

WORK="$(mktemp -d)"
for entry in "${FILES[@]}"; do
    read -r file url sum <<<"${entry}"
    curl --fail --silent --show-error --location --retry 3 -o "${WORK}/${file}" "${url}"
    echo "${sum}  ${WORK}/${file}" | sha256sum -c -
done

python3 -I /ctx/build_files/shared/emoji-data.py "${WORK}" /usr/share/amethystora/emoji
chmod 0644 /usr/share/amethystora/emoji/*.json
install -Dpm0644 "${WORK}/LICENSE" /usr/share/licenses/unicode/LICENSE
rm -rf "${WORK}"

echo "::endgroup::"
