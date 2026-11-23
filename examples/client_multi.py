"""Simulating several clients against one server, reported in a single file.

Start the server with examples/multi.yaml, then run, for example:
    python3 examples/client_multi.py                  # 3 clients, all pass
    python3 examples/client_multi.py -n 5 --wrong 2   # client 2 sends a bad PING
"""
import argparse
import socket
import threading

SERVER_HOST = '127.0.0.1'
SERVER_PORT = 6020


def session(client: int, sock: socket.socket, wrong_ping: bool) -> None:
    ping = b"PIGN\n" if wrong_ping else b"PING\n"

    with sock, sock.makefile("rb") as replies:
        for request, expected in [(b"HELLO\n", b"WELCOME\n"), (ping, b"PONG\n")]:
            sock.sendall(request)
            response = replies.readline()

            if response != expected:
                print(f"client {client}: sent {request!r}, got {response!r}")
                return

        sock.sendall(b"BYE\n")
        print(f"client {client}: session completed")


parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
parser.add_argument("-n", "--clients", type=int, default=3)
parser.add_argument("--wrong", type=int, metavar="N",
                    help="client number (1-based) that sends a wrong PING")
args = parser.parse_args()

# Connect one by one so client N is the server's connection N in the report,
# then run all sessions at the same time.
sockets = [socket.create_connection((SERVER_HOST, SERVER_PORT)) for _ in range(args.clients)]
print(f"connected {args.clients} clients to {SERVER_HOST}:{SERVER_PORT}")

threads = [
    threading.Thread(target=session, args=(client, sock, client == args.wrong))
    for client, sock in enumerate(sockets, start=1)
]
for thread in threads:
    thread.start()
for thread in threads:
    thread.join()
