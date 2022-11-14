pub mod bindings;
use crate::protocol::bindings::api::*;
use chrono::{Utc, Duration};
use console::{Term, style};
use enigo::{Enigo, KeyboardControllable};
use std::{io::{self, Read, Write, Error, ErrorKind}, net::*, borrow::Cow, collections::VecDeque};
use rand::{distributions::Alphanumeric, Rng};
use quick_protobuf::{Writer, MessageWrite, deserialize_from_slice};

const LATENCY_UPDATE_PERIOD: Option<usize> = Some(20);

#[derive(Clone, Debug, Copy)]
pub enum TelekeyMode {
    Client,
    Server(u16)
}

#[derive(Clone, Debug)]
pub struct TelekeyConfig {
    hostname: String,
    version: u32,
    mode: TelekeyMode,
    update_screen: bool,
    cold_run: bool
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

    pub fn set_update_screen(&mut self, update_screen: bool) {
        self.update_screen = update_screen;
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
            version: 1,
            mode: TelekeyMode::Client,
            update_screen: true,
            cold_run: false
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
            mode: TelekeyMode::Client,
            update_screen: true,
            cold_run: false
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

impl From<KeyEvent> for enigo::Key {
    fn from(e: KeyEvent) -> Self {
        use KeyKind::*;
        match e.kind {
            CHAR => Self::Layout(char::from_u32(e.key).unwrap()),
            _ => todo!("todo: press keys")
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
            KeyKind::BACKSPACE => write!(f, "[BAK]"),
            KeyKind::INSERT => write!(f, "[INS]"),
            KeyKind::CHAR => write!(f, "{}", char::from_u32(self.key).unwrap()),
            KeyKind::TAB => write!(f, "\\t"),
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
    state: TelekeyState,
    enigo: Enigo
}

impl Telekey {
    pub fn new(conf: TelekeyConfig) -> Self {
        let remote = TelekeyRemote { me: conf, secret: None, remote: None };
        Telekey { remote, state: TelekeyState::Idle, enigo: Enigo::new() }
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
                state: TelekeyState::Idle,
                enigo: Enigo::new()
            };
            if let Err(e) = telekey.listen_loop(stream?) {
                println!("{}: {}", style("ERROR").red().bold(), e);
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
                    println!("{}: {}", style("ERROR").red().bold(), e);
                }

                io::Result::Ok(())
            },
            Err(e) => {
                println!("{}: Couldn't connect to server...",
                         style("ERROR").red().bold());
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
        -> io::Result<()> {
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
                    Self::send_packet(stream, 0, HandshakeResponse {
                        hostname: Cow::Borrowed(&self.remote.me.hostname),
                        version: self.remote.me.version
                    })?;
                    self.remote.put_remote(msg.into());

                    self.wait_for_input(stream)?;
                } else {
                    let msg: HandshakeResponse = deserialize_from_slice(&buf)
                        .expect("Cannot read HandshakeResponse message");
                    println!("{} {}", self.print_header(stream),
                        style(msg.hostname.to_string()).cyan());
                    self.remote.put_remote(TelekeyConfig {
                        hostname: msg.hostname.to_string(),
                        version: msg.version,
                        mode: TelekeyMode::Server(target_ip.port()),
                        update_screen: true,
                        cold_run: false
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

                    if self.remote.me.cold_run {
                        print!("{}", msg);
                        io::stdout().flush()?;
                    } else {
                        self.enigo.key_down(msg.into());
                    }
                }
            },
            2 => {
                let tm = Utc::now().timestamp_nanos();
                let mut buf: Vec<u8> = Vec::with_capacity(5 + 8);
                buf.push(2);
                buf.extend_from_slice(&(16u32).to_be_bytes());
                buf.extend_from_slice(&tm.to_be_bytes());
                stream.write_all(&buf)?;
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

    fn measure_latency(stream: &mut TcpStream) -> io::Result<i64> {
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

    fn print_header(&self, stream: &TcpStream) -> String
    {
        let name = style(format!("TeleKey v{} ", self.remote.me.version))
            .color256(173).italic();
        let peer = style(format!(" {} ", stream.peer_addr().unwrap()))
            .bg(console::Color::Color256(239)).fg(console::Color::Magenta);
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

    fn wait_for_input(&mut self, stream: &mut TcpStream) -> io::Result<()> {
        let header = self.print_header(stream);
        let term = Term::stdout();

        let nano = Self::measure_latency(stream)?;
        let mut latency = if let Ok(d) = Duration::nanoseconds(nano).to_std() {
            style(format!(" {:?} ", d)).yellow()
        } else {
            style(" ??ms ".to_string()).yellow()
        }.to_string();


        if self.remote.me.update_screen {
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

                if let Some(period) = LATENCY_UPDATE_PERIOD {
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
