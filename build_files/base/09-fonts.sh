#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# The themes' interface and monospace fonts that Fedora does not package (each theme's font.theme and mono.theme,
# `ame desktop font` and `mono-font`), from Google Fonts' own repository at one commit, each file checked against the
# SHA-256 pinned beside it. Every one is under the SIL Open Font License 1.1, whose text ships with it in
# /usr/share/licenses/google-fonts/<family>. The ones Fedora packages are in 04-packages.sh; Fira Code's package ships
# no licence file, so its OFL.txt is pinned here without the font. A new commit is pinned by hand, with every sum
# again.
#
# In two halves (plans/PLAN-build-layers.md): `make` in the assets layer (layers/assets.Containerfile) downloads under
# ${ASSETS}, and `install` in the image, whose build copies the layer in (build.sh), adds Quicksand, builds the cache
# and checks every theme's fonts
GOOGLE_FONTS_COMMIT="6c1dbe27dcf7d817d4b62cec033ba77fa8e2a5ec"
GOOGLE_FONTS_URL="https://raw.githubusercontent.com/google/fonts/${GOOGLE_FONTS_COMMIT}/ofl"

# folder/file in the repository's ofl/, and its SHA-256; a folder's OFL.txt is its licence
FONTS=(
    # Atkinson Hyperlegible Next and Mono, drawn so that the letters people confuse (I l 1, O 0, rn m) look different:
    # offered to every theme as the interface and monospace font (`ame desktop font`, `mono-font`), named by none
    "atkinsonhyperlegiblemono/AtkinsonHyperlegibleMono[wght].ttf 5ce8b1698d1ded7dff2178c1a3ad159470085a58ea239e8b2cb88f4fb4a6f646"
    "atkinsonhyperlegiblemono/AtkinsonHyperlegibleMono-Italic[wght].ttf e18f523877530e4abe229df6a36d394326254da488a7f5abb9b7b5ea8f780cd4"
    "atkinsonhyperlegiblemono/OFL.txt 1ebb31cf7393164f20d10c1d48406cddb5314feff8465531cf1e4ba37e9dd740"
    "atkinsonhyperlegiblenext/AtkinsonHyperlegibleNext[wght].ttf 5a455d1cfa099b601ab70751bb9673e8fe1854dc4500c80e1a220d0d75e31745"
    "atkinsonhyperlegiblenext/AtkinsonHyperlegibleNext-Italic[wght].ttf ce9cffed32742ad2d9238c561a93220385e5934cdc02b8eb4097a50efa957dc6"
    "atkinsonhyperlegiblenext/OFL.txt aca6a428580965d2297d1b718042dd427c2a9443ece3b0d02d758e161e0c4030"
    "dmmono/DMMono-Regular.ttf 55b4c98f123daebb3ed27947ba47b2af00554fc6284d639a540bcef5e6258ad2"
    "dmmono/DMMono-Italic.ttf 32b5bad9cbce64eac6d05c8abbeb619317f7e4cb354e1c33db761adbfaae1b16"
    "dmmono/DMMono-Medium.ttf fd327daf461db87b44a87def475d251bf03b997f7c07d9680592d75dbbfaad0b"
    "dmmono/DMMono-MediumItalic.ttf a3b2211bb9cf5c4fc1a0b487553e673a737c2270c59b86bf1e0b48c77e587aa2"
    "dmmono/OFL.txt 2bada5ea45c3c63b7f1ea1f88ce9672c9e4f0c42b2c3b7378949084fe55a3066"
    "dmsans/DMSans[opsz,wght].ttf 8cd08d97e89c24d0aa92edd2f0f4c8ee6195eee9b7c9f154865a58b02f0c1c0d"
    "dmsans/DMSans-Italic[opsz,wght].ttf 22259c0cc8237221b80f44c76ba8d36e6bce3cda72779f5b2773643d499720ae"
    "dmsans/OFL.txt 9af36190332437f5ecd09974de43c1f7c77a310a996cdd8ceb25628b458840e1"
    "figtree/Figtree[wght].ttf 26ad3db9b31ff7dde67a91ff515d022d2f495cd506590699cf264f0bfe6fb714"
    "figtree/Figtree-Italic[wght].ttf 94dec1f18b9275d69e8b4a91b6514bdc18199048347e1c36e1285b11b0b87653"
    "figtree/OFL.txt 140d37233e7f3ce7313798befa9600893bcceaf41a55fa0fa5ad52f7f657a268"
    "firacode/OFL.txt 926041dac670e6922505e35ac1661a4e8d20f1ffeabbbcb5edb5544370702369"
    "firasans/FiraSans-Regular.ttf c29556a2719bf613ef3d5e070e40d903a8965d9c081beca1375dc1e6e0f93c23"
    "firasans/FiraSans-Italic.ttf a4204f9a8029c1c55daa5385ac95936f74abea3e232770f1fb2862a19d1dfa13"
    "firasans/FiraSans-Medium.ttf cbc1842cbed8c1d1146ba7c9db97d8f28c9bedfd25f41c5b0e1259ca48622328"
    "firasans/FiraSans-SemiBold.ttf db0321f83eb3e9f527b8af384a1b3fefdc1039cf2b06fd39b3f61492bda9561c"
    "firasans/FiraSans-Bold.ttf a4d8e149ecdd4874a0726eb0af894488b3b31c423d6b0017c8f415ed1b795b45"
    "firasans/FiraSans-BoldItalic.ttf 1b96c16db9819bcaa9d588068bb397bb6e977c4874965d3d50d789a1943757eb"
    "firasans/OFL.txt 8f24842e9174beda18a556c2ae7d54f5dc444340c19a3a9ef77e23bca366adbd"
    "karla/Karla[wght].ttf ed3ca4cd9bdd899c543927c30bf5ff50706b24b3f2b7328e64b25f7d2a9d23dc"
    "karla/Karla-Italic[wght].ttf 98c02551288197a43d9c7e62c105f4b6f283a57f0f06d623313ea32d5c701977"
    "karla/OFL.txt edf2c840acb9570fe02f40721126aa0da4e4011a0030cc6dce30780569f609b6"
    "lexend/Lexend[wght].ttf 3add53e641fbc81da64da4bb254285e2831b52b029527bc0714e2b9610832ee6"
    "lexend/OFL.txt 5da8505887d0fa7fe963445fd58852707fda34adfeb65af25c99d152bab285bd"
    "manrope/Manrope[wght].ttf 3ae11c49db0455a3cc33e37d380f20fdb8c7f8b41dc07625c177e3d87a9d6ae6"
    "manrope/OFL.txt 58172e0c0fac2cda8a37b348164bb55e44b0e69051e557e92b1d3f6910141f7b"
    "outfit/Outfit[wght].ttf fc7287273e66929776e2ba54f144fe699080bec29f61bf649d70d871468aeade"
    "outfit/OFL.txt c676351bf8576b9aba743cd5eaa8c0e7ee0d51f805d720447b4df4ddb6a2e416"
    "plusjakartasans/PlusJakartaSans[wght].ttf 89b3fb38aa0d275d7a731d0d817a4f1622b316b4d7fbdedcf02ee9099ff68bc8"
    "plusjakartasans/PlusJakartaSans-Italic[wght].ttf 9529eb888668b6a3c6dd75b6341a2fc5263fb6c9e788822e6117c29dd9e8b115"
    "plusjakartasans/OFL.txt 995c7199cab65954f545996326755daee7b63cc6b42b06c13da1f9502ab08a99"
    "sora/Sora[wght].ttf 84ff7096ae3ec6c8be47d906d1a0ba4de7f2ce78c615275c77301964a316e16c"
    "sora/OFL.txt ba0b9729c9428ba79a0459ab8ec575791b51509dbec213e383d0316d37fec299"
    "spacegrotesk/SpaceGrotesk[wght].ttf acad6de1fc93436f5c0f1f4137751ef04f1aea3063e7036535970ffcfbd79f72"
    "spacegrotesk/OFL.txt 564ce565c371c5e5bbf286006565a7c9aa55a9f56e7ca58d56e05d649dd61a72"
    "spacemono/SpaceMono-Regular.ttf 95837e182baeeada83368f7748db28357f0a1b75c6b84ff7065b5edf933c8e18"
    "spacemono/SpaceMono-Italic.ttf eca264c8690a91fe837d19a6badf2d88961610c4c8aafcb196526192a9f258c5"
    "spacemono/SpaceMono-Bold.ttf 405e73d41afb7e5906efce206a326af5c956f38e255f35421c260e861e599c59"
    "spacemono/SpaceMono-BoldItalic.ttf cdcc95eddee312879a9728494e595f591cb159f3f276d79bc58673dec7e57138"
    "spacemono/OFL.txt 8e4ee42b2553e1e01504e61cb0d46d148cd8c9e5eacaa3622a7df2d4f2955b9f"
    "victormono/VictorMono[wght].ttf 6fab3abe37b456f56d180987e04a3a0c326bace2cb825bc638b6be1eb03edf8f"
    "victormono/VictorMono-Italic[wght].ttf e393afee873d56e052c10bdfcbf67053c429895bd73a4f8e59100a09ffb9d8ec"
    "victormono/OFL.txt 493589852331b4ea83f1d6b0cbdc157b002ffdb995cf2429acf0c9863bc8f3df"
)

