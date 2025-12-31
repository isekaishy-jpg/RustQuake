use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use qw_common::NetAddr;

pub struct NetClient {
    socket: UdpSocket,
    remote: SocketAddr,
}

impl NetClient {
    pub fn connect(remote: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, remote })
    }

    #[allow(dead_code)]
    pub fn connect_netaddr(addr: NetAddr) -> io::Result<Self> {
        Self::connect(SocketAddr::from(addr.to_socket_addr()))
    }

    #[allow(dead_code)]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn send(&self, data: &[u8]) -> io::Result<usize> {
        self.socket.send_to(data, self.remote)
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        match self.socket.recv_from(buf) {
            Ok((size, addr)) => Ok(Some((size, addr))),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_round_trip() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let client = NetClient::connect(server_addr).unwrap();
        client.send(b"ping").unwrap();

        let mut buf = [0u8; 64];
        let (size, client_addr) = server.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..size], b"ping");

        server.send_to(b"pong", client_addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();

        let mut recv_buf = [0u8; 64];
        let mut received = None;
        for _ in 0..5 {
            if let Some((len, _)) = client.recv(&mut recv_buf).unwrap() {
                received = Some(len);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let len = received.expect("client received response");
        assert_eq!(&recv_buf[..len], b"pong");
    }
}
