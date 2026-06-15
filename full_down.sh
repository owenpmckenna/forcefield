sudo docker kill gen_1
sudo docker kill gen_2
sudo docker kill gen_3
sudo docker kill citadel
sudo docker rm gen_1
sudo docker rm gen_2
sudo docker rm gen_3
sudo docker rm citadel

sudo rm -rf ./testing/
mkdir ./testing
mkdir ./testing/gen_1
mkdir ./testing/gen_2
mkdir ./testing/gen_3
mkdir ./testing/citadel

sudo docker network rm wg-network
