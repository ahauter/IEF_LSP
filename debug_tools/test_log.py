import socket as sock

s = sock.socket(family=sock.AF_UNIX)
s.connect("./logs/debug.socket")
s.send(b"hello")
