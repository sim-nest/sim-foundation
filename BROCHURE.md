# sim-foundation

In one line: the shared groundwork the rest of SIM stands on -- values, tables, macros, and the runnable lessons that teach them.

## What it gives you

A friendly toolkit for building and reading the small data shapes that flow through SIM, the shared rulebook for naming and requesting table data, the labels that let an author declare SIM building blocks in plain Rust, a clean name-translator that presents SIM's tools to outside systems, and the engine behind SIM's built-in, runnable lessons.

## Why you will be glad

The foundation layer keeps common decisions in one trusted place. Library authors get the same data helpers, table rules, config model, recipe engine, and declaration labels, so they spend less time rebuilding substrate and more time describing behavior. Users get a system whose pieces explain themselves consistently across command-line, web, help, and assistant surfaces.

## Where it fits

This repo sits directly above the SIM kernel and below the behavior libraries. The kernel defines the core contracts; sim-foundation makes those contracts practical to use without adding policy, devices, network effects, or application behavior. Higher libraries build on it for values, settings, table requests, authoring labels, lessons, network framing, and surface names.