if [[ "${1:-make}" == make ]]; then
ASSETS="${ASSETS:-}"
WORK="$(mktemp -d)"
for entry in "${FONTS[@]}"; do
    path="${entry% *}"
    sum="${entry##* }"
    family="${path%%/*}"
    file="${path#*/}"
    # Brackets and commas in a variable font's name, escaped for the URL
    url_file="$(sed -e 's/\[/%5B/g' -e 's/\]/%5D/g' -e 's/,/%2C/g' <<<"${file}")"
    curl --fail --silent --show-error --location --retry 3 -o "${WORK}/${file}" "${GOOGLE_FONTS_URL}/${family}/${url_file}"
    echo "${sum}  ${WORK}/${file}" | sha256sum -c -
    if [[ ${file} == OFL.txt ]]; then
        install -Dpm0644 "${WORK}/${file}" "${ASSETS}/usr/share/licenses/google-fonts/${family}/OFL.txt"
    else
        install -Dpm0644 "${WORK}/${file}" "${ASSETS}/usr/share/fonts/amethystora/${family}/${file}"
    fi
done
rm -rf "${WORK}"
# The make half alone in the assets layer; without an argument (the lab's delta.sh) the install half follows
if [[ "${1:-}" == make ]]; then
    echo "::endgroup::"
    exit 0
