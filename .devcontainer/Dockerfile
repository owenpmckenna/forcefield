FROM rust:1.96.0
RUN apt-get update && apt-get install -y wireguard iproute2 curl iputils-ping iptables speedtest-cli tcpdump tmux python3-full && rm -rf /var/lib/apt/lists/*
ENTRYPOINT ["sh", "-c", "./.devcontainer/main.sh"]