cd generator
cross build --target aarch64-unknown-linux-gnu --release
cargo build --release
cd ..
cd citadel
cargo build --release
cd ..