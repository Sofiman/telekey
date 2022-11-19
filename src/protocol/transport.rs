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

impl From<TelekeyPacketKind> for u8 {
    fn from(kind: TelekeyPacketKind) -> Self {
        use TelekeyPacketKind::*;
        match kind {
            Handshake => 0,
            KeyEvent => 1,
            Ping => 2,
            Unknown => 255
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
        let len = msg.get_size() + 1 + 1; // the last +1 accounts for the packet type
        let mut payload: Vec<u8> = Vec::with_capacity(len);
        Writer::new(&mut payload).write_message(&msg)
            .expect("The payload should have been large enough");
        Self { kind, payload }
    }

    pub fn raw(kind: TelekeyPacketKind, payload: Vec<u8>) -> Self {
        Self { kind, payload }
    }

    pub fn kind(&self) -> TelekeyPacketKind {
        self.kind
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
    fn peer_addr(&self) -> io::Result<SocketAddr>;
}

pub struct TcpTransport {
    stream: TcpStream
}

impl TelekeyTransport for TcpTransport {
    fn recv_packet(&mut self) -> io::Result<TelekeyPacket> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        let len = u32::from_be_bytes(header); // deduce remaining bytes to read

        if len == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                  "Zero length packet received"));
        }

        let mut buf = vec![0; len as usize];
        self.stream.read_exact(&mut buf)?;
        Ok(TelekeyPacket::raw(buf.pop().unwrap().into(), buf))
    }

    fn send_packet(&mut self, mut p: TelekeyPacket) -> io::Result<()> {
        p.payload.push(p.kind().into());
        self.stream.write_all(&(p.payload.len() as u32).to_be_bytes())?;
        self.stream.write_all(&p.payload)
    }

    fn shutdown(&mut self) -> io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl TcpTransport {
    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }
}

impl From<TcpStream> for TcpTransport {
    fn from(stream: TcpStream) -> Self {
        Self { stream }
    }
}

impl From<TcpTransport> for TcpStream {
    fn from(tr: TcpTransport) -> Self {
        tr.stream
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
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        let len = u32::from_be_bytes(header); // deduce remaining bytes to read

        if len == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                  "Zero length packet received"));
        }

        let mut buf = vec![0; len as usize];
        self.stream.read_exact(&mut buf)?;
        let mut buf = aead::open(self.keys.receiving(), &buf).unwrap();
        Ok(TelekeyPacket::raw(buf.pop().unwrap().into(), buf))
    }

    fn send_packet(&mut self, mut p: TelekeyPacket) -> io::Result<()> {
        p.payload.push(p.kind().into());
        let msg = aead::seal(self.keys.transport(), &p.payload).unwrap();
        self.stream.write_all(&(msg.len() as u32).to_be_bytes())?;
        self.stream.write_all(&msg)
    }

    fn shutdown(&mut self) -> io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}
