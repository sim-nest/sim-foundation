# sim-macros

In one line: A set of labels that let an author declare SIM building blocks in plain Rust and have the wiring written for them.

## What it gives you

Adding a new capability to SIM normally means writing a pile of repetitive registration code so the runtime can find and call it. This crate lets an author skip that chore. You mark your items with simple labels -- this is a class, this is a function, this is a constructor, this is a codec, and so on -- and then point one wrapper at the module holding them. The wrapper reads the labels, checks that each declaration is complete and consistent, and generates the connecting code and, when needed, the native bridge for other languages. Mistakes such as a malformed shape or a class missing its constructor are refused right when the code is built, so a broken contract never slips through quietly.

## Why you will be glad

- Declaring a new class, function, or codec stays short and readable.
- The tedious registration wiring is generated for you instead of hand-copied.
- Incomplete or inconsistent declarations are rejected at build time, before they can misbehave.

## Where it fits

This is the authoring convenience that most SIM libraries rest on. A library writer describes what a module offers in ordinary Rust with these labels, and the crate turns that description into the registration the runtime expects. It shapes how new behavior enters SIM and keeps every library declaring itself the same way, which makes the whole collection easier to read and to trust.
