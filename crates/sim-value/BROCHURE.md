# sim-value

In one line: A friendly toolkit for building and reading the small data shapes that flow through SIM.

## What it gives you

SIM moves information around as compact data forms -- numbers, words, lists, and named fields. On their own those forms are bare and awkward to work with. This crate gives you a tidy, consistent set of helpers for making them and reading them back: build a value, look up a field by name, replace one field while leaving its siblings untouched, or walk into a nested spot by a simple address. Everything is done by copying rather than altering shared state, so a change never surprises code that still holds the older value. One shared home means every part of SIM speaks the same small vocabulary instead of each corner inventing its own.

## Why you will be glad

- Reading and writing named fields stays short and clear instead of verbose.
- Updates leave untouched parts exactly as they were, so nothing gets clobbered.
- Every SIM library builds and inspects data the same way, so behavior matches across the system.

## Where it fits

This is one of the ground-floor pieces of SIM. The core defines the raw data shapes but no comfortable way to handle them; this crate supplies that comfort in a single place. It leans only on the core and adds convenience, not new runtime rules, so it stays out of protocol decisions. Dozens of higher libraries build on it to construct results, read requests, and hand data along the pipeline.
