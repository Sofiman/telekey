pub mod bindings;
use crate::protocol::bindings::api::*;
use chrono::{Utc, Duration};
use enigo::{Enigo, KeyboardControllable};
use console::{Term, style};
use std::{io::{self, Read, Write, Error, ErrorKind}, net::*, borrow::Cow};
use std::sync::Arc;
use openssl::ssl::{SslMethod, SslAcceptor, SslConnector, SslStream, SslFiletype};
use std::collections::VecDeque;
use rand::{distributions::Alphanumeric, Rng};
use quick_protobuf::{Writer, MessageWrite, deserialize_from_slice};

type TelekeySocket = SslStream<TcpStream>;

#[derive(Clone, Debug, Copy)]
pub enum TelekeyMode {
    Client,
    Server(u16)
}

#[derive(Clone, Debug)]
pub struct TelekeyConfig {
    hostname: String,
    update_screen: bool,
    refresh_latency: Option<usize>,
    cold_run: bool
}

#[allow(dead_code)]
impl TelekeyConfig {
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn set_update_screen(&mut self, update_screen: bool) {
        self.update_screen = update_screen;
    }

    pub fn set_refresh_latency(&mut self, refresh_latency: Option<usize>) {
        self.refresh_latency = refresh_latency;
    }

    pub fn set_cold_run(&mut self, cold_run: bool) {
        self.cold_run = cold_run;
    }
}

impl Default for TelekeyConfig {
    fn default() -> Self {
        Self {
            hostname: match hostname::get().map(|h| h.into_string()) {
                Ok(Ok(hostname)) => hostname,
                _ => "Telekey Client".to_string()
            },
            refresh_latency: Some(20),
            update_screen: true,
            cold_run: false
        }
    }
}

#[allow(dead_code)]
struct TelekeyRemote {
    hostname: String,
    version: u32,
    mode: TelekeyMode
}

impl From<HandshakeRequest<'_>> for TelekeyRemote {
    fn from(msg: HandshakeRequest) -> Self {
        Self {
            hostname: msg.hostname.to_string(),
            version: msg.version,
            mode: TelekeyMode::Client,
        }
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
            Shift => Self { kind: KeyKind::SHIFT, ..Default::default() },
            Char(x) => Self { kind: KeyKind::CHAR, key: x as u32, ..Default::default() },
            _ => Self { kind: KeyKind::UNKNOWN, ..Default::default() },
        }
    }
}

impl Into<Result<enigo::Key, String>> for &KeyEvent {
    fn into(self) -> Result<enigo::Key, String> {
        use KeyKind::*;
        match self.kind {
            ENTER => Ok(enigo::Key::Return),
            UP => Ok(enigo::Key::UpArrow),
            DOWN => Ok(enigo::Key::DownArrow),
            LEFT => Ok(enigo::Key::LeftArrow),
            RIGHT => Ok(enigo::Key::RightArrow),
            ESC => Ok(enigo::Key::Escape),
            BACKSPACE => Ok(enigo::Key::Backspace),
            HOME => Ok(enigo::Key::Home),
            END => Ok(enigo::Key::End),
            TAB => Ok(enigo::Key::Tab),
            DELETE => Ok(enigo::Key::Delete),
            CHAR => Ok(enigo::Key::Layout(char::from_u32(self.key).unwrap())),
            PAGEUP => Ok(enigo::Key::PageUp),
            PAGEDOWN => Ok(enigo::Key::PageDown),
            SHIFT => Ok(enigo::Key::Shift),
            META => Ok(enigo::Key::Meta),
            _ => Err(format!("From<KeyEvent> => enigo::Key for {:?}", self))
        }
    }
}

