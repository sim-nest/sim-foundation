# sim-lib-surface-card

In one line: The shared name-translator that presents SIM's tools cleanly to outside systems and people.

## What it gives you

Inside SIM a tool carries a structured name with a group and a dotted path. Outside systems each want that name in their own style: one tool platform allows only letters and underscores, while a human reading a menu wants the natural slashed-and-dotted form. This crate holds the single agreed rule for that translation, so a given SIM name always turns into the same external name for the same audience. It also carries a plain description spine -- a codec-neutral record of what a surface offers -- built only from core types. Concrete surfaces draw their outward names through this one door, which keeps a single source of truth for the naming rules instead of each surface bending them a little differently.

## Why you will be glad

- The same SIM tool always shows up under the same outside name for a given audience.
- Names fit the character rules of each destination without any surface guessing on its own.
- One shared record of the mangling rules keeps every surface honest and in step.

## Where it fits

This is the codec-neutral heart of a surface card: the part that describes a tool for foreign consumers without committing to any one wire format. It rests on core types alone, and the concrete surfaces -- a chat tool descriptor, a file-facing view, human-facing text -- build on top of it. By owning the naming policy in one place, it keeps SIM's outward face steady across every audience it meets.
