# mocktide 🌊
A configurable mock server for testing network clients.

**This is a working in progress!**

## How to run 
_For now, still not released_

Simplest usage example, of a SBE logon req/ack:
```bash
# In one terminal, check options
$ cargo run -- --help

# Run the server
$ cargo run -- -v examples/sbe.yaml

# In another terminal
$ python3 examples/client_sbe.py
```

## Mapping file
Mapping file basic structure
```yaml
# Mapping of messages to be handled by the server, either to be sent or received
# name: value (text or binary repr)
messages:
  first_msg: "\x01\x02\x03\x04"
  second_msg: "\x04\x03\x02\x01"
  third_msg: "\x01\x00\x00\x01"

# Sequence of server actions (Send or Recv a msg), binding a message to an action
actions:
  - message: first_msg
    action: Recv
  - message: second_msg
    action: Send
  - message: third_msg
    action: Recv
  - message: third_msg
    action: Recv
  - action: Shutdown
```

Current actions:
  - Send => server will the the mapped message
  - Recv => server will wait for the mapped message
  - Shutdown => server will shutdown, does not will require a mapped message

## TODOs

- [x]  TCP server
- [x]  CLI
- [x]  Tests (First test setup and CI)
- [ ]  Sleep/wait logic
- [ ]  Results output
- [ ]  UDP server
- [ ]  Better examples
