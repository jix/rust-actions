#!/bin/bash

set -euo pipefail
shopt -s globstar

: ${ZSTD_FLAGS:=-T0 -19}

tag="$1"
head_ref=$(git rev-parse HEAD)

out=target/actions_dist

rm -rf "$out"
mkdir -p "$out/bin"

(cd sccache; cargo build --target=x86_64-unknown-linux-musl --release \
    --no-default-features --features gha,openssl/vendored)
cargo build --target=x86_64-unknown-linux-musl --release

cp sccache/target/x86_64-unknown-linux-musl/release/sccache \
    "$out/bin/linux-x64-sccache"

cp target/x86_64-unknown-linux-musl/release/rust-actions \
    "$out/bin/linux-x64-rust-actions"

for file in "$out/bin"/*; do
    zstd $ZSTD_FLAGS "$file"
done

mkdir -p "$out/tree"

cp actions/js/loader.js "$out/tree"
if [[ -z "$DEBUG_URL" ]]; then
    echo 'module.exports = "'$head_ref'";' > "$out/tree/rev.js"
else
    echo 'module.exports = "'$DEBUG_URL'";' > "$out/tree/rev.js"
fi

for action in actions/yml/*.yml; do
    action=$(basename "$action" .yml)
    mkdir "$out/tree/$action"
    cp actions/yml/$action.yml "$out/tree/$action/action.yml"
    cp actions/js/stub.js "$out/tree/$action/main.js"
done

export GIT_INDEX_FILE=$PWD/$out/index

git update-index --add $(find "$out/tree" -type f)

dist_tree=$(git write-tree --prefix="$out/tree")
dist_commit=$(git commit-tree $dist_tree -m "rust-actions dist" -p $head_ref)

if [[ -z "$DEBUG_URL" ]]; then
    git tag "$tag" $dist_commit
    gh release create bin-$head_ref -p --target $head_ref \
        -n "Binaries for $head_ref" "$out/bin"/*.zst
    git push origin "refs/tags/$tag"
else
    git tag -f "$tag" $dist_commit
fi
