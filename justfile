set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

install-tools:
    rustup component add rustfmt clippy
    cargo install cargo-audit cargo-deny

fine: fmt clippy check build test doc

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets --all-features -- \
        -D warnings \
        -D clippy::unwrap_used \
        -D clippy::expect_used \
        -D clippy::panic \
        -D clippy::todo \
        -D clippy::unimplemented \
        -D clippy::undocumented_unsafe_blocks

check:
    cargo check --workspace --all-targets --all-features

build:
    cargo build --workspace --release --all-features

test:
    cargo test --workspace --all-features

doc:
    $env:RUSTDOCFLAGS = "-D warnings"; cargo doc --workspace --no-deps --all-features
