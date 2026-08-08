#!/usr/bin/env python3
"""Enumerate painty's public numeric surface from rustdoc's own resolved view of it.

WHY THIS EXISTS, AND WHY IT IS NOT THE `syn` CENSUS
---------------------------------------------------
`tests/numeric_widths.rs` reads painty's source and *recognises* the places a public numeric can
appear.  Three consecutive reviews each found another place it did not recognise — a
rustfmt-wrapped signature, a trait method, a name that prefixes a pinned one, a macro inside an
impl block, a const-generic parameter — and a change of mechanism partway through, from scanning
lines to parsing with `syn`, made the recogniser better without making it closed.  Every one of
those holes has the same shape: the check established its conclusion relative to a frame, *the set
of syntactic positions a public numeric can occupy*, that it never established.

That frame cannot be composed away by a reader of source text, because the set of positions is
whatever the grammar allows and the grammar grows.  It can be handed to something that
**enumerates** the public API instead of recognising it, and rustdoc is the only thing that does:
it has already resolved aliases, expanded macros, applied visibility and dropped private fields.

The walk below therefore names no syntactic position at all.  It recurses over arbitrary JSON
looking for `{"primitive": "<integer>"}`, so a width in a const-generic parameter, a type-parameter
default, a supertrait binding or a macro expansion is found by the same three lines that find one
in a return type.  There is nothing here to add a case to.

WHAT IS STILL A FRAME
---------------------
Reachability.  Deciding which items are *public* follows four kinds of child link, and rustdoc
marks enum variants `default` even though they are as public as their enum.  That is a much
smaller and far more stable frame than the grammar, and it is checked from the other side: the
expected set below is a literal, so an item appearing or disappearing fails either way.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

INTEGERS = {
    "u8", "u16", "u32", "u64", "u128",
    "usize", "i8", "i16", "i32", "i64", "i128", "isize",
}

# The format this script was written against.  rustdoc's JSON schema is unstable, so a bump has to
# fail loudly here rather than silently find nothing: a walk over a shape it does not recognise
# reports an empty surface, and an empty surface passes every comparison that matters.
EXPECTED_FORMAT_VERSION = 61

# Every public item that carries a primitive integer, and which ones.  Derived by this script and
# checked in, so that adding a public numeric fails until somebody writes it down here as well as
# pinning it in `tests/numeric_widths.rs`.
EXPECTED = {
    "Line::char_count": ["u64"],
    "Line::column_at": ["u64", "usize"],
    "Line::number": ["u64"],
    "LineBreak::byte_len": ["usize"],
    "Location::entire": ["u32"],
    "Location::new": ["u32"],
    "Location::source": ["u32"],
    "PathSegment::Index": ["u64"],
    "Position::column": ["u64"],
    "Position::line": ["u64"],
    "Position::offset": ["usize"],
    "Region::line_count": ["u64"],
    "RegionLine::columns": ["u64"],
    "Source::len": ["usize"],
    "Source::line": ["u64"],
    "Source::line_at": ["usize"],
    "Source::line_count": ["u64"],
    "Source::position": ["usize"],
    "Span::contains": ["usize"],
    "Span::empty": ["usize"],
    "Span::end": ["usize"],
    "Span::len": ["usize"],
    "Span::new": ["usize"],
    "Span::start": ["usize"],
}

# Methods whose signature is a foreign trait's rather than painty's.  `Iterator::size_hint` returns
# `(usize, Option<usize>)` because `Iterator` says so; painty chooses nothing there.  Keyed on the
# trait and the method, so implementing a *different* trait method with a numeric still reports.
FOREIGN = {("Iterator", "size_hint")}


def primitives(node, found):
    """Every primitive integer anywhere inside `node`, at any depth, in any position."""
    if isinstance(node, dict):
        name = node.get("primitive")
        if isinstance(name, str) and name in INTEGERS:
            found.add(name)
        for value in node.values():
            primitives(value, found)
    elif isinstance(node, list):
        for value in node:
            primitives(value, found)


def children(item):
    """The ids an item owns: nesting links, plus the target of a re-export."""
    inner = item.get("inner")
    if not isinstance(inner, dict):
        return []
    out = []
    for body in inner.values():
        if not isinstance(body, dict):
            continue
        # A `pub use` is how every one of painty's types reaches the root: they are declared in
        # private modules and re-exported flat. Not following it finds nothing at all, which is
        # what the empty-surface guard below caught on the first run.
        target = body.get("id")
        if isinstance(target, int):
            out.append(target)
        for key in ("items", "variants", "impls"):
            out.extend(i for i in body.get(key) or [] if isinstance(i, int))
        kind = body.get("kind")
        if isinstance(kind, dict):
            for shape in kind.values():
                if isinstance(shape, dict):
                    out.extend(i for i in shape.get("fields") or [] if isinstance(i, int))
                elif isinstance(shape, list):
                    out.extend(i for i in shape if isinstance(i, int))
    return out


def walk(doc):
    """Every public item, as `path -> sorted primitives`, plus the ones a foreign trait owns."""
    index = {int(k): v for k, v in doc["index"].items()}
    surface, skipped = {}, []
    seen = set()
    # (id, owner name, the trait this item is implementing, if any)
    queue = [(doc["root"], None, None)]

    while queue:
        ident, owner, via_trait = queue.pop()
        if (ident, owner) in seen:
            continue
        seen.add((ident, owner))
        item = index.get(ident)
        if item is None:
            continue

        inner = item.get("inner") if isinstance(item.get("inner"), dict) else {}
        kind = next(iter(inner), "")
        name = item.get("name")

        if kind == "impl":
            trait = (inner["impl"].get("trait") or {}).get("path")
            for child in children(item):
                queue.append((child, owner, trait))
            continue

        # A variant is as public as its enum; everything else states its own visibility.
        public = item.get("visibility") == "public" or kind in ("variant", "struct_field")
        if not public:
            continue

        # A variant owns its fields, so `PathSegment::Index`'s payload is attributed to the
        # variant and not to the enum. Without this the tuple field reports as `PathSegment::0`.
        if kind == "variant":
            inherited = f"{owner}::{name}" if owner else str(name)
        elif kind in ("struct", "enum", "trait"):
            inherited = str(name)
        else:
            inherited = owner
        for child in children(item):
            queue.append((child, inherited, None))

        if kind in ("module", "struct", "enum", "trait", "use"):
            continue

        found = set()
        primitives(inner, found)
        if not found:
            continue

        # A tuple field has no name of its own — `0`, `1` — so it reports as the variant or
        # struct that owns it, which is what a caller writes.
        if kind == "struct_field" and str(name).isdigit():
            path = str(owner)
        else:
            path = f"{owner}::{name}" if owner else str(name)
        if via_trait and (via_trait, name) in FOREIGN:
            skipped.append(f"{path} (from `{via_trait}`)")
            continue
        surface.setdefault(path, set()).update(found)

    return {k: sorted(v) for k, v in surface.items()}, sorted(skipped)


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "target/doc/painty.json")
    doc = json.loads(path.read_text())

    version = doc.get("format_version")
    if version != EXPECTED_FORMAT_VERSION:
        print(
            f"::error::rustdoc JSON is format {version}, this script reads "
            f"{EXPECTED_FORMAT_VERSION}. A walk over a shape it does not recognise reports an "
            f"empty surface and passes, so it stops instead. Re-read the schema and update "
            f"EXPECTED_FORMAT_VERSION."
        )
        return 1

    surface, skipped = walk(doc)
    for name, widths in sorted(surface.items()):
        print(f"  {name}: {','.join(widths)}")
    for name in skipped:
        print(f"  (a foreign trait's width, not painty's) {name}")

    if not surface:
        print("::error::the walk found no public numeric at all, which cannot be right")
        return 1

    failed = False
    for name in sorted(set(surface) | set(EXPECTED)):
        got, want = surface.get(name), EXPECTED.get(name)
        if got == want:
            continue
        failed = True
        if want is None:
            print(f"::error::{name} carries {got} and is not in the expected surface")
        elif got is None:
            print(f"::error::{name} was expected to carry {want} and is gone")
        else:
            print(f"::error::{name} carries {got}, expected {want}")

    if failed:
        print(
            "::error::painty's public numeric surface has changed. Place each new member with the "
            "ordered rule in README.md#numeric-widths, pin it in tests/numeric_widths.rs under the "
            "rule's own function, and record it here."
        )
        return 1

    print(f"ok: {len(surface)} public numeric members, all as recorded")
    return 0


if __name__ == "__main__":
    sys.exit(main())
