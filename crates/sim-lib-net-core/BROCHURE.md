# sim-lib-net-core

In one line: The quiet reading room that turns raw web traffic into clean, understandable pieces.

## What it gives you

When SIM talks to web services, the bytes arriving over the wire are messy: addresses to pick apart, response headers to read, a body whose length is signalled in one of several ways, and streams that dribble in one line or one record at a time. This crate does that careful reading and nothing else. It splits addresses, parses the head of a response, works out how the body is delivered, frames incoming lines even when a line is cut across two arrivals, and decodes the common streaming formats that live services push. It touches no socket, opens no connection, and makes no decisions about what a message means; the caller keeps full control of the network and the meaning, while this crate handles the fiddly framing correctly and the same way every time.

## Why you will be glad

- The awkward corners of web wire formats are parsed for you, correctly and consistently.
- A single tested reader keeps framing behavior consistent across libraries.
- Because it does no networking itself, it is easy to test and simple to reason about.

## Where it fits

This is a small, self-contained leaf that SIM's network-facing libraries share. Any lib talking to a web service reads the wire through the same trusted code, while transport and interpretation stay in the layers above it.
