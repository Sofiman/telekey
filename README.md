# telekey
Telekey is a remote keyboard interface for working on two computers at once.
Pressed keys are transmitted using Protobuf over a TCP Connection (encryption WIP).

⚠️  __**Warning**__: Be careful while using this software in public places - no
encryption yet - !

## Requirements
- `libxdo-dev` for linux users.

# Usage
Subject to change.

Start a server on the input machine:
```shell
$ cargo run -- -s
```
This will automatically start a TCP Listener on *all* network interfaces (0.0.0.0)
on port **8384**.

To connect a client:
```shell
$ cargo run
Starting client as `[HOSTNAME]`
Enter address: [IP ADDRESS OF THE SERVER]:8384
```
To confirm the connection to the server, the token shown in the server console must be entered in the client console.

Available options:
| Option | Name     | Description                                                                                                    |
|--------|----------|----------------------------------------------------------------------------------------------------------------|
| -s     | Server   | Hosts a Telekey server.                                                                                        |
| -c     | Cold Run | If present, the client will not try to emulate the keyboard. All received keys will be printed in the console. |
| -r     | Raw      | If present, the Graphical User Interface will be limited to only the status bar (no history).                  |

# Todo
- [x] Graphical User Interface
- [ ] End-to-end encryption
- [ ] Command Line Interface
- [ ] Add missing keys
