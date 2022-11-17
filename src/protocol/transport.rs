use std::{io::{self, Write, Read}, net::{TcpStream, SocketAddr}};
use quick_protobuf::{MessageWrite, Writer};
use orion::{kex::SessionKeys, aead};

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
    len: u32,
    kind: TelekeyPacketKind,
    payload: Vec<u8>
}

impl TelekeyPacket {
    pub fn new<T: MessageWrite>(kind: TelekeyPacketKind, msg: T) -> Self {
        let len = msg.get_size() + 1;
        let mut payload: Vec<u8> = Vec::with_capacity(len);
        Writer::new(&mut payload).write_message(&msg)
            .expect("The payload should have been large enough");
        Self { kind, len: len as u32, payload }
    }

    pub fn raw(kind: TelekeyPacketKind, len: u32, payload: Vec<u8>) -> Self {
        Self { kind, len, payload }
    }

    pub fn kind(&self) -> TelekeyPacketKind {
        self.kind
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn data(&self) -> &[u8] {
        &self.payload
    }
}

pub trait TelekeyTransport {
    /// blocking function
    fn recv_packet(&mut self) -> io::Result<TelekeyPacket>;
    fn send_packet(&mut self, p: TelekeyPacket) -> io::Result<()>;
    fn shutdown(&mut self) -> io::Result<()>;
    fn peer_addr(&mut self) -> io::Result<SocketAddr>;
}

pub struct TcpTranspport {
    stream: TcpStream
}

impl TelekeyTransport for TcpTranspport {
    fn recv_packet(&mut self) -> io::Result<TelekeyPacket> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        // deduce remaining bytes to read
        let len = u32::from_be_bytes(header);

        let mut buf = vec![0; len as usize + 1];
        self.stream.read_exact(&mut buf)?;
        Ok(TelekeyPacket::raw(buf.pop().unwrap().into(), len, buf))
    }

    fn send_packet(&mut self, mut p: TelekeyPacket) -> io::Result<()> {
        self.stream.write_all(&p.len.to_be_bytes())?;
        p.payload.push(p.kind().into());
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

pub struct KexTransport {
    stream: TcpStream,
    keys: SessionKeys
}

impl KexTransport {
    pub fn new(stream: TcpStream, keys: SessionKeys) -> Self {
        Self { stream, keys }
    }
}

impl TelekeyTransport for KexTransport {
    fn recv_packet(&mut self) -> io::Result<TelekeyPacket> {
        let mut header = [0; 4];
        self.stream.read_exact(&mut header)?;
        let len = u32::from_be_bytes(header);

        let mut buf = vec![0; len as usize + 1];
        self.stream.read_exact(&mut buf)?;
        let mut buf = aead::open(self.keys.receiving(), &buf).unwrap();
        Ok(TelekeyPacket::raw(buf.pop().unwrap().into(), len, buf))
    }

    fn send_packet(&mut self, mut p: TelekeyPacket) -> io::Result<()> {
        p.payload.push(p.kind().into());
        let msg = aead::seal(self.keys.transport(), &p.payload).unwrap();
        self.stream.write_all(&p.len().to_be_bytes())?;
        self.stream.write_all(&msg)
    }

    fn shutdown(&mut self) -> io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)
    }

    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}
