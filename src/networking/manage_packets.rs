use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use chrono::Local;
use dns_lookup::lookup_addr;
use etherparse::{Ethernet2Header, IpHeader, PacketHeaders, TransportHeader};
use pcap::{Active, Address, Capture, Device};

use crate::mmdb::asn::get_asn;
use crate::mmdb::country::get_country;
use crate::mmdb::types::mmdb_reader::MmdbReader;
use crate::networking::types::address_port_pair::AddressPortPair;
use crate::networking::types::app_protocol::from_port_to_application_protocol;
use crate::networking::types::data_info_host::DataInfoHost;
use crate::networking::types::host::Host;
use crate::networking::types::icmp_type::{IcmpType, IcmpTypeV4, IcmpTypeV6};
use crate::networking::types::info_address_port_pair::InfoAddressPortPair;
use crate::networking::types::my_device::MyDevice;
use crate::networking::types::packet_filters_fields::PacketFiltersFields;
use crate::networking::types::traffic_direction::TrafficDirection;
use crate::networking::types::traffic_type::TrafficType;
use crate::utils::formatted_strings::get_domain_from_r_dns;
use crate::IpVersion::{IPv4, IPv6};
use crate::{AppProtocol, InfoTraffic, IpVersion, Protocol};

/// Calls methods to analyze link, network, and transport headers.
/// Returns the relevant collected information.
pub fn analyze_headers(
    headers: PacketHeaders,
    mac_addresses: &mut (Option<String>, Option<String>),
    exchanged_bytes: &mut u128,
    icmp_type: &mut IcmpType,
    packet_filters_fields: &mut PacketFiltersFields,
) -> Option<AddressPortPair> {
    analyze_link_header(headers.link, &mut mac_addresses.0, &mut mac_addresses.1);

    if !analyze_network_header(
        headers.ip,
        exchanged_bytes,
        &mut packet_filters_fields.ip_version,
        &mut packet_filters_fields.source,
        &mut packet_filters_fields.dest,
    ) {
        return None;
    }

    if !analyze_transport_header(
        headers.transport,
        &mut packet_filters_fields.sport,
        &mut packet_filters_fields.dport,
        &mut packet_filters_fields.protocol,
        icmp_type,
    ) {
        return None;
    }

    Some(AddressPortPair::new(
        packet_filters_fields.source.to_string(),
        packet_filters_fields.sport,
        packet_filters_fields.dest.to_string(),
        packet_filters_fields.dport,
        packet_filters_fields.protocol,
    ))
}

/// This function analyzes the data link layer header passed as parameter and updates variables
/// passed by reference on the basis of the packet header content.
/// Returns false if packet has to be skipped.
fn analyze_link_header(
    link_header: Option<Ethernet2Header>,
    mac_address1: &mut Option<String>,
    mac_address2: &mut Option<String>,
) {
    if let Some(header) = link_header {
        *mac_address1 = Some(mac_from_dec_to_hex(header.source));
        *mac_address2 = Some(mac_from_dec_to_hex(header.destination));
    } else {
        *mac_address1 = None;
        *mac_address2 = None;
    }
}

/// This function analyzes the network layer header passed as parameter and updates variables
/// passed by reference on the basis of the packet header content.
/// Returns false if packet has to be skipped.
fn analyze_network_header(
    network_header: Option<IpHeader>,
    exchanged_bytes: &mut u128,
    network_protocol: &mut IpVersion,
    address1: &mut IpAddr,
    address2: &mut IpAddr,
) -> bool {
    match network_header {
        Some(IpHeader::Version4(ipv4header, _)) => {
            *network_protocol = IpVersion::IPv4;
            *address1 = IpAddr::from(ipv4header.source);
            *address2 = IpAddr::from(ipv4header.destination);
            *exchanged_bytes = u128::from(ipv4header.payload_len);
            true
        }
        Some(IpHeader::Version6(ipv6header, _)) => {
            *network_protocol = IpVersion::IPv6;
            *address1 = IpAddr::from(ipv6header.source);
            *address2 = IpAddr::from(ipv6header.destination);
            *exchanged_bytes = u128::from(ipv6header.payload_length);
            true
        }
        _ => false,
    }
}

