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


def type_of(inner):
    """A best-effort name for an impl's self type, for reporting only."""
    target = (inner.get("impl") or {}).get("for_") or {}
    return (target.get("resolved_path") or {}).get("path", "?")


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
            # An impl carries its own generics — `impl<const N: usize> T for U<N>` — which belong
            # to no child. Falling straight through to the children was the traversal-shaped hole.
            found = set()
            primitives(inner, found)
            if found:
                surface.setdefault(f"impl {owner or type_of(inner)}", set()).update(found)
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

        # SCAN THE CONTAINER TOO. Its children are queued above and are separate items, so nothing
        # is counted twice — but a const-generic parameter, a type-parameter default, a supertrait
        # bound and a where-clause predicate all live on the CONTAINER, and skipping it here is how
        # the R6 category survived the move from a source reader to this one. The recursion matches
        # no syntactic position; the traversal did, and "children" was that position.
        if kind in ("module", "use"):
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


# Every position a public numeric can occupy that is not an ordinary signature, and the width each
# one carries in `ci/fixtures/positions.json`. That fixture is real rustdoc output from a scratch
# crate written to hold one of each, pruned to the items this walk reaches.
#
# WHY A FIXTURE AND NOT A CANARY. The previous guard here only fired when the walk found *nothing*,
# and the traversal defect it was meant to notice — containers skipped, so const generics, defaults,
# supertraits and where clauses were invisible — left every other member findable. A guard that
# trips only on total emptiness cannot see a category-shaped gap. This one names the categories, so
# losing one is a failure rather than a smaller number.
POSITIONS = {
    "Page": ["usize"],            # a const-generic parameter
    "Defaulted": ["u16"],         # a defaulted type parameter
    "Bounded": ["u64"],           # a where-clause predicate
    "Rows": ["i8"],               # a supertrait binding
    "Holder::LIMIT": ["u128"],    # an inherent associated constant
    "Grid": ["isize"],            # a const generic reached through its own impl
    "impl Grid": ["isize"],       # an impl's own generics
    "impl Page": ["usize"],
    "Kind::Wide": ["i16"],        # a tuple payload on a public enum
    "neighbour": ["u16"],         # an ordinary signature, so "found only this" is distinguishable
}


def self_test() -> int:
    """Check the traversal still reaches every position we know a width can occupy."""
    fixture = Path(__file__).parent / "fixtures" / "positions.json"
    doc = json.loads(fixture.read_text())
    surface, _ = walk(doc)
    missing = {k: v for k, v in POSITIONS.items() if surface.get(k) != v}
    extra = {k: v for k, v in surface.items() if k not in POSITIONS}
    for name, want in sorted(missing.items()):
        print(f"::error::the walk no longer reaches {name} ({want}); it reports {surface.get(name)}")
    for name, got in sorted(extra.items()):
        print(f"::error::the walk reports {name} ({got}), which the fixture does not describe")
    if missing or extra:
        print("::error::the traversal has stopped reaching a position a public numeric can occupy")
        return 1
    print(f"ok: the traversal reaches all {len(POSITIONS)} known positions")
    return 0


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--self-test":
        return self_test()
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

    # Superseded by `--self-test`, which names the categories rather than counting them, and is run
    # first by the `public-surface` job. Kept because it costs nothing and a walk that returns
    # nothing here means something changed about painty rather than about the traversal.
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
