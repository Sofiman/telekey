mod protocol;
use crate::protocol::*;
use std::{io::{self, Write}, env};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let (conf, mode) = parse_args(args);
    match mode {
        TelekeyMode::Server(port) => Telekey::serve(port, conf),
        TelekeyMode::Client => {
            println!("Starting client as `{}`", conf.hostname());

            let mut inp = String::new();
            print!("Enter address: ");
            io::stdout().flush()?;
            io::stdin().read_line(&mut inp)?;
            let addr = inp.trim().parse().expect("Invalid address");

            Telekey::connect_to(addr, conf)
        }
    }
}

fn parse_args(args: Vec<String>) -> (TelekeyConfig, TelekeyMode) {
    let mut mode = TelekeyMode::Client;
    let mut conf = TelekeyConfig::default();
    // TODO: Add options with variables (ex: `-opt val -other-opt [...]`
    for arg in args {
        if arg.starts_with('-') {
            for c in arg.chars().skip(1) {
                match c {
                    's' => { mode = TelekeyMode::Server(8384) },
                    'r' => { conf.set_update_screen(false) } // raw display
                    'c' => { conf.set_cold_run(true) },
                     _ => ()
                }
            }
        }
    }
    (conf, mode)
}
