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