"""Simulating a dummy protocol exchange"""
import socket

SERVER_HOST = '127.0.0.1'
SERVER_PORT = 6020

send_messages = [
    bytes(b"+hello world"),
    bytes(b"-123neg"),
]

recv_messages = [
    bytes(b"+some data"),
]

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as client_socket:
    client_socket.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
    client_socket.connect((SERVER_HOST, SERVER_PORT))
    print(f"Connected to server at {SERVER_HOST}:{SERVER_PORT}")


    for msg in send_messages:
        client_socket.sendall(msg)
        print(f"send: {msg}")

    for msg in recv_messages:
        response = client_socket.recv(1024)

        if msg == response:
            print(f"recv: {response}")
        else:
            print(f"wrong response recv: {response}")
