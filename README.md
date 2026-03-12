# Text-Voice-Chat

A monolithic client-server chat application written in Rust.

- **Text chat** over TCP with length-prefixed binary framing
- **Voice chat** over UDP with Opus-encoded audio packets

## Architecture

```
         TCP :8080              TCP :8080
Client A ◄──────────► Server ◄──────────► Client B
         UDP :8081              UDP :8081
```

- Single central server, all clients connect directly
- TCP handles authentication, text messages, and signaling
- UDP handles real-time voice packet forwarding (server relays, does not decode)
- SQLite persists users and message history

## Project Structure

```
common/   — Shared message protocol and TCP framing utilities
server/   — TCP/UDP server, database layer, connection management
client/   — TCP/UDP client, terminal UI, audio capture/playback
```
