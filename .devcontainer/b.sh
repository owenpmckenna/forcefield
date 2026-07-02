pwd
echo "[+] building generator..."
RUSTFLAGS="--cfg generator" cargo build
mkdir /opt2/
cp ./target/release/forcefield /opt2/forcefield_8080
docker run --network=wg-network --ip=192.168.2.3 --name gen_1 -v /opt2/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -d wireguard_box
docker run --network=wg-network --ip=192.168.2.4 --name gen_2 -v /opt2/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -d wireguard_box
docker run --network=wg-network --ip=192.168.2.5 --name gen_3 -v /opt2/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -d wireguard_box

cat ./.devcontainer/instructions.txt
