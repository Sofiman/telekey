pub mod bindings;
use crate::protocol::bindings::api::*;
use console::Term;
use std::{io::{self, Read, Write, Error, ErrorKind}, net::*, borrow::Cow};
use rand::{distributions::Alphanumeric, Rng};
use quick_protobuf::{Writer, MessageWrite, deserialize_from_slice};

#[derive(Clone, Debug, Copy)]
pub enum TelekeyMode {
    Client,
    Server(u16)
}

#[derive(Clone, Debug)]
pub struct TelekeyConfig {
    hostname: String,
    version: u32,
    mode: TelekeyMode
}

impl TelekeyConfig {
    pub fn mode(&self) -> TelekeyMode {
        self.mode
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn set_mode(&mut self, mode: TelekeyMode) {
        self.mode = mode;
    }
}

impl Default for TelekeyConfig {
    fn default() -> Self {
        Self {
            hostname: match hostname::get().map(|h| h.into_string()) {
                Ok(Ok(hostname)) => hostname,
                _ => "Telekey Client".to_string()
            },
            version: 1,
            mode: TelekeyMode::Client
        }
    }
}

struct TelekeyRemote {
    me: TelekeyConfig,
    secret: Option<Vec<u8>>,
    remote: Option<TelekeyConfig>
}

impl From<HandshakeRequest<'_>> for TelekeyConfig {
    fn from(msg: HandshakeRequest) -> Self {
        Self {
            hostname: msg.hostname.to_string(),
            version: msg.version,
            mode: TelekeyMode::Client
        }
    }
}

impl TelekeyRemote {
    fn am_i_server(&self) -> bool {
        matches!(self.me.mode, TelekeyMode::Server(_))
    }

    fn is_secure(&self) -> bool {
        self.remote.is_some()
    }

    fn put_remote(&mut self, remote: TelekeyConfig) {
        self.remote = Some(remote);
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum TelekeyState {
    #[default]
    Idle,
    Active
}

impl From<console::Key> for KeyEvent {
    fn from(key: console::Key) -> Self {
        use console::Key::*;
        match key {
            Enter => Self { kind: KeyKind::ENTER, ..Default::default() },
            ArrowUp => Self { kind: KeyKind::UP, ..Default::default() },
            ArrowDown => Self { kind: KeyKind::DOWN, ..Default::default() },
            ArrowLeft => Self { kind: KeyKind::LEFT, ..Default::default() },
            ArrowRight => Self { kind: KeyKind::RIGHT, ..Default::default() },
            Escape => Self { kind: KeyKind::ESC, ..Default::default() },
            Backspace => Self { kind: KeyKind::BACKSPACE, ..Default::default() },
            Home => Self { kind: KeyKind::HOME, ..Default::default() },
            End => Self { kind: KeyKind::END, ..Default::default() },
            Tab => Self { kind: KeyKind::TAB, ..Default::default() },
            Del => Self { kind: KeyKind::DELETE, ..Default::default() },
            Insert => Self { kind: KeyKind::INSERT, ..Default::default() },
            PageUp => Self { kind: KeyKind::PAGEUP, ..Default::default() },
            PageDown => Self { kind: KeyKind::PAGEDOWN, ..Default::default() },
            Char(x) => Self { kind: KeyKind::CHAR, key: x as u32, ..Default::default() },
            _ => Self { kind: KeyKind::NONE, ..Default::default() },
        }
    }
}

impl std::fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            KeyKind::ENTER => writeln!(f),
            KeyKind::UP => write!(f, "[A^]"),
            KeyKind::DOWN => write!(f, "[Av]"),
            KeyKind::LEFT => write!(f, "[A<]"),
            KeyKind::RIGHT => write!(f, "[A>]"),
            KeyKind::BACKSPACE => write!(f, "[BAK]"),
            KeyKind::INSERT => write!(f, "[INS]"),
            KeyKind::CHAR => write!(f, "{}", char::from_u32(self.key).unwrap()),
            KeyKind::TAB => write!(f, "\t"),
            KeyKind::HOME => write!(f, "[HOM]"),
            KeyKind::ESC => write!(f, "[ESC]"),
            KeyKind::DELETE => write!(f, "[DEL]"),
            KeyKind::PAGEUP => write!(f, "[P^]"),
            KeyKind::PAGEDOWN => write!(f, "[Pv]"),
            KeyKind::END => write!(f, "[END]"),
            KeyKind::FUNCTION => write!(f, "[F{}]", self.key),
            KeyKind::NONE => write!(f, "[?]")
        }
    }
}

pub struct Telekey {
    remote: TelekeyRemote,
    state: TelekeyState
}

impl Telekey {
    pub fn new(conf: TelekeyConfig) -> Self {
        let remote = TelekeyRemote { me: conf, secret: None, remote: None };
        Telekey { remote, state: TelekeyState::Idle }
    }

    pub fn serve(port: u16, conf: TelekeyConfig) -> io::Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        println!("Server listenning on {} as `{}`", addr, conf.hostname);
        let listener = TcpListener::bind(addr)?;

