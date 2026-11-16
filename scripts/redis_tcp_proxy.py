#!/usr/bin/env python3
"""Minimal TCP proxy used only by scripts/test_distributed_invalidation.sh.

It lets the harness sever a replica's Redis connection without stopping Redis,
so the durable stream backlog can be replayed when the replica reconnects.
"""

import argparse
import socket
import threading


def pipe(src: socket.socket, dst: socket.socket) -> None:
    try:
        while True:
            data = src.recv(65536)
            if not data:
                break
            dst.sendall(data)
    except OSError:
        pass
    finally:
        for sock in (src, dst):
            try:
                sock.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            sock.close()


def handle(client: socket.socket, target_port: int) -> None:
    try:
        upstream = socket.create_connection(("127.0.0.1", target_port), timeout=5)
        upstream.settimeout(None)
    except OSError:
        client.close()
        return
    client.settimeout(None)
    threading.Thread(target=pipe, args=(client, upstream), daemon=True).start()
    pipe(upstream, client)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("listen_port", type=int)
    parser.add_argument("target_port", type=int)
    args = parser.parse_args()

    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", args.listen_port))
    server.listen(128)
    while True:
        client, _ = server.accept()
        threading.Thread(
            target=handle, args=(client, args.target_port), daemon=True
        ).start()


if __name__ == "__main__":
    main()