impl std::fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            KeyKind::ENTER => write!(f, "\\n"),
            KeyKind::UP => write!(f, "[A^]"),
            KeyKind::DOWN => write!(f, "[Av]"),
            KeyKind::LEFT => write!(f, "[A<]"),
            KeyKind::RIGHT => write!(f, "[A>]"),
            KeyKind::BACKSPACE => write!(f, "[BACKSPACE]"),
            KeyKind::INSERT => write!(f, "[INSERT]"),
            KeyKind::CHAR => write!(f, "{}", char::from_u32(self.key).unwrap()),
            KeyKind::TAB => write!(f, "\\t"),
            KeyKind::HOME => write!(f, "[HOM]"),
            KeyKind::ESC => write!(f, "[ESC]"),
            KeyKind::DELETE => write!(f, "[DEL]"),
            KeyKind::PAGEUP => write!(f, "[P^]"),
            KeyKind::PAGEDOWN => write!(f, "[Pv]"),
            KeyKind::END => write!(f, "[END]"),
            KeyKind::FUNCTION => write!(f, "[F{}]", self.key),
            KeyKind::SHIFT => write!(f, "[SHIFT]"),
            KeyKind::META => write!(f, "[WIN|CMD]"),
            KeyKind::UNKNOWN => write!(f, "[?]")
        }
    }
}

pub struct Telekey {
    config: TelekeyConfig,
    version: u32,
    mode: TelekeyMode,

    session_id: Option<Vec<u8>>,
    remote: Option<TelekeyRemote>,
    state: TelekeyState,
    enigo: Enigo
}

impl Telekey {
    pub fn is_server(&self) -> bool {
        matches!(self.mode, TelekeyMode::Server(_))
    }

    pub fn serve(port: u16, config: TelekeyConfig) -> io::Result<()> {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key_file("key.pem", SslFiletype::PEM).unwrap();
        acceptor.set_certificate_chain_file("cert.pem").unwrap();
        acceptor.check_private_key().unwrap();
        let acceptor = Arc::new(acceptor.build());

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr)?;
        println!("Server listenning on {} as `{}`", addr, config.hostname);

