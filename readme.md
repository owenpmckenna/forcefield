# Forcefield

Forcefield is a multi-hop vpn client and server manager, which allows users to build chains of both self-hosted and commercial VPN servers. 
This is to reduce the risk from any single entity/server being compromised or otherwise monitored.

### Features

* Create multi-hop vpn tunnels
* Automatically manage wireguard keys
* Tunnel wireguard over websocket to bypass Firewalls or Deep Packet Inspection
* Fully TUI/CLI based as of now - can be run on headless machines
* No need to open ports in firewall, can configure "reverse routes" for machines with outbound connections only

### Why though?

In the modern age, security and privacy is becoming increasingly difficult to access. 
Many tools exist to increase privacy, for instance modern, functionally unbreakable encryption and vpns, but they are cumbersome to use, even for those experienced in the field. 
Most people would turn to a VPN provider to make this process easier, but most of those are based in countries requiring them to divulge information to authorities. VPNs are also massive targets for hackers given the data they posess.
Even worse for privacy, it's now known that the US government and others monitor Tor nodes, even owning some of them. That's not to say that my threat model currently includes the US (or any other) Government, but it doesn't hurt to be prepared.
For these reasons, I have taken a shot at making my own VPN client, designed to be selfhosted on custom infrastructure.

### So what is this thing?

Forcefield is a project with two parts: a Terminal User Interface (the program on your device, the "citadel") and remote software (the VPN server controller, the "generator"). It allows anyone to build of a routing chain of Wireguard (VPN) servers, each being used to connect to the next. 
This can be used either for privacy or for management of services inside a network. One of the more useful features is that you can use reverse routes. 
Ordinarily, each server in the chain would need to have a publicly routable IP address. Forcefield can configure a generator behind a firewall to hold a connection open with a publicly available server. 
So as long as A) the target generator can connect to a public generator and B) you can connect to the same public generator, you can connect indirectly to the target.
 
Forcefield does provide the ability to run commands on your target, for all (some of) your C2 needs (lol don't commit crimes people). Forcefield also mostly supports ipv6, but it is still missing some more useful features. 

### How to use

0. Make sure you have cargo installed. Also, due to tight integration with the kernel networking stack, this project only works on Linux or in WSL.
1. Download/clone this repo, generate your keys, and run a build.
```shell
git clone https://github.com/owenpmckenna/forcefield
cd forcefield/key
openssl genrsa -out private_pkcs1.pem 4096
openssl rsa -in private_pkcs1.pem -pubout -out public.pem
cd ..
cargo build --release
```
2. Copy the client to your servers. It's at `target/release/generator`. It must be renamed to include the port number after an underscore.
```shell
scp ./target/release/generator user@remote-server:/home/user/forcefield_51820
```
3. Run all the Forcefield instances as root on every server you want to connect to.
4. Execute the citadel program: `./target/release/citadel`. The controls are as follows: Tab to switch panels, Up/Down and Left/Right for most others controls. Enter to select or confirm.
5. Select "Connect to Generator", enter the IP Address and port (`10.0.0.5:51820`), and press enter to connect. Do this for every machine you want to connect to.
6. Go down to "Set Wireguard Path" and press enter.
7. Select the servers you want to use, in the order you want to use them (Up/Down, Enter to select, left/right to set Websocket port). Then press Tab, enter the CIDRs that you want to talk to (or leave them the way they are), before pressing down and enter to confirm.
8. Test your internet. See if it works.
9. Try configuring a Generator. Go to "Control Generators" and select a generator, then do whatever you want.
10. (Optional) Configure a Generator in the middle of the chain. You should see a "Add Reverse Route Via ___" in the Routes column. Navigate to it and press enter. Now you will still be able to connect to your generator when it's behind a firewall/NAT