fi
fi
# Every font the layer made is in place
for entry in "${FONTS[@]}"; do
    path="${entry% *}"
    if [[ ${path#*/} == OFL.txt ]]; then
        test -s "/usr/share/licenses/google-fonts/${path%%/*}/OFL.txt"
    else
        test -s "/usr/share/fonts/amethystora/${path%%/*}/${path#*/}"
    fi
done

# Quicksand, the wordmark's face, which the Amethystora theme writes in: the same file the apps carry (13-apps.sh)
install -Dpm0644 /ctx/branding/fonts/QuicksandVariable.ttf /usr/share/fonts/amethystora/quicksand/QuicksandVariable.ttf
install -Dpm0644 /ctx/branding/fonts/Quicksand-OFL.txt /usr/share/licenses/google-fonts/quicksand/OFL.txt

fc-cache --force /usr/share/fonts/amethystora

# Every family a theme names is installed, the packaged ones included, its monospace font among them
for font_theme in /ctx/system_files/shared/usr/share/amethystora/themes/*/font.theme /ctx/system_files/shared/usr/share/amethystora/themes/*/mono.theme; do
    family="$(head -n1 "${font_theme}")"
    # Read to its end: grep -q would stop fc-list mid-write, which pipefail counts as a failure
    fc-list : family | tr ',' '\n' | grep -Fx -- "${family}" >/dev/null ||
        { echo "::error::${font_theme} names ${family}, which is not installed"; exit 1; }
done

echo "::endgroup::"