/// This function analyzes the transport layer header passed as parameter and updates variables
/// passed by reference on the basis of the packet header content.
/// Returns false if packet has to be skipped.
fn analyze_transport_header(
    transport_header: Option<TransportHeader>,
    port1: &mut Option<u16>,
    port2: &mut Option<u16>,
    protocol: &mut Protocol,
    icmp_type: &mut IcmpType,
) -> bool {
    match transport_header {
        Some(TransportHeader::Udp(udp_header)) => {
            *port1 = Some(udp_header.source_port);
            *port2 = Some(udp_header.destination_port);
            *protocol = Protocol::UDP;
            true
        }
        Some(TransportHeader::Tcp(tcp_header)) => {
            *port1 = Some(tcp_header.source_port);
            *port2 = Some(tcp_header.destination_port);
            *protocol = Protocol::TCP;
            true
        }
        Some(TransportHeader::Icmpv4(icmpv4_header)) => {
            *port1 = None;
            *port2 = None;
            *protocol = Protocol::ICMP;
            *icmp_type = IcmpTypeV4::from_etherparse(&icmpv4_header.icmp_type);
            true
        }
        Some(TransportHeader::Icmpv6(icmpv6_header)) => {
            *port1 = None;
            *port2 = None;
            *protocol = Protocol::ICMP;
            *icmp_type = IcmpTypeV6::from_etherparse(&icmpv6_header.icmp_type);
            true
        }
        _ => false,
    }
}

pub fn get_app_protocol(src_port: Option<u16>, dst_port: Option<u16>) -> AppProtocol {
    let mut application_protocol = from_port_to_application_protocol(src_port);
    if (application_protocol).eq(&AppProtocol::Unknown) {
        application_protocol = from_port_to_application_protocol(dst_port);
    }
    application_protocol
}

/// Function to insert the source and destination of a packet into the shared map containing the analyzed traffic.
pub fn modify_or_insert_in_map(
    info_traffic_mutex: &Arc<Mutex<InfoTraffic>>,
    key: &AddressPortPair,
    my_device: &MyDevice,
    mac_addresses: (Option<String>, Option<String>),
    icmp_type: IcmpType,
    exchanged_bytes: u128,
    application_protocol: AppProtocol,
) -> InfoAddressPortPair {
    let now = Local::now();
    let mut traffic_direction = TrafficDirection::default();

    if !info_traffic_mutex.lock().unwrap().map.contains_key(key) {
        // first occurrence of key

        // update device addresses
        let mut my_interface_addresses = Vec::new();
        for dev in Device::list().expect("Error retrieving device list\r\n") {
            if dev.name.eq(&my_device.name) {
                let mut my_interface_addresses_mutex = my_device.addresses.lock().unwrap();
                *my_interface_addresses_mutex = dev.addresses.clone();
                drop(my_interface_addresses_mutex);
                my_interface_addresses = dev.addresses;
                break;
            }
        }
        // determine traffic direction
        let source_ip = &key.address1;
        let destination_ip = &key.address2;
        traffic_direction = get_traffic_direction(
            source_ip,
            destination_ip,
            key.port1,
            key.port2,
            &my_interface_addresses,
        );
    };

    let mut info_traffic = info_traffic_mutex
        .lock()
        .expect("Error acquiring mutex\n\r");

    let new_info: InfoAddressPortPair = info_traffic
        .map
        .entry(key.clone())
        .and_modify(|info| {
            info.transmitted_bytes += exchanged_bytes;
            info.transmitted_packets += 1;
            info.final_timestamp = now;
            if key.protocol.eq(&Protocol::ICMP) {
                info.icmp_types
                    .entry(icmp_type)
                    .and_modify(|n| *n += 1)
                    .or_insert(1);
            }
        })
        .or_insert_with(|| InfoAddressPortPair {
            mac_address1: mac_addresses.0,
            mac_address2: mac_addresses.1,
            transmitted_bytes: exchanged_bytes,
            transmitted_packets: 1,
            initial_timestamp: now,
            final_timestamp: now,
            app_protocol: application_protocol,
            traffic_direction,
            icmp_types: if key.protocol.eq(&Protocol::ICMP) {
                HashMap::from([(icmp_type, 1)])
            } else {
                HashMap::new()
            },
        })
        .clone();

    if let Some(host_info) = info_traffic
        .addresses_resolved
        .get(&get_address_to_lookup(key, new_info.traffic_direction))
        .cloned()
    {
        if info_traffic.favorite_hosts.contains(&host_info.1) {
            info_traffic.favorites_last_interval.insert(host_info.1);
        }
    }

    new_info
}

