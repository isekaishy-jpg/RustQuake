// IPv4 address parsing helpers.

use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct NetAddr {
    pub ip: [u8; 4],
    pub port: u16,
}

impl NetAddr {
    pub fn new(ip: [u8; 4], port: u16) -> Self {
        Self { ip, port }
    }

    pub fn to_string(&self) -> String {
        format!("{}.{}.{}.{}:{}", self.ip[0], self.ip[1], self.ip[2], self.ip[3], self.port)
    }

    pub fn to_socket_addr(&self) -> SocketAddrV4 {
        SocketAddrV4::new(
            Ipv4Addr::new(self.ip[0], self.ip[1], self.ip[2], self.ip[3]),
            self.port,
        )
    }

    pub fn parse(input: &str, default_port: u16) -> Result<Self, NetAddrError> {
        let mut parts = input.split(':');
        let ip_part = parts.next().ok_or(NetAddrError::InvalidFormat)?;
        let port_part = parts.next();

        if parts.next().is_some() {
            return Err(NetAddrError::InvalidFormat);
        }

        let octets: Vec<&str> = ip_part.split('.').collect();
        if octets.len() != 4 {
            return Err(NetAddrError::InvalidFormat);
        }

        let mut ip = [0u8; 4];
        for (i, octet) in octets.iter().enumerate() {
            let value = octet.parse::<u8>().map_err(|_| NetAddrError::InvalidFormat)?;
            ip[i] = value;
        }

        let port = match port_part {
            Some(port_str) if !port_str.is_empty() => port_str
                .parse::<u16>()
                .map_err(|_| NetAddrError::InvalidFormat)?,
            _ => default_port,
        };

        Ok(NetAddr { ip, port })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetAddrError {
    InvalidFormat,
}

impl fmt::Display for NetAddrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetAddrError::InvalidFormat => write!(f, "invalid net address"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_with_port() {
        let addr = NetAddr::parse("127.0.0.1:27500", 27001).unwrap();
        assert_eq!(addr.ip, [127, 0, 0, 1]);
        assert_eq!(addr.port, 27500);
    }

    #[test]
    fn parses_ipv4_without_port() {
        let addr = NetAddr::parse("10.0.0.5", 27001).unwrap();
        assert_eq!(addr.ip, [10, 0, 0, 5]);
        assert_eq!(addr.port, 27001);
    }

    #[test]
    fn converts_to_socket_addr() {
        let addr = NetAddr::new([127, 0, 0, 1], 27500);
        let socket = addr.to_socket_addr();
        assert_eq!(socket.ip().octets(), [127, 0, 0, 1]);
        assert_eq!(socket.port(), 27500);
    }
}
