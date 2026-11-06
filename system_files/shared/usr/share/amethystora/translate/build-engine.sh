#!/usr/bin/bash
# Builds Bergamot, the translator of R7-47 (bergamot-translator, MPL-2.0; Marian, MIT), from its source at one commit, inside
# the pinned Fedora container that `amethystora-translate engine` starts on the person's own machine, when they ask for the
# translation. It is not part of the image's build, which would take about twenty more minutes on every image, and the image
# carries no compiler: the program and its licences are written to /out, and nothing of either is passed on.
#
# Its upstream last moved in 2024, so a current compiler needs the first three changes. The last three are the limit's:
# /usr/share/amethystora/translate/models.json holds what this build was measured to use.
#
# Inputs: /out, a folder to write into; EXPECT_TREE, the SHA-256 of what was fetched (see TREE below); JOBS, the compile jobs.

set -euo pipefail

COMMIT=9271618ebbdc5d21ac4dc4df9e72beb7ce644774

dnf5 -y install --setopt=install_weak_deps=False git-core cmake gcc-c++ make pcre2-devel

mkdir -p /tmp/bergamot /out/licenses
cd /tmp/bergamot
git init --quiet
git remote add origin https://github.com/browsermt/bergamot-translator
git fetch --quiet --depth 1 origin "${COMMIT}"
git checkout --quiet FETCH_HEAD
# Only the parts that are compiled, each at the commit its parent names: the rest of the tree (tests, examples, the GPU
# libraries, a second web server, Python bindings) is neither built nor fetched
MARIAN=3rd_party/marian-dev
git submodule update --init --depth 1 --quiet 3rd_party/marian-dev 3rd_party/ssplit-cpp
(cd "${MARIAN}" && git submodule update --init --depth 1 --recursive --quiet \
    src/3rd_party/sentencepiece src/3rd_party/intgemm src/3rd_party/onnxjs)

# What was fetched, as one SHA-256 over every file of the tree and of those submodules, which the caller pinned: a commit id is
# SHA-1, and a repository that has moved or been replaced fails here and builds nothing
TREE="$(git ls-files --recurse-submodules -z | LC_ALL=C sort -z |
    while IFS= read -r -d '' file; do [[ -f "${file}" ]] && sha256sum -- "${file}"; done | sha256sum | cut -d' ' -f1)"
echo "tree ${TREE}"
[[ "${TREE}" == "${EXPECT_TREE}" ]] || { echo "The source is not the one that was pinned." >&2; exit 1; }

# The code predates GCC 15's headers, its newer warnings are not errors of ours, and CMake 4 refuses its oldest files
sed -i '1i #include <cstdint>' "${MARIAN}/src/microsoft/quicksand.h"
grep -rlE -- '-Werror' CMakeLists.txt "${MARIAN}/CMakeLists.txt" "${MARIAN}/src/CMakeLists.txt" src |
    xargs -r sed -i 's/-Werror=[a-z-]*//g; s/-Werror//g'
# Marian's own matrix routines rather than a BLAS library (its option offers them only to WebAssembly)
sed -i '/^CMAKE_DEPENDENT_OPTION(USE_ONNX_SGEMM /,+1c set(USE_ONNX_SGEMM ON)' "${MARIAN}/CMakeLists.txt"
# Less memory: smaller steps when the workspace grows, and the model moved rather than copied while it loads
sed -i 's/const size_t CHUNK = 128;/const size_t CHUNK = 32;/' "${MARIAN}/src/tensors/tensor_allocator.h"
sed -i 's/scorers.push_back(scorerByType(fname, weights\[i\], items, modelOptions));/scorers.push_back(scorerByType(fname, weights[i], std::move(items), modelOptions));/' \
    "${MARIAN}/src/translator/scorers.cpp"
sed -i 's/^        items_(items),$/        items_(std::move(items)),/' "${MARIAN}/src/translator/scorers.h"
grep -q 'CHUNK = 32;' "${MARIAN}/src/tensors/tensor_allocator.h"
grep -q 'items_(std::move(items))' "${MARIAN}/src/translator/scorers.h"
grep -q '^set(USE_ONNX_SGEMM ON)$' "${MARIAN}/CMakeLists.txt"

# x86-64 with intgemm's choice of instructions at run time, never the builder's own CPU. EIGEN_MPL2_ONLY makes the build fail
# if Eigen's non-MPL2 code is ever pulled in (onnxjs compiles only its MPL-2.0 routines today)
mkdir build
cd build
cmake .. -DCMAKE_BUILD_TYPE=Release -DBUILD_ARCH=x86-64 -DUSE_WASM_COMPATIBLE_SOURCE=OFF -DCOMPILE_TESTS=OFF \
    -DCOMPILE_UNIT_TESTS=OFF -DCMAKE_POLICY_VERSION_MINIMUM=3.5 '-DCMAKE_CXX_FLAGS=-include cstdint -w -DEIGEN_MPL2_ONLY' -DCMAKE_C_FLAGS=-w
# Marian's heaviest files take about a gigabyte each to compile, so the caller says how many
make -j"${JOBS:-2}" bergamot
install -Dpm0755 app/bergamot /out/amethystora-bergamot
strip /out/amethystora-bergamot
cd ..

# Every licence file in the compiled tree, under the path it has there; the one licence only a header carries (a BSD licence with a
# binary clause, Marian's half-float code) is cut out of it
find . -path ./build -prune -o -type f \( -iname 'LICEN[CS]E*' -o -iname 'COPYING*' -o -iname 'NOTICE*' \) -print |
    while read -r file; do
        install -Dpm0644 "${file}" "/out/licenses/${file#./}"
    done
HALF="${MARIAN}/src/3rd_party/half_float/umHalf.h"
mkdir -p "/out/licenses/${HALF%/*}"
sed -n '/^\/\*/,/\*\//p' "${HALF}" | sed '/\*\//q' >"/out/licenses/${HALF%.h}.LICENSE"
test -s "/out/licenses/${HALF%.h}.LICENSE"
test -s /out/licenses/LICENSE
test -s "/out/licenses/${MARIAN}/LICENSE.md"

cd /
rm -rf /tmp/bergamot
