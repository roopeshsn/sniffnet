use std::fmt;

/// Enum representing the possible observed values of application layer protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(clippy::upper_case_acronyms)]
pub enum AppProtocol {
    /// File Transfer Protocol
    FTP,
    /// Secure Shell
    SSH,
    /// Telnet
    Telnet,
    /// Simple Mail Transfer Protocol
    SMTP,
    /// Terminal Access Controller Access-Control System
    TACACS,
    /// Domain Name System
    DNS,
    /// Dynamic Host Configuration Protocol
    DHCP,
    /// Trivial File Transfer Protocol
    TFTP,
    /// Hypertext Transfer Protocol
    HTTP,
    /// Post Office Protocol
    POP,
    /// Network Time Protocol
    NTP,
    /// NetBIOS
    NetBIOS,
    /// Post Office Protocol 3 over TLS/SSL
    POP3S,
    /// Internet Message Access Protocol
    IMAP,
    /// Simple Network Management Protocol
    SNMP,
    /// Border Gateway Protocol
    BGP,
    /// Lightweight Directory Access Protocol
    LDAP,
    ///Hypertext Transfer Protocol over TLS/SSL
    HTTPS,
    /// Lightweight Directory Access Protocol over TLS/SSL
    LDAPS,
    /// File Transfer Protocol over TLS/SSL
    FTPS,
    /// Multicast DNS
    #[allow(non_camel_case_types)]
    mDNS,
    ///Internet Message Access Protocol over TLS/SSL
    IMAPS,
    /// Simple Service Discovery Protocol
    SSDP,
    /// Extensible Messaging and Presence Protocol |
    XMPP,
    /// Not identified
    #[default]
    Unknown,
    /// Not applicable
    NotApplicable,
}

/// Given an integer in the range `0..=65535`, this function returns an `Option<AppProtocol>` containing
/// the respective application protocol represented by a value of the `AppProtocol` enum.
/// Only the most common application layer protocols are considered; if a unknown port number
/// is provided, this function returns `None`.
///
/// # Arguments
///
/// * `port` - An integer representing the transport layer port to be mapped to
/// an application layer protocol.
///
/// # Examples
///
/// ```
/// let x = from_port_to_application_protocol(25);
/// //Simple Mail Transfer Protocol
/// assert_eq!(x, Option::Some(AppProtocol::SMTP));
///
/// let y = from_port_to_application_protocol(1999);
/// //Unknown port-to-protocol mapping
/// assert_eq!(y, Option::None);
/// ```
pub fn from_port_to_application_protocol(port: Option<u16>) -> AppProtocol {
    if let Some(res) = port {
        match res {
            20..=21 => AppProtocol::FTP,
            22 => AppProtocol::SSH,
            23 => AppProtocol::Telnet,
            25 => AppProtocol::SMTP,
            49 => AppProtocol::TACACS,
            53 => AppProtocol::DNS,
            67..=68 => AppProtocol::DHCP,
            69 => AppProtocol::TFTP,
            80 | 8080 => AppProtocol::HTTP,
            109..=110 => AppProtocol::POP,
            123 => AppProtocol::NTP,
            137..=139 => AppProtocol::NetBIOS,
            143 | 220 => AppProtocol::IMAP,
            161..=162 | 199 => AppProtocol::SNMP,
            179 => AppProtocol::BGP,
            389 => AppProtocol::LDAP,
            443 => AppProtocol::HTTPS,
            636 => AppProtocol::LDAPS,
            989..=990 => AppProtocol::FTPS,
            993 => AppProtocol::IMAPS,
            995 => AppProtocol::POP3S,
            1900 => AppProtocol::SSDP,
            5222 => AppProtocol::XMPP,
            5353 => AppProtocol::mDNS,
            _ => AppProtocol::Unknown,
        }
    } else {
        // ICMP
        AppProtocol::NotApplicable
    }
}

impl fmt::Display for AppProtocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.eq(&AppProtocol::Unknown) {
            write!(f, "?")
        } else if self.eq(&AppProtocol::NotApplicable) {
            write!(f, "-")
        } else {
            write!(f, "{self:?}")
        }
    }
}

// impl AppProtocol {
// pub(crate) const ALL: [AppProtocol; 25] = [
//     AppProtocol::Unknown,
//     AppProtocol::BGP,
//     AppProtocol::DHCP,
//     AppProtocol::DNS,
//     AppProtocol::FTP,
//     AppProtocol::FTPS,
//     AppProtocol::HTTP,
//     AppProtocol::HTTPS,
//     AppProtocol::IMAP,
//     AppProtocol::IMAPS,
//     AppProtocol::LDAP,
//     AppProtocol::LDAPS,
//     AppProtocol::mDNS,
//     AppProtocol::NetBIOS,
//     AppProtocol::NTP,
//     AppProtocol::POP,
//     AppProtocol::POP3S,
//     AppProtocol::SMTP,
//     AppProtocol::SNMP,
//     AppProtocol::SSDP,
//     AppProtocol::SSH,
//     AppProtocol::TACACS,
//     AppProtocol::Telnet,
//     AppProtocol::TFTP,
//     AppProtocol::XMPP,
// ];
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_port_to_application_protocol_ftp() {
        let result1 = from_port_to_application_protocol(Some(20));
        assert_eq!(AppProtocol::FTP, result1);
        let result2 = from_port_to_application_protocol(Some(21));
        assert_eq!(AppProtocol::FTP, result2);
    }

    #[test]
    fn from_port_to_application_protocol_ssh() {
        let result = from_port_to_application_protocol(Some(22));
        assert_eq!(AppProtocol::SSH, result);
    }

    #[test]
    fn from_port_to_application_protocol_unknown() {
        let result = from_port_to_application_protocol(Some(500));
        assert_eq!(AppProtocol::Unknown, result);
    }

    #[test]
    fn from_port_to_application_protocol_not_applicable() {
        let result = from_port_to_application_protocol(None);
        assert_eq!(AppProtocol::NotApplicable, result);
    }

    #[test]
    fn app_protocol_display_ftp() {
        let test_str = AppProtocol::FTP.to_string();
        assert_eq!(test_str, "FTP");
    }

    #[test]
    fn app_protocol_display_unknown() {
        let test_str = AppProtocol::Unknown.to_string();
        assert_eq!(test_str, "?");
    }

    #[test]
    fn app_protocol_display_not_applicable() {
        let test_str = AppProtocol::NotApplicable.to_string();
        assert_eq!(test_str, "-");
    }
}
