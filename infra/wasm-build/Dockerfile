# Pinned environment for the browser WebAssembly modules (auditor, signer,
# scanner, wallet), so a build on a laptop and a build in CI give the same
# bytes (review item D2, issue #61). Everything that reaches the .wasm is fixed
# here: the Rust toolchain (by image digest), the clang that compiles the
# secp256k1 C code, and wasm-bindgen. Build with scripts/build-wasm.sh.
FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581

# clang/llvm-ar for wasm32 (C dependencies), git for BUILD.json.
RUN apt-get update \
 && apt-get install -y --no-install-recommends clang-19 llvm-19 lld-19 git ca-certificates pkg-config \
 && rm -rf /var/lib/apt/lists/*

# rust-toolchain.toml asks for these; installing them here keeps builds offline.
RUN rustup component add rustfmt clippy && rustup target add wasm32-unknown-unknown

# Must match the wasm-bindgen crate version in Cargo.lock (the scripts check).
RUN cargo install --locked wasm-bindgen-cli --version 0.2.129 \
 && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

ENV LLVM_BIN=/usr/lib/llvm-19/bin \
    CARGO_TARGET_DIR=/target
RUN git config --system --add safe.directory /sealforge
WORKDIR /sealforge
