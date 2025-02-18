# mocktide 🌊
A configurable mock server for testing network clients.

**This is a working in progress!**

Main workflow:
- Create a mapping file (example in [Mapping File](#mapping-file))
- Start the server pointing to your mapping file
  - You can optionally set different host, port and report file path
- Run your client binding to server port
  - Server will validate expected received messages
- Check server logs and result file
  - Result file is in JUnit XML format,
    - The name in mapping file is considered the test suite
    - Each `Recv` action is a test case


## How to run 
_For now, still not released_

Simplest usage example, of a SBE logon req/ack:
```bash
# For checking options
$ cargo run -- --help

# Run the server
$ cargo run -- -v  examples/sbe.yaml 

# Optionally, Run the server, setting the output file in a different path
$ cargo run -- -v -r path/to/test.xml examples/sbe.yaml 

# In another terminal
$ python3 examples/client_sbe.py
```

## Mapping file
Mapping file basic structure
```yaml
# Name for the test
name: My test

# Mapping of messages to be handled by the server, either to be sent or received
# name: value (text or binary repr)
messages:
  first_msg: "\x01\x02\x03\x04"
  second_msg: "\x04\x03\x02\x01"
  third_msg: "\x01\x00\x00\x01"

# Sequence of server actions (Send, Recv, Shutdown) to be executed
actions:
  - execute: Recv
    message: first_msg
  - execute: Send
    message: second_msg
    wait_for: 2  # You can set a waiting time for an action, in seconds
  - execute: Recv
    message: third_msg
  - execute: Recv
    message: third_msg
  - execute: Shutdown
```

Actions:
  - Send => server will send the mapped message
  - Recv => server will wait for the mapped message and validate it
  - Shutdown => server will shutdown, will not require a mapped message

## Roadmap

- [x]  TCP server
- [x]  CLI
- [x]  Tests (First test setup and CI)
- [x]  Sleep/wait logic
- [x]  Results output
- [ ]  Tests and documentation for outputs
- [ ]  UDP server
- [ ]  Better examples
