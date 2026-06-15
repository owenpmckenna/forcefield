FROM debian:13.5
RUN apt update && apt install wireguard iproute2 curl iputils-ping iptables speedtest-cli -y && rm -rf /var/lib/apt/lists/*
COPY ./container_run.sh /
ENTRYPOINT ["/bin/sh", "-c", "/container_run.sh"]