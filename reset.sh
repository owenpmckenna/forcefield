./full_down.sh &> /dev/null

echo "generator" > feature_flag
cargo build --release
cp ./target/release/forcefield ./testing/gen_1/forcefield_8080
cp ./target/release/forcefield ./testing/gen_2/forcefield_8080
cp ./target/release/forcefield ./testing/gen_3/forcefield_8080

echo "citadel" > feature_flag
cargo build --release
cp ./target/release/forcefield ./testing/citadel/forcefield

sudo docker build -t wireguard_box .
sudo docker network create --subnet 192.168.2.0/24 wg-network

sudo docker run --network=wg-network --ip=192.168.2.3 --name gen_1 -v ./testing/gen_1/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -d wireguard_box
sudo docker run --network=wg-network --ip=192.168.2.4 --name gen_2 -v ./testing/gen_2/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -d wireguard_box
sudo docker run --network=wg-network --ip=192.168.2.5 --name gen_3 -v ./testing/gen_3/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -d wireguard_box

sudo docker run --network=wg-network --ip=192.168.2.2 --name citadel -v ./testing/citadel/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -it wireguard_box
