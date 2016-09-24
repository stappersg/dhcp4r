#[macro_use(u32_bytes, bytes_u32)]
extern crate dhcp4r;

use std::net::{UdpSocket, SocketAddr, Ipv4Addr, IpAddr};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::ops::Add;

use dhcp4r::{packet, options, server};

// Server configuration
const SERVER_IP: [u8; 4] = [192, 168, 0, 76];
const IP_START: [u8; 4] = [192, 168, 0, 180];
const SUBNET_MASK: [u8; 4] = [255, 255, 255, 0];
const DNS_IPS: [u8; 4] = [192, 168, 0, 254]; //[8, 8, 8, 8,8, 8, 4, 4]; // google dns servers
const ROUTER_IP: [u8; 4] = [192, 168, 0, 254];
const LEASE_DURATION_SECS: u32 = 7200;
const LEASE_NUM: u32 = 100;

// Derrived constants
const LEASE_DURATION_BYTES: [u8; 4] = u32_bytes!(LEASE_DURATION_SECS);
const IP_START_NUM: u32 = bytes_u32!(IP_START);

fn main() {
    let socket = UdpSocket::bind("0.0.0.0:67").unwrap();
    socket.set_broadcast(true).unwrap();

    let mut ms = my_server {
        leases: HashMap::new(),
        last_lease: 0,
        lease_duration: Duration::new(LEASE_DURATION_SECS as u64, 0),
    };

    server::Server::serve(socket, SERVER_IP, ms);
}

struct my_server {
    leases: HashMap<u32, ([u8; 6], Instant)>,
    last_lease: u32,
    lease_duration: Duration,
}

impl server::Handler for my_server {
    // fn handle_request(&Server, u8, Packet);
    fn handle_request(&mut self,
                      server: &server::Server,
                      msg_type: u8,
                      in_packet: packet::Packet) {
        match msg_type {
            dhcp4r::DISCOVER => {
                for _ in 0..LEASE_NUM {
                    // TODO prefer REQUESTED_IP_ADDRESS
                    self.last_lease = (self.last_lease + 1) % LEASE_NUM;
                    if self.available(&in_packet.chaddr, IP_START_NUM + &self.last_lease) {
                        reply(server,
                              dhcp4r::OFFER,
                              in_packet,
                              u32_bytes!(IP_START_NUM + &self.last_lease));
                        break;
                    }
                }
            }

            dhcp4r::REQUEST => {
                let req_ip = match in_packet.option(options::REQUESTED_IP_ADDRESS) {
                    None => in_packet.ciaddr,
                    Some(x) => {
                        if x.len() != 4 {
                            return;
                        } else {
                            [x[0], x[1], x[2], x[3]]
                        }
                    }
                };
                let req_ip_num = bytes_u32!(req_ip);
                if !&self.available(&in_packet.chaddr, req_ip_num) {
                    nak(server, in_packet, b"Requested IP not available");
                    return;
                }
                self.leases.insert(req_ip_num,
                                   (in_packet.chaddr, Instant::now().add(self.lease_duration)));
                reply(server, dhcp4r::ACK, in_packet, req_ip);
            }
            // Not technically necessary
            dhcp4r::RELEASE => {
                let ip_num = bytes_u32!(in_packet.ciaddr);
                if self.available(&in_packet.chaddr, ip_num) {
                    self.leases.remove(&ip_num);
                }
            }

            _ => {}
        }
    }
}

impl my_server {
    fn available(&self, chaddr: &[u8; 6], pos: u32) -> bool {
        return pos >= IP_START_NUM && pos < IP_START_NUM + LEASE_NUM &&
               match self.leases.get(&pos) {
            Some(x) => x.0 == *chaddr && Instant::now().gt(&x.1),
            None => true,
        };
    }
}

fn reply(s: &server::Server, msg_type: u8, req_packet: packet::Packet, offer_ip: [u8; 4]) {
    s.reply(msg_type,
            vec![options::Option {
                     code: options::IP_ADDRESS_LEASE_TIME,
                     data: &LEASE_DURATION_BYTES,
                 },
                 options::Option {
                     code: options::SUBNET_MASK,
                     data: &SUBNET_MASK,
                 },
                 options::Option {
                     code: options::ROUTER,
                     data: &ROUTER_IP,
                 },
                 options::Option {
                     code: options::DOMAIN_NAME_SERVER,
                     data: &DNS_IPS,
                 }],
            offer_ip,
            req_packet);
}

fn nak(s: &server::Server, req_packet: packet::Packet, message: &[u8]) {
    s.reply(dhcp4r::NAK,
            vec![options::Option {
                     code: options::MESSAGE,
                     data: message,
                 }],
            [0, 0, 0, 0],
            req_packet);
}
