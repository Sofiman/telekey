use std::{io::{self, Write, Read}, net::{TcpStream, SocketAddr}};
use quick_protobuf::{MessageWrite, Writer};

#[derive(Default, Debug, Clone, Copy)]
pub enum TelekeyPacketKind {
    #[default]
    Unknown,
    Handshake,
    KeyEvent,
    Ping
}

impl From<u8> for TelekeyPacketKind {
    fn from(id: u8) -> Self {
        match id {
            0 => Self::Handshake,
            1 => Self::KeyEvent,
            2 => Self::Ping,
            _ => Self::Unknown
        }
    }
}

impl Into<u8> for TelekeyPacketKind {
    fn into(self) -> u8 {
        match self {
            Self::Handshake => 0,
            Self::KeyEvent => 1,
            Self::Ping => 2,
            Self::Unknown => 255
        }
    }
}

#[derive(Debug, Clone)]
pub struct TelekeyPacket {
    kind: TelekeyPacketKind,
    payload: Vec<u8>
}

impl TelekeyPacket {
    pub fn new<T: MessageWrite>(kind: TelekeyPacketKind, msg: T) -> Self {
        let len = msg.get_size() + 1;
        let mut payload: Vec<u8> = Vec::with_capacity(5 + len);
        payload.push(kind.into());
        payload.extend_from_slice(&(len as u32).to_be_bytes());
        Writer::new(&mut payload).write_message(&msg)
            .expect("The payload should have been large enough");
        Self { kind, payload }
    }

    pub fn raw(payload: Vec<u8>) -> Self {
        assert!(payload.len() >= 5);
        Self { kind: payload[0].into(), payload }
    }

    pub fn kind(&self) -> TelekeyPacketKind {
        self.kind
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn data(&self) -> &[u8] {
        &self.payload[5..]
    }
}

pub trait TelekeyTransport {
    /// blocking function
    fn recv_packet(&mut self) -> io::Result<TelekeyPacket>;
    fn send_packet(&mut self, p: &TelekeyPacket) -> io::Result<()>;
    fn shutdown(&mut self) -> io::Result<()>;
    fn peer_addr(&mut self) -> io::Result<SocketAddr>;
}

pub struct TcpTranspport {
    stream: TcpStream
}

impl TelekeyTransport for TcpTranspport {
    fn recv_packet(&mut self) -> io::Result<TelekeyPacket> {
        let mut header = [0u8; 5];
        self.stream.read_exact(&mut header)?;

        // deduce remaining bytes to read
        let len = u32::from_be_bytes(header[1..].try_into().unwrap());
        if len == 0 {
            return Ok(TelekeyPacket::raw(header.to_vec()));
        }
        let mut buf = vec![0; len as usize + 5];

        self.stream.read_exact(&mut buf[5..])?;
        buf.splice(..5, header);
        Ok(TelekeyPacket::raw(buf))
    }

    fn send_packet(&mut self, p: &TelekeyPacket) -> io::Result<()> {
        self.stream.write_all(&p.payload)
    }

    fn shutdown(&mut self) -> io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)
    }

    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl From<TcpStream> for TcpTranspport {
    fn from(stream: TcpStream) -> Self {
        Self { stream }
    }
}
