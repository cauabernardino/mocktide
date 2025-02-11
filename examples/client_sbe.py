"""Simulating SBE logon req/ack"""
import socket
import time

SERVER_HOST = '127.0.0.1'
SERVER_PORT = 6020

login = bytes(b"\x14\x00\x00\x00\x02\x00\x01\x00\x00\x00user1\x00pass1\x00\x39\x30\x00\x00")
heartbeat = bytes(b"\x08\x00\x00\x00\x01\x00\x01\x00\x00\x00")
login_ack = bytes(b"\x0C\x00\x00\x00\x03\x00\x01\x00\x00\x00\x01\x00\x39\x30\x00\x00")


with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as client_socket:
    client_socket.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
    client_socket.connect((SERVER_HOST, SERVER_PORT))
    print(f"connected to server at {SERVER_HOST}:{SERVER_PORT}")


    client_socket.sendall(login)
    print(f"send: {login}")

    time.sleep(0.2)
    response = client_socket.recv(1024)

    if response == login_ack:
        print(f"recv: {response}")
    else:
        print(f"wrong response recv: {response}")
        exit()

    for _ in range(3):
        time.sleep(2)
        client_socket.sendall(heartbeat)
        print(f"send: {heartbeat}")