pub fn reverse_dns_lookup(
    info_traffic: &Arc<Mutex<InfoTraffic>>,
    key: &AddressPortPair,
    traffic_direction: TrafficDirection,
    my_device: &MyDevice,
    country_db_reader: &Arc<MmdbReader>,
    asn_db_reader: &Arc<MmdbReader>,
) {
    let address_to_lookup = get_address_to_lookup(key, traffic_direction);
    let my_interface_addresses = my_device.addresses.lock().unwrap().clone();

    // perform rDNS lookup
    let lookup_result = lookup_addr(&address_to_lookup.parse().unwrap());

    // get new host info and build the new host
    let traffic_type = get_traffic_type(
        &address_to_lookup,
        &my_interface_addresses,
        traffic_direction,
    );
    let is_loopback = is_loopback(&address_to_lookup);
    let is_local = is_local_connection(&address_to_lookup, &my_interface_addresses);
    let country = get_country(&address_to_lookup, country_db_reader);
    let asn = get_asn(&address_to_lookup, asn_db_reader);
    let r_dns = if let Ok(result) = lookup_result {
        if result.is_empty() {
            address_to_lookup.clone()
        } else {
            result
        }
    } else {
        address_to_lookup.clone()
    };
    let new_host = Host {
        domain: get_domain_from_r_dns(r_dns.clone()),
        asn,
        country,
    };

    let mut info_traffic_lock = info_traffic.lock().unwrap();
    // collect the data exchanged from the same address so far and remove the address from the collection of addresses waiting a rDNS
    let other_data = info_traffic_lock
        .addresses_waiting_resolution
        .remove(&address_to_lookup)
        .unwrap_or_default();
    // insert the newly resolved host in the collections, with the data it exchanged so far
    info_traffic_lock
        .addresses_resolved
        .insert(address_to_lookup, (r_dns, new_host.clone()));
    info_traffic_lock
        .hosts
        .entry(new_host.clone())
        .and_modify(|data_info_host| {
            data_info_host.data_info += other_data;
        })
        .or_insert_with(|| DataInfoHost {
            data_info: other_data,
            is_favorite: false,
            is_loopback,
            is_local,
            traffic_type,
        });
    // check if the newly resolved host was featured in the favorites (possible in case of already existing host)
    if info_traffic_lock.favorite_hosts.contains(&new_host) {
        info_traffic_lock.favorites_last_interval.insert(new_host);
    }

    drop(info_traffic_lock);
}

/// Returns the traffic direction observed (incoming or outgoing)
fn get_traffic_direction(
    source_ip: &String,
    destination_ip: &String,
    source_port: Option<u16>,
    dest_port: Option<u16>,
    my_interface_addresses: &[Address],
) -> TrafficDirection {
    let my_interface_addresses_string: Vec<String> = my_interface_addresses
        .iter()
        .map(|address| address.addr.to_string())
        .collect();

    // first let's handle TCP and UDP loopback
    if is_loopback(source_ip) && is_loopback(destination_ip) {
        if let (Some(sport), Some(dport)) = (source_port, dest_port) {
            return if sport > dport {
                TrafficDirection::Outgoing
            } else {
                TrafficDirection::Incoming
            };
        }
    }

    if my_interface_addresses_string.contains(source_ip) {
        // source is local
        TrafficDirection::Outgoing
    } else if source_ip.ne("0.0.0.0") {
        // source not local and different from 0.0.0.0
        TrafficDirection::Incoming
    } else if !my_interface_addresses_string.contains(destination_ip) {
        // source is 0.0.0.0 (local not yet assigned an IP) and destination is not local
        TrafficDirection::Outgoing
    } else {
        TrafficDirection::Incoming
    }
}

