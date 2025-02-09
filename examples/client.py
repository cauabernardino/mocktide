import socket

SERVER_HOST = '127.0.0.1'  # Server is running on localhost
SERVER_PORT = 6000        # Port to connect to

messages = [
    b"+some data here\r\n",
    b"+another data\r\n",
    b"-12341241\r\n"
]

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as client_socket:
    client_socket.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
    client_socket.connect((SERVER_HOST, SERVER_PORT))
    print(f"Connected to server at {SERVER_HOST}:{SERVER_PORT}")

    for msg in messages:
        client_socket.sendall(msg)
        print(f"Sent binary data: {msg}")

        response = client_socket.recv(1024)
        print(f"Received response: {response}")
