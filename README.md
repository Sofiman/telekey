# TeleKey

TeleKey is a remote keyboard interface for working on two computers at once.
Pressed keys are encrypted and transmitted using Protobuf over a
TCP Connection.

## Features

- Connect two computers on the same local network
- End-to-end encryption using X25519 and ChaCha-Poly1035
- Cross platform (tested on Windows, macOS and Ubuntu)


## Quick Start

Syntax:
```bash
$ telekey.exe [OPTIONS]
```

### 📝 Start as client
A Telekey Client will receive key events and emulate the key presses.

```bash
$ telekey.exe -t 127.0.0.1
```
By default (without any options), TeleKey will try to connect to **127.0.0.1:8384**.
And all received keys will be emulated on the system (real key presses).

### 🧑‍💻 Start as server
Telekey Server will listen and send key events to the Telekey Client.

```bash
$ telekey.exe -s 0.0.0.0
```
By default (without any options), TeleKey will send all keys typed in the stdin of
the program.

## Options

Option Syntax follows the Unix-standard. Combined options and equal-seperated options are accepted.
| Usage                       | Description                                                                                                    | Default        |
|-----------------------------|----------------------------------------------------------------------------------------------------------------|----------------|
| -t, --target-ip <IP[:PORT]> | [Runs telekey as client] Defines the target address to connect to                                              | 127.0.0.1:8384 |
| -s, --serve <IP[:PORT]>     | [Runs telekey as server] TCP port to listen to                                                                 | 0.0.0.0:8384   |
| -m, --simple-menu           | If enabled, server's menu will only show minimal information and only update latency                           | `false`        |
| -c, --cold-run              | If enabled, the key presses will be printed to the standard output rather than being emulated                  | `false`        |
| -l, --refresh-latency <n>   | Triggers a latency check after `n` keys. Use **0** to disable latency checks.                                  | 20             |
| -u, --unsecure              | Unsecure mode. No encryption: use it at your own risk!                                                         | `false`        |
| -h, --help.                 | Print help version (list of options and usage)                                                                 | N/A            |
| -v, --version               | Print version information                                                                                      | N/A            |


## Installation

### Requirements
- Rust version 1.60 minimum (not tested on older versions)

```bash
  cargo build
```
    
## Todo

- [x] Graphical User Interface
- [x] End-to-end encryption
- [x] Improve End-to-end encryption to prevent key-dictating (+man in the middle) attacks
- [x] Command Line Interface
- [ ] Add missing keys


## Contribution & Feedback

If you have any feedback, please open an issue.
If you encounter any bugs or unwanted behaviour, please open an issue.

This projet is open to contributions, feel free to submit your pull requests!


## License

[GNU General Public License v3.0](https://choosealicense.com/licenses/gpl-3.0/)

