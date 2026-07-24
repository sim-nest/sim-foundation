# sim-table-core

In one line: The shared rulebook for naming and requesting table data across SIM.

## What it gives you

Many parts of SIM store and fetch data arranged as tables, and they need to agree on three things: which names are allowed for a location, how relative references resolve, and how a request to read or change a table is spelled out on the wire. This crate is the single home for those rules. It checks that a path segment is a real, safe name -- not empty, not a sneaky parent escape, not carrying a stray separator -- and it resolves absolute and relative references into one canonical path. It also describes each table request in one agreed form and translates it to and from SIM data using the exact spellings the remote backends already use, so a local store and a distant one understand each other without guesswork.

## Why you will be glad

- Unsafe or malformed table names are caught early instead of causing trouble later.
- Relative and absolute table references resolve the same way in every backend.
- Local and remote table backends speak one request language, so they interoperate cleanly.
- One tested definition covers the shared checks each backend needs.

## Where it fits

This sits underneath SIM's table features as shared connective tissue. It holds no storage engine and does no input or output of its own; it only settles the naming rules and the request shape that every table backend depends on. Leaning solely on the core and the value helpers, it keeps table libraries consistent with each other and with the wider system.
