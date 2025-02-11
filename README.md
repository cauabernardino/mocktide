# mocktide
A file configurable mock server for testing network clients.

**This is a working in progress!**

Simplest usage example, of a SBE logon req/ack:
```bash
# In one terminal, check options
$ cargo run -- --help

# Run the server
$ cargo run -- -v examples/sbe.yaml

# In another terminal
$ python3 examples/client_sbe.py
```

## TODOs

- [x]  TCP server
- [x]  CLI
- [ ]  Tests
- [ ]  Sleep/wait logic
- [ ]  Results output
- [ ]  UDP server
- [ ]  Better examples
