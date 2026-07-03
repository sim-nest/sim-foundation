# sim-cookbook

In one line: The engine behind SIM's built-in, runnable lessons that teach each library from the inside.

## What it gives you

Learning a system is easiest when the lessons live next to the thing they explain and actually run. This crate powers exactly that. A lesson -- a small runnable setup paired with a short note on its purpose -- ships inside the library it teaches. When that library loads, its lessons register themselves, and this engine gathers them into books and chapters that any SIM surface can show: the command line, the web view, the in-product help, or an assistant. It reads and checks each lesson collection, embeds them at build time, keeps a searchable store, works out what to read next, and lets a person layer their own notes on top in a steady, repeatable way. The result is one consistent library of hands-on examples drawn from across the whole system.

## Why you will be glad

- The examples that explain a library travel with it, so they stay honest and current.
- One engine feeds every surface, so the lessons read the same everywhere you meet them.
- Search and a sensible next-lesson suggestion make it easy to keep learning without getting lost.

## Where it fits

This is the shared teaching engine at SIM's foundation. It does the gathering, checking, embedding, and projection of lessons but leaves the runtime actions to a companion library, so the command line, the web view, help, and assistant surfaces all read from one projection rather than each growing its own. That single source keeps SIM's guided material coherent no matter where a newcomer starts.