        // accept connections and process them serially
        for stream in listener.incoming() {
            let secret: Vec<u8> = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(7)
                .collect();
            println!("Enter this token to confirm: {}",
                     String::from_utf8(secret.clone()).unwrap());
            let remote = TelekeyRemote { me: conf.clone(),
                secret: Some(secret), remote: None };
            let mut telekey = Telekey {
                remote,
                state: TelekeyState::Idle
            };
            if let Err(e) = telekey.listen_loop(stream?) {
                println!("<!> Got error {}", e);
            }
        }
        Ok(())
    }

    pub fn connect_to(addr: SocketAddr, conf: TelekeyConfig) -> io::Result<()> {
        let mut telekey = Telekey::new(conf);
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                println!("Connected to the server!");
                telekey.handshake(&mut stream)?;

                if let Err(e) = telekey.listen_loop(stream) {
                    println!("<!> Got error {}", e);
                }

                io::Result::Ok(())
            },
            Err(e) => {
                println!("Couldn't connect to server...");
                io::Result::Err(e)
            }
        }
    }

    fn handshake(&self, stream: &mut TcpStream) -> io::Result<()> {
        let mut inp = String::new();
        print!("Connection token: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut inp)?;

        let handshake = HandshakeRequest {
            hostname: Cow::Borrowed(&self.remote.me.hostname),
            version: self.remote.me.version,
            token: Cow::Borrowed(inp.trim().as_bytes())
        };
        Self::send_packet(stream, 0, handshake)
    }

    fn listen_loop(&mut self, mut stream: TcpStream) -> io::Result<()> {
        let mut header = [0u8; 5];

        loop {
            stream.read_exact(&mut header)?;
            let len = u32::from_be_bytes(header[1..].try_into().unwrap());
            self.handle_packet(&mut stream, header[0], len)?;
        }
    }

    fn handle_packet(&mut self, stream: &mut TcpStream, kind: u8, len: u32)
        -> io::Result<()>
    {
        let mut buf = vec![0; len as usize];
        stream.read_exact(&mut buf)?;
        match kind {
            0 => {
                if self.remote.is_secure() {
                    return Ok(());
                }
                let target_ip = stream.peer_addr().unwrap();
                if self.remote.am_i_server() {
                    let msg: HandshakeRequest = deserialize_from_slice(&buf)
                        .expect("Cannot read HandshakeRequest message");
                    let token: &[u8] = &msg.token.clone();
                    if self.remote.secret.as_ref().unwrap() != token {
                        stream.shutdown(Shutdown::Both)?;
                        return io::Result::Err(Error::new(ErrorKind::NotConnected, "Invalid secret"));
                    }
                    println!("-----------------------------");
                    println!("Remote:");
                    println!("\tIP: {:?}", target_ip);
                    println!("\tName: `{}`", msg.hostname);
                    println!("\tClient Version: {}", msg.version);
                    println!("-----------------------------");
                    Self::send_packet(stream, 0, HandshakeResponse {
                        hostname: Cow::Borrowed(&self.remote.me.hostname),
                        version: self.remote.me.version
                    })?;
                    self.remote.put_remote(msg.into());

                    self.wait_for_input(stream)?;
                } else {
                    let msg: HandshakeResponse = deserialize_from_slice(&buf)
                        .expect("Cannot read HandshakeResponse message");
                    println!("-----------------------------");
                    println!("Successfully connected to `{}`", msg.hostname);
                    println!("-----------------------------");
                    self.remote.put_remote(TelekeyConfig {
                        hostname: msg.hostname.to_string(),
                        version: msg.version,
                        mode: TelekeyMode::Server(target_ip.port())
                    });
                }
            },
            1 => {
                if !self.remote.is_secure() {
                    return stream.shutdown(Shutdown::Both);
                }
                if !self.remote.am_i_server() {
                    let msg: KeyEvent = deserialize_from_slice(&buf)
                        .expect("Cannot read KeyEvent message");
                    print!("{}", msg);
                    io::stdout().flush()?;
                }
            }
            _ => ()
        }
        Ok(())
    }

    fn send_packet<T: MessageWrite>(stream: &mut TcpStream, kind: u8, msg: T)
        -> io::Result<()> {
        let len = msg.get_size() + 1;
        stream.write_all(&[kind])?;
        stream.write_all(&(len as u32).to_be_bytes())?;
        let mut writer = Writer::new(stream);
        writer.write_message(&msg).map_err(|e|
            io::Error::new(io::ErrorKind::Other, e))
    }

    fn wait_for_input(&mut self, stream: &mut TcpStream) -> io::Result<()> {
        let term = Term::stdout();
        loop {
            match self.state {
                TelekeyState::Idle => {
                    if let Ok(_key) = term.read_key() {
                        self.state = TelekeyState::Active;
                        println!("switched to mode active");
                    }
                },
                TelekeyState::Active => {
                    if let Ok(key) = term.read_key() {
                        let e: KeyEvent = key.into();
                        Self::send_packet(stream, 1, e)?;
                    }
                }
            }
        }
    }
}
