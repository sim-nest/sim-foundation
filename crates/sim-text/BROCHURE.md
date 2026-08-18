# sim-text

In one line: One lossless home for exact text that may contain code units which ordinary Unicode strings cannot represent.

## What it gives you

Some language and file formats must preserve UTF-16 code units exactly,
including lone surrogates and NUL. `sim-text` gives those formats a shared,
dependency-light value instead of making every runtime or codec invent its own.

## Why you will be glad

- JavaScript, codecs, and the JVM can exchange exact text without data loss.
- Conversion to ordinary scalar text is an explicit checked boundary.
- A neutral foundation owner prevents guest-language implementations from
  depending on one another.

## Where it fits

The crate sits below codecs and runtimes in `sim-foundation`. It carries text
representation, not language policy, object identity, interning, or encoding
rules.
