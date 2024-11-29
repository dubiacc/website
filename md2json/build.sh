cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-unknown-linux-gnux32
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
