mod protocol;
use crate::protocol::*;
use std::{net::{SocketAddr, IpAddr}, str::FromStr};
use anyhow::{Result, Context, bail};
use tui_markup_ansi_macro::ansi;

const HELP: &str = ansi!("<brown TeleKey> by Sofiane Meftah
Secure remote keyboard interface over TCP.

<u Usage:> telekey.exe <yellow [OPTIONS...]>

<u Options:>
  -t, --target-ip \\<<arg IP<opt [:PORT]>>\\>  <green [Runs telekey as client]> Defines the target address to connect to. <def defaults to 127.0.0.1:8384>
  -s, --serve \\<<arg IP<opt [:PORT]>>\\>      <green [Runs telekey as server]> IP address to start a TCP Listener on. <def defaults to 0.0.0.0:8384>
  -m, --simple-menu            If enabled, server's menu will only show minimal information and only update latency.
  -c, --cold-run               If enabled, the key presses will be printed to the standard output rather than being emulated.
  -l, --refresh-latency \\<<arg N>\\>    Triggers a latency check after <arg N> keys. Use 0 to disable latency checks. <def defaults to 20>
  -u, --unsecure               <red Unsecure mode.> <i No encryption: use it at your own risk!>
  -h, --help                   Print help information.
  -v, --version                Print version information.",
  "brown" => "173",
  "arg" => "cyan",
  "opt" => "blue,d",
  "def" => "magenta,i"
);

fn parse_ip(s: &str) -> Result<SocketAddr> {
    if let Ok(addr) = SocketAddr::from_str(s) {
        return Ok(addr)
    }
    let addr = IpAddr::from_str(s)?;
    Ok(SocketAddr::new(addr, 8384))
}

fn parse_args() -> Result<(SocketAddr, TelekeyMode, TelekeyConfig)> {
    use lexopt::prelude::*;

    let mut config = TelekeyConfig::default();
    let mut target_ip: Option<SocketAddr> = None;
    let mut bind: Option<SocketAddr> = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('s') | Long("serve") => {
                let ip: String = parser.value()?.parse()?;
                bind = Some(parse_ip(&ip)
                     .context("Invalid IP address to bind")?);
            }
            Short('t') | Long("target-ip") => {
                let ip: String = parser.value()?.parse()?;
                target_ip = Some(parse_ip(&ip)
                     .context("Invalid target IP address")?);
            }
            Short('m') | Long("simple-menu") => config.set_update_screen(false),
            Short('c') | Long("cold-run") => config.set_cold_run(true),
            Short('u') | Long("unsecure") => config.set_secure(false),
            Short('l') | Long("refresh-latency") => {
                let n: usize = parser.value()?.parse()?;
                config.set_refresh_latency(if n == 0 { None } else { Some(n) });
            }
            Short('v') | Long("version") => {
                println!("TeleKey {} by Sofiane Meftah",
                    VERSION.unwrap_or("Unknown"));
                std::process::exit(0);
            }
            Short('h') | Long("help") => {
                println!("{}", HELP);
                std::process::exit(0);
            }
            _ => bail!(arg.unexpected()),
        }
    }

    if let Some(addr) = bind {
        Ok((addr, TelekeyMode::Server, config))
    } else {
        let addr = target_ip.unwrap_or_else(||
            SocketAddr::from(([127, 0, 0, 1], 8384)));
        Ok((addr, TelekeyMode::Client, config))
    }
}

fn main() -> Result<()> {
    use TelekeyMode::*;
    let (addr, mode, config) = parse_args()?;
    match mode {
        Client => Telekey::connect_to(addr, config),
        Server => Telekey::serve(addr, config)
    }
}
