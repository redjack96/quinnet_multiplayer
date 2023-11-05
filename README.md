# quinnet_multiplayer
Example of use of client-server bevy 0.12.0 chat app with quinnet crate, using the QUIC internet protocol.

Capabilities:
- The server sends message in broadcast to each client.
- When a client writes a messages, it communicates with the server which forwards the message to all other clients.
- When a client connects, the server informs all other client that a new user has joined.
- When a client disconnects, the server waits until a timeout, then informs all other clients of the disconnection.

Useful as a baseline to make client-server multiplayer games.
