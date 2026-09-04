# Forcefield
In the modern age, security and privacy is becoming increasingly difficult to access. 
Many tools exist to increase privacy, for instance modern, functionally unbreakable encryption and vpns, but they are cumbersome to use, even for those experienced in the field. 
Most people would turn to a VPN provider to make this process easier, but most of those are based in countries requiring them to divulge information to authorities. VPNs are also massive targets for hackers given the data they posess.
Even worse for privacy, it's now known that the US government and others monitor Tor nodes, even owning some of them. That's not to say that my threat model currently includes the US (or any other) Government, but it doesn't hurt to be prepared.
For these reasons, I have taken a shot at making my own VPN client, designed to be selfhosted on custom infrastructure.

Forcefield is a project with two parts: a Terminal User Interface (the program on your device, the "citadel") and remote software (the VPN server controller, the "generator"). It allows anyone to build of a routing chain of Wireguard (VPN) servers, each being used to connect to the next. 
This can be used either for privacy or for management of services inside a network. One of the more useful features is that you can use reverse routes. 
Ordinarily, each server in the chain would need to have a publicly routable IP address to build up a chain. Forcefield can configure "reverse routes." 
So as long as A) the target Generator can connect to a public generator and B) you can connect to the same public generator, you can connect indirectly to the target.
 
Forcefield does provide the ability to run commands on your target, for all (some of) your C2 needs (lol don't commit crimes people). Forcefield also mostly supports ipv6, but it is still missing a feature there I would like to add. 
Eventually I want to assign the citadel a public ipv6 address should the last generator in the chain be connected to an ipv6 network. Finally, Forcefield can tunnel wireguard connections over Websocket with wstunnel to bypass DPI/Firewalls/whatever cellular networks are doing.