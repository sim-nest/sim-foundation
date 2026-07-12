# sim-config

In one line: layered SIM settings stay inspectable because every source becomes the same table-shaped data before it is merged.

## What it gives you

`sim-config` gives SIM a common base for configuration without inventing a second settings world. Built-in defaults, probed defaults, home files, working-directory files, and explicit overrides all land in one directory of library tables. The merge result carries where each effective field came from, so a loader, command line, report, or agent can show why the current setting has that value.

## Why you will be glad

Configuration becomes easier to trust. A user can keep one file per library or one combined file, and both paths feed the same shape. A builder can add a new source without changing how the rest of SIM reads settings. When something surprising happens, provenance points to the layer that won instead of leaving the answer buried in loader order.

## Where it fits

This crate sits in the foundation layer. Codecs turn text into tables, loaders decide which sources to read, probes produce modeled or real defaults, and runtime libraries consume typed views over the final tables. `sim-config` is the shared substrate those pieces meet on.
