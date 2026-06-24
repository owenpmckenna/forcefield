echo "[+] building gen..."
echo "generator" > feature_flag
export PKG_CONFIG_ALLOW_CROSS=1
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar
RUSTFLAGS="--cfg generator" cross build --target aarch64-unknown-linux-gnu --release
RUST_BACKTRACE=1 RUSTFLAGS="--cfg generator" cargo build --release
echo "[+] uploading gen..."
#141.148.52.76
scp ./target/release/forcefield ubuntu@141.148.52.76:/home/ubuntu/forcefield_51820
scp ./target/aarch64-unknown-linux-gnu/release/forcefield owen@192.168.0.166:/home/owen/forcefield_8080

echo "[+] building citadel..."
echo "citadel" > feature_flag
RUST_BACKTRACE=1 RUSTFLAGS="--cfg citadel" cargo build --release
echo "[+] running citadel..."
cp ./target/release/forcefield /home/owen/Downloads/forcefield/
cd /home/owen/Downloads/forcefield/
chmod +x forcefield
./forcefield