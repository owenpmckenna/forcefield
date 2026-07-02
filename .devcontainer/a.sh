sudo docker run --network=wg-network --ip=192.168.2.2 --name citadel -v /opt/:/forcefield/ --cap-add=NET_ADMIN --device /dev/net/tun -it wireguard_box
