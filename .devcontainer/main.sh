mkdir key
if [ -z "$(find key -mindepth 1 -maxdepth 1)" ]; then
  echo "[+] No key. Generating..."
  cd key
  openssl genrsa -out private_pkcs1.pem 4096
  openssl rsa -in private_pkcs1.pem -pubout -out public.pem
  cd ..
fi
echo "[+] Running build citadel"
pwd
RUSTFLAGS="--cfg citadel" cargo build
cp target/debug/forcefield /opt/

sudo docker build -t wireguard_box ..
sudo docker network create --subnet 192.168.2.0/24 wg-network

tmux new-session -d -s dev
tmux send-keys -t dev:0 "./.devcontainer/a.sh" C-m
tmux split-window -h
tmux send-keys -t dev:0.1 "./.devcontainer/b.sh" C-m
tmux attach -t dev