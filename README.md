# telekey
Telekey is a remote keyboard interface for working on multiple computers at once.

## Requirements
- `libxdo-dev` for linux users.

# Usage
Subject to change.


Start a server on the input machine:
```shell
$ cargo run -- -s
```
This will automatically start a TCP Listenner on port **8384**

To connect a client:
```shell
$ cargo run
Starting client as `[HOSTNAME]`
Enter address: [IP ADDRESS OF THE SERVER]:8384
```
To confirm the connection to the server, the token shown in the server console must be entered in the client console.

# Todo
- Encryption
- CLI
- Interface
