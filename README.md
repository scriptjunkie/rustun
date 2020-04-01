# rustun
Simple Linux tunnel ("VPN" but not encrypted/authenticated) in 100 lines of Rust. IPv4. This is a PoC, not an OpenVPN replacement.

# Building
- Clone or download this repository onto a Linux system
- Ensure you have a Rust build environment https://rustup.rs
- Run `cargo build --release`

# Usage
- Copy rustun to your Linux server and client systems
- On the server box, run `sudo ./rustun -s`
- On the client box, run `sudo ./rustun your.server.ip:14284`
- You should now have a tunnel. You can verify this by running `ifconfig` on each box
- Use your tunnel by running `ping 10.8.3.1` on your client. It should ping the server and you should see replies. If not, check firewalls in between.

# Advanced
Want to route your traffic through the tunnel?
- On your server run `sudo iptables -t nat -A POSTROUTING -j MASQUERADE ; echo "1" | sudo tee /proc/sys/net/ipv4/ip_forward` to enable forwarding and NAT'ing
- On your client set up a static /32 route for your target IP (so the VPN packets don't try to get routed through the VPN) then set 10.8.3.1 as your default gateway. Something like this might work, please substitute out $SERVERIP first though: `OLDGW=$(route -n | grep '^0\.0\.0\.0' | awk '{print $2}') ; route add -host $SERVERIP gw $OLDGW ; route add default gw 10.8.3.1`