/// Returns the traffic type observed (unicast, multicast or broadcast)
/// It refers to the remote host
pub fn get_traffic_type(
    destination_ip: &str,
    my_interface_addresses: &[Address],
    traffic_direction: TrafficDirection,
) -> TrafficType {
    if traffic_direction.eq(&TrafficDirection::Outgoing) {
        if is_multicast_address(destination_ip) {
            TrafficType::Multicast
        } else if is_broadcast_address(destination_ip, my_interface_addresses) {
            TrafficType::Broadcast
        } else {
            TrafficType::Unicast
        }
    } else {
        TrafficType::Unicast
    }
}

/// Determines if the input address is a multicast address or not.
///
/// # Arguments
///
/// * `address` - string representing an IPv4 or IPv6 network address.
fn is_multicast_address(address: &str) -> bool {
    let mut ret_val = false;
    if address.contains(':') {
        //IPv6 address
        if address.starts_with("ff") {
            ret_val = true;
        }
    } else {
        //IPv4 address
        let first_group = address
            .split('.')
            .next()
            .unwrap()
            .to_string()
            .parse::<u8>()
            .unwrap();
        if (224..=239).contains(&first_group) {
            ret_val = true;
        }
    }
    ret_val
}

/// Determines if the input address is a broadcast address or not.
///
/// # Arguments
///
/// * `address` - string representing an IPv4 or IPv6 network address.
fn is_broadcast_address(address: &str, my_interface_addresses: &[Address]) -> bool {
    if address.eq("255.255.255.255") {
        return true;
    }
    // check if directed broadcast
    let my_broadcast_addresses: Vec<String> = my_interface_addresses
        .iter()
        .map(|address| {
            address
                .broadcast_addr
                .unwrap_or_else(|| "255.255.255.255".parse().unwrap())
                .to_string()
        })
        .collect();
    if my_broadcast_addresses.contains(&address.to_string()) {
        return true;
    }
    false
}

fn is_loopback(address_to_lookup: &str) -> bool {
    IpAddr::from_str(address_to_lookup)
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
        .is_loopback()
}

/// Determines if the connection is local
pub fn is_local_connection(address_to_lookup: &str, my_interface_addresses: &Vec<Address>) -> bool {
    let mut ret_val = false;

    let address_to_lookup_type = if address_to_lookup.contains(':') {
        IPv6
    } else {
        IPv4
    };

    for address in my_interface_addresses {
        match address.addr {
            IpAddr::V4(local_addr) if address_to_lookup_type.eq(&IPv4) => {
                // check if the two IPv4 addresses are in the same subnet
                let address_to_lookup_parsed: Ipv4Addr = address_to_lookup
                    .parse()
                    .unwrap_or_else(|_| Ipv4Addr::from(0));
                // remote is link local?
                if address_to_lookup_parsed.is_link_local() {
                    ret_val = true;
                }
                // is the same subnet?
                else if let Some(IpAddr::V4(netmask)) = address.netmask {
                    let mut local_subnet = Vec::new();
                    let mut remote_subnet = Vec::new();
                    let netmask_digits = netmask.octets();
                    let local_addr_digits = local_addr.octets();
                    let remote_addr_digits = address_to_lookup_parsed.octets();
                    for (i, netmask_digit) in netmask_digits.iter().enumerate() {
                        local_subnet.push(netmask_digit & local_addr_digits[i]);
                        remote_subnet.push(netmask_digit & remote_addr_digits[i]);
                    }
                    if local_subnet == remote_subnet {
                        ret_val = true;
                    }
                }
            }
            IpAddr::V6(local_addr) if address_to_lookup_type.eq(&IPv6) => {
                // check if the two IPv6 addresses are in the same subnet
                let address_to_lookup_parsed: Ipv6Addr = address_to_lookup
                    .parse()
                    .unwrap_or_else(|_| Ipv6Addr::from(0));
                // remote is link local?
                if address_to_lookup.starts_with("fe80") {
                    ret_val = true;
                }
                // is the same subnet?
                else if let Some(IpAddr::V6(netmask)) = address.netmask {
                    let mut local_subnet = Vec::new();
                    let mut remote_subnet = Vec::new();
                    let netmask_digits = netmask.octets();
                    let local_addr_digits = local_addr.octets();
                    let remote_addr_digits = address_to_lookup_parsed.octets();
                    for (i, netmask_digit) in netmask_digits.iter().enumerate() {
                        local_subnet.push(netmask_digit & local_addr_digits[i]);
                        remote_subnet.push(netmask_digit & remote_addr_digits[i]);
                    }
                    if local_subnet == remote_subnet {
                        ret_val = true;
                    }
                }
            }
            _ => {}
        }
    }

    ret_val
}