        // accept connections and process them serially
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let secret: Vec<u8> = rand::thread_rng()
                    .sample_iter(&Alphanumeric).take(8).collect();
                println!("Enter this token to confirm: {}",
                         String::from_utf8(secret.clone()).unwrap());
                let mut telekey = Telekey {
                    config: config.clone(), mode: TelekeyMode::Server(port),
                    version: 1, session_id: Some(secret), remote: None,
                    state: TelekeyState::Idle, enigo: Enigo::new()
                };
                let acceptor = acceptor.clone();
                let stream = acceptor.accept(stream).unwrap();
                if let Err(e) = telekey.listen_loop(stream) {
                    println!("{}: {}", style("ERROR").red().bold(), e);
                }
            }
        }
        Ok(())
    }

    pub fn connect_to(addr: SocketAddr, config: TelekeyConfig)
        -> io::Result<()> {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                let mut telekey = Telekey {
                    config, mode: TelekeyMode::Client, version: 1,
                    session_id: None, remote: None,
                    state: TelekeyState::Idle, enigo: Enigo::new()
                };
                let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
                connector.set_verify(openssl::ssl::SslVerifyMode::NONE);
                connector.set_ca_file("cert.pem").unwrap();
                let connector = connector.build();
                println!("{} connected to the server!",
                    style("Successfully").green().bold());
                let mut stream = connector.connect("127.0.0.1", stream).unwrap();
                telekey.handshake(&mut stream)?;

                if let Err(e) = telekey.listen_loop(stream) {
                    println!("{}: {}", style("ERROR").red().bold(), e);
                }

                Ok(())
            },
            Err(e) => {
                println!("{}: Couldn't connect to server...",
                         style("ERROR").red().bold());
                Err(e)
            }
        }
    }

    fn handshake(&self, stream: &mut TelekeySocket) -> io::Result<()> {
        let mut inp = String::new();
        print!("Please enter token to continue: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut inp)?;
        let bytes = inp.trim().as_bytes();
        if bytes.len() != 8 {
            return Err(Error::new(ErrorKind::Other, "Invalid token"));
        }

        let handshake = HandshakeRequest {
            hostname: Cow::Borrowed(&self.config.hostname),
            version: self.version,
            token: Cow::Borrowed(bytes)
        };
        Self::send_packet(stream, 0, handshake)
    }

    fn listen_loop(&mut self, mut stream: TelekeySocket) -> io::Result<()> {
        let mut header = [0u8; 5];

        loop {
            stream.read_exact(&mut header)?;
            let len = u32::from_be_bytes(header[1..].try_into().unwrap());
            self.handle_packet(&mut stream, header[0], len)?;
        }
    }

    fn handle_packet(&mut self, stream: &mut TelekeySocket, kind: u8, len: u32)
        -> io::Result<()> {
        let mut buf = vec![0; len as usize];
        stream.read_exact(&mut buf)?;
        match kind {
            0 => {
                if self.remote.is_some() {
                    return Ok(());
                }
                let Ok(target_ip) = stream.get_ref().peer_addr() else {
                    stream.shutdown().map_err(|e| Error::new(ErrorKind::Other, e))?;
                    return Ok(())
                };
                if self.is_server() {
                    let msg: HandshakeRequest = deserialize_from_slice(&buf)
                        .expect("Cannot read HandshakeRequest message");
                    let expected = self.session_id.as_ref()
                        .expect("Server must have a valid session ID");
                    let token: &[u8] = &msg.token;
                    if expected != token {
                        stream.shutdown().map_err(|e| Error::new(ErrorKind::Other, e))?;
                        return Err(Error::new(ErrorKind::NotConnected,
                                "Invalid secret"));
                    }
                    Self::send_packet(stream, 0, HandshakeResponse {
                        hostname: Cow::Borrowed(&self.config.hostname),
                        version: self.version
                    })?;
                    self.remote = Some(msg.into());

                    self.wait_for_input(stream)
                } else {
                    let msg: HandshakeResponse = deserialize_from_slice(&buf)
                        .expect("Cannot read HandshakeResponse message");
                    println!("{} {}", self.print_header(stream),
                        style(msg.hostname.to_string()).cyan());
                    self.remote = Some(TelekeyRemote {
                        hostname: msg.hostname.to_string(),
                        version: msg.version,
                        mode: TelekeyMode::Server(target_ip.port()),
                    });
                    Ok(())
                }
            },
            1 => {
                if self.remote.is_none() {
                    stream.shutdown().map_err(|e| Error::new(ErrorKind::Other, e))?;
                    return Ok(())
                }
                if !self.is_server() {
                    let msg: KeyEvent = deserialize_from_slice(&buf)
                        .expect("Cannot read KeyEvent message");

                    if self.config.cold_run {
                        print!("{}", msg);
                        io::stdout().flush()?;
                    } else {
                         // TODO: Support pressing and releasing keys rather
                         // than just pressing them
                        let r: Result<enigo::Key, String> = (&msg).into();
                        match r {
                            Ok(k) => self.enigo.key_click(k),
                            Err(e) => {
                                println!("{} while receiving `{}`: {:?}", 
                                         style("RUNTIME ERROR").yellow().bold(),
                                         style(format!("{}", msg)).green(), e);
                            }
                        }
                    }
                }
                Ok(())
            },
            2 => {
                let tm = Utc::now().timestamp_nanos();
                let mut buf: Vec<u8> = Vec::with_capacity(5 + 8);
                buf.push(2);
                buf.extend_from_slice(&(4u32).to_be_bytes());
                buf.extend_from_slice(&tm.to_be_bytes());
                stream.write_all(&buf)
            }
            _ => {
                println!("{}: Unknown packet {{ id: {}, len: {}b }}",
                     style("RUNTIME ERROR").yellow().bold(), kind, len);
                Ok(())
            }
        }
    }

    fn send_packet<T: MessageWrite, S: Write>(stream: &mut S, kind: u8, msg: T)
        -> io::Result<()> {
        let len = msg.get_size() + 1;
        let mut packet: Vec<u8> = Vec::with_capacity(5 + len);
        packet.push(kind);
        packet.extend_from_slice(&(len as u32).to_be_bytes());
        Writer::new(&mut packet).write_message(&msg).map_err(|e|
            io::Error::new(io::ErrorKind::Other, e))?;

        stream.write_all(&packet)
    }

    fn measure_latency(stream: &mut TelekeySocket) -> io::Result<i64> {
        let mut packet = vec![0; 5 + 8];
        let start = Utc::now().timestamp_nanos();
        stream.write_all(&[2, 0, 0, 0, 0])?;
        stream.read_exact(&mut packet)?;
        let end = Utc::now().timestamp_nanos();
        let middle = i64::from_be_bytes(packet[5..].try_into().unwrap());
        let d1 = middle - start;
        let d2 = end - middle;
        Ok((d1 + d2) / 2)
    }

    fn print_header(&self, stream: &TelekeySocket) -> String
    {
        let name = style(format!("TeleKey v{} ", self.version))
            .color256(173).italic();
        let Ok(peer_addr) = stream.get_ref().peer_addr() else {
            return format!("{}{}", name, style("!! Unkown peer !!").on_red());
        };
        let peer = if let Some(remote) = &self.remote {
            style(format!(" {} ({})", peer_addr, remote.hostname))
        } else {
            style(format!(" {} ", peer_addr))
        }.bg(console::Color::Color256(239)).fg(console::Color::Magenta);
        format!("{}{}", name, peer)
    }

    fn print_menu(&self, header: &str, latency: &str,
                  history: Option<&VecDeque<KeyEvent>>) {
        let state = match self.state {
            TelekeyState::Idle => style(" IDLE ").on_blue().black(),
            TelekeyState::Active => style(" ACTIVE ").on_green().black(),
        };

        println!("{}{}{}", header, state, latency);
        if let Some(hist) = history {
            for l in hist {
                println!("{}", l);
            }
        }
        println!("{}", style("--> Press any key <--").color256(246));
    }

    fn wait_for_input(&mut self, stream: &mut TelekeySocket) -> io::Result<()> {
        let header = self.print_header(stream);
        let term = Term::stdout();

        let nano = Self::measure_latency(stream)?;
        let mut latency = if let Ok(d) = Duration::nanoseconds(nano).to_std() {
            style(format!(" {:?} ", d)).yellow()
        } else {
            style(" ??ms ".to_string()).yellow()
        }.to_string();

        if self.config.update_screen {
            term.clear_screen()?;
            self.print_menu(&header, &latency, None);

            let mut l = 0;
            let mut history = VecDeque::with_capacity(20);
            loop {
                match self.state {
                    TelekeyState::Idle => {
                        if let Ok(_key) = term.read_key() {
                            self.state = TelekeyState::Active;
                        }
                    },
                    TelekeyState::Active => {
                        if let Ok(key) = term.read_key() {
                            let e: KeyEvent = key.into();
                            Self::send_packet(stream, 1, e.clone())?;
                            if history.len() == 20 {
                                history.pop_front();
                            }
                            history.push_back(e);
                        }
                    }
                }

                if let Some(period) = self.config.refresh_latency {
                    if l == period { // after x reads, measure latency
                        let nano = Self::measure_latency(stream)?;
                        latency = if let Ok(d) = Duration::nanoseconds(nano).to_std() {
                            style(format!(" {:?} ", d)).yellow()
                        } else {
                            style(" ??ms ".to_string()).yellow()
                        }.to_string();
                        l = 0;
                    } else {
                        l += 1;
                    }
                }

                term.clear_screen()?;
                self.print_menu(&header, &latency, Some(&history));
            }
        } else {
            self.print_menu(&header, &latency, None);

            loop {
                match self.state {
                    TelekeyState::Idle => {
                        if let Ok(_key) = term.read_key() {
                            self.state = TelekeyState::Active;
                            term.clear_last_lines(2)?;
                            self.print_menu(&header, &latency, None);
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
}
