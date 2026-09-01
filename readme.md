# Forcefield
In the modern age, internet security and privacy is becoming increasingly difficult to access. 
Many tools exist to increase privacy, for instance modern, functionally unbreakable encryption, but they are difficult and cumbersome to use, even for those experienced in the field. 
Most people would turn to a VPN provider to make this process easier, but most of those are based in countries where they are legally required to divulge information to authorities.
Even worse for privacy, it's now known that the US government and others monitor Tor nodes, even owning some of them. That's not to say that my threat model currently includes the US Government, but it doesn't hurt to be prepared.
For these reasons, I have taken a shot at making my own VPN client, designed to be selfhosted on custom infrastructure.

Forcefield is a project with two parts: a Terminal User Interface (the program on your device, the "citadel") and remote software (the VPN server controller, the "generator"). It allows anyone to build of a routing chain of Wireguard (VPN) servers, each being used to connect to the next. 
This can be used either for privacy or for management of services inside a network. One of the more useful features is that you can use reverse routes. 
Ordinarily, each server in the chain would need to have a publicly routable IP address to build up a chain. Forcefield can configure "reverse routes." So
 
Due to the fact that the many remote implants are meant to be connected to by one main computer, "client" and "server" doesn't make much sense, so the "citadel" is the main computer to protect, and the "generators" are the devices to be connected to.
Forcefield allows anyone to set up any number of 

How it works is, a generator, when it spins up the first time, holds a TCP port open. When it gets a connection it encrypts two public keys (that of the wireguard interface it will run and it's own access key) with the citadel's public key and sends them.
The citadel will decrypt the keys and use one to instruct the Generator to begin normal operation with a new configuration (it sends ip addresses, more keys, etc.). The generator then resets and replaces the open config port with a wireguard endpoint. It spins up a different config port for later use.