/// Determines if the address passed as parameter belong to the chosen adapter
pub fn is_my_address(local_address: &String, my_interface_addresses: &Vec<Address>) -> bool {
    for address in my_interface_addresses {
        if address.addr.to_string().eq(local_address) {
            return true;
        }
    }
    is_loopback(local_address)
}

/// Determines if the capture opening resolves into an Error
pub fn get_capture_result(device: &MyDevice) -> (Option<String>, Option<Capture<Active>>) {
    let cap_result = Capture::from_device(device.to_pcap_device())
        .expect("Capture initialization error\n\r")
        .promisc(true)
        .snaplen(256) //limit stored packets slice dimension (to keep more in the buffer)
        .immediate_mode(true) //parse packets ASAP!
        .open();
    if cap_result.is_err() {
        let err_string = cap_result.err().unwrap().to_string();
        (Some(err_string), None)
    } else {
        (None, cap_result.ok())
    }
}

/// Converts a MAC address in its hexadecimal form
fn mac_from_dec_to_hex(mac_dec: [u8; 6]) -> String {
    let mut mac_hex = String::new();
    for n in &mac_dec {
        mac_hex.push_str(&format!("{n:02x}:"));
    }
    mac_hex.pop();
    mac_hex
}

pub fn get_address_to_lookup(key: &AddressPortPair, traffic_direction: TrafficDirection) -> String {
    match traffic_direction {
        TrafficDirection::Outgoing => key.address2.clone(),
        TrafficDirection::Incoming => key.address1.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use pcap::Address;

    use crate::networking::manage_packets::{
        get_traffic_direction, get_traffic_type, is_local_connection, mac_from_dec_to_hex,
    };
    use crate::networking::types::traffic_direction::TrafficDirection;
    use crate::networking::types::traffic_type::TrafficType;

    #[test]
    fn mac_simple_test() {
        let result = mac_from_dec_to_hex([255, 255, 10, 177, 9, 15]);
        assert_eq!(result, "ff:ff:0a:b1:09:0f".to_string());
    }

    #[test]
    fn mac_all_zero_test() {
        let result = mac_from_dec_to_hex([0, 0, 0, 0, 0, 0]);
        assert_eq!(result, "00:00:00:00:00:00".to_string());
    }

    #[test]
    fn ipv6_simple_test() {
        let result = IpAddr::from([
            255, 10, 10, 255, 255, 10, 10, 255, 255, 10, 10, 255, 255, 10, 10, 255,
        ]);
        assert_eq!(
            result.to_string(),
            "ff0a:aff:ff0a:aff:ff0a:aff:ff0a:aff".to_string()
        );
    }

    #[test]
    fn ipv6_zeros_in_the_middle() {
        let result =
            IpAddr::from([255, 10, 10, 255, 0, 0, 0, 0, 28, 4, 4, 28, 255, 1, 0, 0]).to_string();
        assert_eq!(result, "ff0a:aff::1c04:41c:ff01:0".to_string());
    }

    #[test]
    fn ipv6_leading_zeros() {
        let result =
            IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 28, 4, 4, 28, 255, 1, 0, 10]).to_string();
        assert_eq!(result, "::1c04:41c:ff01:a".to_string());
    }

    #[test]
    fn ipv6_tail_one_after_zeros() {
        let result =
            IpAddr::from([28, 4, 4, 28, 255, 1, 0, 10, 0, 0, 0, 0, 0, 0, 0, 1]).to_string();
        assert_eq!(result, "1c04:41c:ff01:a::1".to_string());
    }

    #[test]
    fn ipv6_tail_zeros() {
        let result =
            IpAddr::from([28, 4, 4, 28, 255, 1, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0]).to_string();
        assert_eq!(result, "1c04:41c:ff01:a::".to_string());
    }

    #[test]
    fn ipv6_multiple_zero_sequences_first_longer() {
        let result = IpAddr::from([32, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1]).to_string();
        assert_eq!(result, "2000::101:0:0:1".to_string());
    }

    #[test]
    fn ipv6_multiple_zero_sequences_first_longer_head() {
        let result = IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1]).to_string();
        assert_eq!(result, "::101:0:0:1".to_string());
    }

    #[test]
    fn ipv6_multiple_zero_sequences_second_longer() {
        let result = IpAddr::from([1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 3, 118]).to_string();
        assert_eq!(result, "100:0:0:1::376".to_string());
    }

    #[test]
    fn ipv6_multiple_zero_sequences_second_longer_tail() {
        let result = IpAddr::from([32, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0]).to_string();
        assert_eq!(result, "2000:0:0:1:101::".to_string());
    }

    #[test]
    fn ipv6_multiple_zero_sequences_equal_length() {
        let result = IpAddr::from([118, 3, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1]).to_string();
        assert_eq!(result, "7603::1:101:0:0:1".to_string());
    }

    #[test]
    fn ipv6_all_zeros() {
        let result = IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).to_string();
        assert_eq!(result, "::".to_string());
    }

    #[test]
    fn ipv6_x_all_zeros() {
        let result = IpAddr::from([161, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).to_string();
        assert_eq!(result, "a100::".to_string());
    }

    #[test]
    fn ipv6_all_zeros_x() {
        let result = IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 176]).to_string();
        assert_eq!(result, "::b0".to_string());
    }

    #[test]
    fn ipv6_many_zeros_but_no_compression() {
        let result = IpAddr::from([0, 16, 16, 0, 0, 1, 7, 0, 0, 2, 216, 0, 1, 0, 0, 1]).to_string();
        assert_eq!(result, "10:1000:1:700:2:d800:100:1".to_string());
    }

    #[test]
    fn traffic_direction_ipv4_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe80::8b1:1234:5678:d065".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ffff::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = get_traffic_direction(
            &"172.20.10.9".to_string(),
            &"99.88.77.00".to_string(),
            Some(99),
            Some(99),
            &address_vec,
        );
        assert_eq!(result1, TrafficDirection::Outgoing);
        let result2 = get_traffic_direction(
            &"172.20.10.10".to_string(),
            &"172.20.10.9".to_string(),
            Some(99),
            Some(99),
            &address_vec,
        );
        assert_eq!(result2, TrafficDirection::Incoming);
        let result3 = get_traffic_direction(
            &"172.20.10.9".to_string(),
            &"0.0.0.0".to_string(),
            Some(99),
            Some(99),
            &address_vec,
        );
        assert_eq!(result3, TrafficDirection::Outgoing);
        let result4 = get_traffic_direction(
            &"0.0.0.0".to_string(),
            &"172.20.10.9".to_string(),
            Some(99),
            Some(99),
            &address_vec,
        );
        assert_eq!(result4, TrafficDirection::Incoming);
        let result4 = get_traffic_direction(
            &"0.0.0.0".to_string(),
            &"172.20.10.10".to_string(),
            Some(99),
            Some(99),
            &address_vec,
        );
        assert_eq!(result4, TrafficDirection::Outgoing);
    }

    #[test]
    fn traffic_type_multicast_ipv4_test() {
        let result1 = get_traffic_type("227.255.255.0", &[], TrafficDirection::Outgoing);
        assert_eq!(result1, TrafficType::Multicast);
        let result2 = get_traffic_type("239.255.255.255", &[], TrafficDirection::Outgoing);
        assert_eq!(result2, TrafficType::Multicast);
        let result3 = get_traffic_type("224.0.0.0", &[], TrafficDirection::Outgoing);
        assert_eq!(result3, TrafficType::Multicast);
        let result4 = get_traffic_type("223.255.255.255", &[], TrafficDirection::Outgoing);
        assert_eq!(result4, TrafficType::Unicast);
        let result5 = get_traffic_type("240.0.0.0", &[], TrafficDirection::Outgoing);
        assert_eq!(result5, TrafficType::Unicast);

        let result6 = get_traffic_type("227.255.255.0", &[], TrafficDirection::Incoming);
        assert_eq!(result6, TrafficType::Unicast);
        let result7 = get_traffic_type("239.255.255.255", &[], TrafficDirection::Incoming);
        assert_eq!(result7, TrafficType::Unicast);
        let result8 = get_traffic_type("224.0.0.0", &[], TrafficDirection::Incoming);
        assert_eq!(result8, TrafficType::Unicast);
        let result9 = get_traffic_type("223.255.255.255", &[], TrafficDirection::Incoming);
        assert_eq!(result9, TrafficType::Unicast);
        let result10 = get_traffic_type("240.0.0.0", &[], TrafficDirection::Incoming);
        assert_eq!(result10, TrafficType::Unicast);
    }

    #[test]
    fn traffic_type_multicast_ipv6_test() {
        let result1 = get_traffic_type("ff::", &[], TrafficDirection::Outgoing);
        assert_eq!(result1, TrafficType::Multicast);
        let result2 = get_traffic_type("fe80:1234::", &[], TrafficDirection::Outgoing);
        assert_eq!(result2, TrafficType::Unicast);
        let result3 = get_traffic_type("ffff:ffff:ffff::", &[], TrafficDirection::Outgoing);
        assert_eq!(result3, TrafficType::Multicast);

        let result4 = get_traffic_type("ff::", &[], TrafficDirection::Incoming);
        assert_eq!(result4, TrafficType::Unicast);
        let result5 = get_traffic_type("fe80:1234::", &[], TrafficDirection::Incoming);
        assert_eq!(result5, TrafficType::Unicast);
        let result6 = get_traffic_type("ffff:ffff:ffff::", &[], TrafficDirection::Incoming);
        assert_eq!(result6, TrafficType::Unicast);
    }

    #[test]
    fn traffic_type_host_local_broadcast_test() {
        let result1 = get_traffic_type("255.255.255.255", &[], TrafficDirection::Outgoing);
        assert_eq!(result1, TrafficType::Broadcast);
        let result2 = get_traffic_type("255.255.255.255", &[], TrafficDirection::Incoming);
        assert_eq!(result2, TrafficType::Unicast);
        let result3 = get_traffic_type("255.255.255.254", &[], TrafficDirection::Outgoing);
        assert_eq!(result3, TrafficType::Unicast);

        let mut address_vec: Vec<Address> = Vec::new();
        let my_address = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        address_vec.push(my_address);

        let result1 = get_traffic_type("255.255.255.255", &address_vec, TrafficDirection::Outgoing);
        assert_eq!(result1, TrafficType::Broadcast);
        let result2 = get_traffic_type("255.255.255.255", &address_vec, TrafficDirection::Incoming);
        assert_eq!(result2, TrafficType::Unicast);
    }

    #[test]
    fn traffic_type_host_directed_broadcast_test() {
        let result1 = get_traffic_type("172.20.10.15", &[], TrafficDirection::Outgoing);
        assert_eq!(result1, TrafficType::Unicast);
        let result2 = get_traffic_type("172.20.10.15", &[], TrafficDirection::Incoming);
        assert_eq!(result2, TrafficType::Unicast);

        let mut address_vec: Vec<Address> = Vec::new();
        let my_address = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        address_vec.push(my_address);

        let result1 = get_traffic_type("172.20.10.15", &address_vec, TrafficDirection::Outgoing);
        assert_eq!(result1, TrafficType::Broadcast);
        let result2 = get_traffic_type("172.20.10.15", &address_vec, TrafficDirection::Incoming);
        assert_eq!(result2, TrafficType::Unicast);
    }

    #[test]
    fn is_local_connection_ipv4_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe80::8b1:1234:5678:d065".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ffff::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = is_local_connection("104.18.43.158", &address_vec);
        assert_eq!(result1, false);

        let result2 = is_local_connection("172.20.10.15", &address_vec);
        assert_eq!(result2, true);

        let result3 = is_local_connection("172.20.10.16", &address_vec);
        assert_eq!(result3, false);

        let result4 = is_local_connection("172.20.10.0", &address_vec);
        assert_eq!(result4, true);

        let result5 = is_local_connection("172.20.10.7", &address_vec);
        assert_eq!(result5, true);

        let result6 = is_local_connection("172.20.10.99", &address_vec);
        assert_eq!(result6, false);
    }

    #[test]
    fn is_local_connection_ipv6_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe90:8b1:1234:5678:d065::1234".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ff11::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = is_local_connection("fe90:8b1:1234:5611:d065::1234", &address_vec);
        assert_eq!(result1, false);

        let result2 = is_local_connection("fe90:8b1:1234:5610:d065::1234", &address_vec);
        assert_eq!(result2, true);

        let result3 = is_local_connection("ff90:8b1:1234:5610:d065::1234", &address_vec);
        assert_eq!(result3, false);

        let result4 = is_local_connection("fe90:8b1:1234:5610:ffff:eeee:9876:1234", &address_vec);
        assert_eq!(result4, true);
    }

    #[test]
    fn is_local_connection_ipv4_2_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.0".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe80::8b1:1234:5678:d065".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ffff::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = is_local_connection("255.255.255.255", &address_vec);
        assert_eq!(result1, false);

        let result2 = is_local_connection("172.20.10.9", &address_vec);
        assert_eq!(result2, true);

        let result3 = is_local_connection("172.20.10.9", &address_vec);
        assert_eq!(result3, true);

        let result4 = is_local_connection("172.20.10.9", &address_vec);
        assert_eq!(result4, true);

        let result5 = is_local_connection("172.20.10.7", &address_vec);
        assert_eq!(result5, true);

        let result6 = is_local_connection("172.20.10.99", &address_vec);
        assert_eq!(result6, true);

        let result7 = is_local_connection("172.20.11.0", &address_vec);
        assert_eq!(result7, false);

        let result8 = is_local_connection("172.20.9.255", &address_vec);
        assert_eq!(result8, false);
    }

    #[test]
    fn is_local_connection_ipv4_multicast_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe80::8b1:1234:5678:d065".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ffff::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = is_local_connection("224.0.0.251", &address_vec);
        assert_eq!(result1, false);
    }

    #[test]
    fn is_local_connection_ipv6_multicast_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe80::8b1:1234:5678:d065".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ffff::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = is_local_connection("ff::1234", &address_vec);
        assert_eq!(result1, false);
    }

    #[test]
    fn is_local_connection_ipv4_link_local_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe80::8b1:1234:5678:d065".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ffff::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = is_local_connection("224.0.1.2", &address_vec);
        assert_eq!(result1, false);

        let result2 = is_local_connection("169.254.17.199", &address_vec);
        assert_eq!(result2, true);

        let result3 = is_local_connection("169.255.17.199", &address_vec);
        assert_eq!(result3, false);
    }

    #[test]
    fn is_local_connection_ipv6_link_local_test() {
        let mut address_vec: Vec<Address> = Vec::new();
        let my_address_v4 = Address {
            addr: IpAddr::V4("172.20.10.9".parse().unwrap()),
            netmask: Some(IpAddr::V4("255.255.255.240".parse().unwrap())),
            broadcast_addr: Some(IpAddr::V4("172.20.10.15".parse().unwrap())),
            dst_addr: None,
        };
        let my_address_v6 = Address {
            addr: IpAddr::V6("fe90::8b1:1234:5678:d065".parse().unwrap()),
            netmask: Some(IpAddr::V6("ffff:ffff:ffff:ffff::".parse().unwrap())),
            broadcast_addr: None,
            dst_addr: None,
        };
        address_vec.push(my_address_v4);
        address_vec.push(my_address_v6);

        let result1 = is_local_connection("ff88::", &address_vec);
        assert_eq!(result1, false);

        let result2 = is_local_connection("fe80::8b1:1234:5678:d065", &address_vec);
        assert_eq!(result2, true);

        let result3 = is_local_connection("fe70::8b1:1234:5678:d065", &address_vec);
        assert_eq!(result3, false);
    }
}
