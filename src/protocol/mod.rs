pub mod bindings;
pub mod transport;
use crate::protocol::bindings::api::*;
use crate::transport::*;
use chrono::{Utc, Duration};
use enigo::{Enigo, KeyboardControllable};
use console::{Term, style};
use std::{io::{self, Write, Error, ErrorKind}, net::*, borrow::Cow};
use std::collections::VecDeque;
use rand::{distributions::Alphanumeric, Rng};
//use orion::{kex::*, aead::*};
use quick_protobuf::deserialize_from_slice;

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
    cold_run: bool,
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

impl From<HandshakeRequest<'_>> for TelekeyPacket {
    fn from(p: HandshakeRequest<'_>) -> Self {
        Self::new(TelekeyPacketKind::Handshake, p)
    }
}

impl From<HandshakeResponse<'_>> for TelekeyPacket {
    fn from(p: HandshakeResponse<'_>) -> Self {
        Self::new(TelekeyPacketKind::Handshake, p)
    }
}

impl From<KeyEvent> for TelekeyPacket {
    fn from(p: KeyEvent) -> Self {
        Self::new(TelekeyPacketKind::KeyEvent, p)
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

impl From<&KeyEvent> for Result<enigo::Key, String> {
    fn from(e: &KeyEvent) -> Self {
        use KeyKind::*;
        match e.kind {
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
            CHAR => Ok(enigo::Key::Layout(char::from_u32(e.key).unwrap())),
            PAGEUP => Ok(enigo::Key::PageUp),
            PAGEDOWN => Ok(enigo::Key::PageDown),
            SHIFT => Ok(enigo::Key::Shift),
            META => Ok(enigo::Key::Meta),
            _ => Err(format!("From<KeyEvent> => enigo::Key for {:?}", e))
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
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr)?;
        println!("Server listenning on {} as `{}`", addr, config.hostname);

        // accept connections and process them serially
        for stream in listener.incoming().flatten() {
            let secret: Vec<u8> = rand::thread_rng()
                .sample_iter(&Alphanumeric).take(8).collect();
            println!("Enter this token to confirm: {}",
                     String::from_utf8(secret.clone()).unwrap());
            let mut telekey = Telekey {
                config: config.clone(), mode: TelekeyMode::Server(port),
                version: 1, session_id: Some(secret), remote: None,
                state: TelekeyState::Idle, enigo: Enigo::new()
            };
            let stream: TcpTransport = stream.into();
            //let session = EphemeralServerSession::new()?;
            let mut stream = telekey.handshake(stream/*,
                (&session.public_key(), &session.private_key())*/)?;
            if let Err(e) = telekey.wait_for_input(&mut stream) {
                println!("{}: {}", style("ERROR").red().bold(), e);
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
                println!("{} connected to the server!",
                    style("Successfully").green().bold());
                let stream: TcpTransport = stream.into();
                //let session = EphemeralClientSession::new()?;
                let stream = telekey.handshake(stream/*,
                    (&session.public_key(), &session.private_key())*/)?;

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

    /*
    fn sec_handshake<T: TelekeyTransport>(&self, tr: &mut T, kp: (&PublicKey, &PrivateKey)) -> io::Result<KexTransport> {
        todo!()
    }*/

    fn handshake(&mut self, mut tr: TcpTransport) -> io::Result<TcpTransport> {
        let Ok(target_ip) = tr.peer_addr() else {
            tr.shutdown()?;
            return Err(Error::new(ErrorKind::NotConnected, "Unknown peer"));
        };
        if matches!(self.mode, TelekeyMode::Server(_)) {
            let expected = self.session_id.as_ref()
                .expect("Server must have a valid session ID");
            let p = tr.recv_packet()?;
            let msg: HandshakeRequest = deserialize_from_slice(p.data())
                .expect("Cannot read HandshakeRequest message");
            let token: &[u8] = &msg.token;
            if expected != token {
                tr.shutdown()?;
                return Err(Error::new(ErrorKind::NotConnected,
                        "Invalid secret"));
            }
            tr.send_packet(HandshakeResponse {
                hostname: Cow::Borrowed(&self.config.hostname),
                version: self.version,
                pkey: Cow::Borrowed(&[])
            }.into())?;
            self.remote = Some(msg.into());

            Ok(tr)
        } else {
            let mut inp = String::new();
            print!("Please enter token to continue: ");
            io::stdout().flush()?;
            io::stdin().read_line(&mut inp)?;
            let bytes = inp.trim().as_bytes();
            if bytes.len() != 8 {
                return Err(Error::new(ErrorKind::Other, "Invalid token"));
            }

            let p = HandshakeRequest {
                hostname: Cow::Borrowed(&self.config.hostname),
                version: self.version,
                token: Cow::Borrowed(bytes),
                pkey: Cow::Borrowed(&[])
            };
            tr.send_packet(p.into())?;

            println!("Waiting for response");
            let p = tr.recv_packet()?;
            println!("Got response");
            let msg: HandshakeResponse = deserialize_from_slice(p.data())
                .expect("Cannot read HandshakeResponse message");
            self.remote = Some(TelekeyRemote {
                hostname: msg.hostname.to_string(),
                version: msg.version,
                mode: TelekeyMode::Server(target_ip.port()),
            });
            println!("{}{}", self.print_header(Some(target_ip)),
                style(" ACTIVE ").on_green().black());
            Ok(tr)
        }
    }

    fn listen_loop<T: TelekeyTransport>(&mut self, mut tr: T) -> io::Result<()> {
        loop {
            let p = tr.recv_packet()?;
            self.handle_packet(&mut tr, p)?;
        }
    }

    fn handle_packet<T: TelekeyTransport>(&mut self, tr: &mut T, p: TelekeyPacket)
        -> io::Result<()> {
        match p.kind() {
            TelekeyPacketKind::Handshake => unreachable!("Handshake should no be sent at this point"),
            TelekeyPacketKind::KeyEvent => {
                if self.remote.is_none() {
                    return tr.shutdown();
                }
                if !self.is_server() {
                    let msg: KeyEvent = deserialize_from_slice(p.data())
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
            TelekeyPacketKind::Ping => {
                let tm = Utc::now().timestamp_nanos();
                let len = std::mem::size_of::<i64>() as u32;
                let buf: Vec<u8> = tm.to_be_bytes().to_vec();
                tr.send_packet(TelekeyPacket::raw(TelekeyPacketKind::Ping, len, buf))
            }
            k => {
                println!("{}: Unknown packet {:?}",
                     style("RUNTIME ERROR").yellow().bold(), k);
                Ok(())
            }
        }
    }

    fn measure_latency<T: TelekeyTransport>(tr: &mut T) -> io::Result<i64> {
        let start = Utc::now().timestamp_nanos();
        tr.send_packet(TelekeyPacket::raw(TelekeyPacketKind::Ping, 0,
                Vec::with_capacity(1)))?;
        let p = tr.recv_packet()?;
        match p.kind() {
            TelekeyPacketKind::Ping => {
                let end = Utc::now().timestamp_nanos();
                let middle = i64::from_be_bytes(p.data().try_into().unwrap());
                let d1 = middle - start;
                let d2 = end - middle;
                Ok((d1 + d2) / 2)
            },
            k => {
                let err = format!("Expected ping packet received {:?}", k);
                Err(Error::new(ErrorKind::InvalidData, err))
            }
        }
    }

    fn print_header(&self, peer_addr: Option<SocketAddr>) -> String
    {
        let name = style(format!("TeleKey v{} ", self.version))
            .color256(173).italic();
        let Some(peer_addr) = peer_addr else {
            return format!("{}{}", name, style("!! Unkown peer !!").on_red());
        };
        let peer = if let Some(remote) = &self.remote {
            style(format!(" {} ({}) ", peer_addr, remote.hostname))
        } else {
            style(format!(" {} ", peer_addr))
        }.bg(console::Color::Color256(238)).fg(console::Color::Magenta);
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

    fn wait_for_input<T: TelekeyTransport>(&mut self, tr: &mut T) -> io::Result<()> {
        let header = self.print_header(tr.peer_addr().ok());
        let term = Term::stdout();

        let nano = Self::measure_latency(tr)?;
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
                            let p: TelekeyPacket = e.clone().into();
                            tr.send_packet(p)?;
                            if history.len() == 20 {
                                history.pop_front();
                            }
                            history.push_back(e);
                        }
                    }
                }

                if let Some(period) = self.config.refresh_latency {
                    if l == period { // after x reads, measure latency
                        let nano = Self::measure_latency(tr)?;
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

            let mut l = 0;
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
                            let e: TelekeyPacket = e.into();
                            tr.send_packet(e)?;
                        }
                    }
                }

                if let Some(period) = self.config.refresh_latency {
                    if l == period { // after x reads, measure latency
                        let nano = Self::measure_latency(tr)?;
                        latency = if let Ok(d) = Duration::nanoseconds(nano).to_std() {
                            style(format!(" {:?} ", d)).yellow()
                        } else {
                            style(" ??ms ".to_string()).yellow()
                        }.to_string();
                        term.clear_last_lines(2)?;
                        self.print_menu(&header, &latency, None);
                        l = 0;
                    } else {
                        l += 1;
                    }
                }
            }
        }
    }
}
