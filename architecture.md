Citadel - thing running on user workstation
Generator - the thing running on servers, dropboxes, implants, etc.

Citadel stores its pub and priv key, saves pub key to every Generator's binary.
On start, Gens hold open a port according to their binary name, and generate pub-priv key pair.
On connection, they encrypt their public key w/ Hub's pub key and write it to the stream.
Hub decrypts Gen public key, then uses it to write back connection info.
Gen gets connection info, verifies it's real, then closes the initial config port, writes data
to disk, and spins up a wg port there with the pub and priv keys. A config port is spun up also.

Gen should have allowIps be set to a unique value inside 10.69.0.0/16.
Gens should have two "outgoing states" Default, and Peer(peer)

Connection Logic:
Generator (vpn provider)                 
1. Generate local keys (wireguard and temporary RSA)
2. Encrypt local keys w/ builtin citadel public RSA key
3. Wait for citadel connection
Citadel (your workstation)
4. Connect to generator
5. (Generator) Send encrypted keys to citadel
6. (Citadel) Receive and decrypt Generator keys with our private RSA
7. Send public wireguard keys, port to use, internal network IP, port
8. (Generator) decrypt configuration, restart using it