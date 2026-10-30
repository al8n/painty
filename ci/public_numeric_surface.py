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
    "Ansi16::from_index": ["u8"],
    "Ansi16::index": ["u8"],
    "Ansi16::to_rgb": ["u8"],
    "Color::Ansi256": ["u8"],
    "Color::Rgb": ["u8"],
    "LineCells::columns_for": ["u64"],
    "LineCells::default_tab_width": ["u64"],
    "LineCells::max_tab_width": ["u64"],
    "LineCells::cells_between": ["u64", "usize"],
    "LineCells::column_at": ["u64", "usize"],
    "LineCells::new": ["u64"],
    "LineCells::tab_width": ["u64"],
    "LineCells::width": ["u64"],
    "Terminal::max_rendered_width": ["u64"],
    "Terminal::max_source_bytes": ["u64"],
    "Terminal::max_svg_message_bytes": ["u64"],
    "Terminal::underline": ["u64"],
    "Terminal::with_tab_width": ["u64"],
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
    """The name of an impl's self type.

    The key is `for`, not `for_`. Reading the wrong one returned `"?"` for every impl and went
    unnoticed for a round, because the caller at the time already had the name from elsewhere and
    only fell back to this. It surfaced the moment that caller was removed, which is the argument
    for the unnamed check in `walk`: a label this file cannot build is now a failure.
    """
    holder = inner.get("impl") or {}
    target = holder.get("for") or holder.get("for_") or {}
    return (target.get("resolved_path") or {}).get("path", "?")


def children(item):
    """The ids an item owns: nesting links, plus the target of a re-export.

    NAMING ONLY. Nothing here decides whether an item is public — see `walk`. A gap in this list
    costs a label, not a member, and `walk` fails when it cannot label something.
    """
    inner = item.get("inner")
    if not isinstance(inner, dict):
        return []
    out = []
    for body in inner.values():
        if not isinstance(body, dict):
            continue
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


def qualify(ident, index, parent):
    """`Owner::name` for an item, walking the whole ownership chain.

    All the way up, not to the first named ancestor: `PathSegment::Index`'s payload is a tuple
    field owned by a variant owned by an enum, and stopping early named it `Index`.
    """
    item = index.get(ident) or {}
    inner = item.get("inner") if isinstance(item.get("inner"), dict) else {}
    if next(iter(inner), "") == "impl":
        return f"impl {type_of(inner)}", True

    parts, hops = [], ident
    while hops is not None:
        node = index.get(hops) or {}
        held = node.get("inner") if isinstance(node.get("inner"), dict) else {}
        kind = next(iter(held), "")
        if kind == "module":
            break
        if kind == "impl":
            parts.append(type_of(held))
            break
        name = node.get("name")
        # A tuple field has no name of its own; whatever owns it supplies one.
        if not (kind == "struct_field" and str(name).isdigit()) and name:
            parts.append(str(name))
        hops = parent.get(hops)

    return "::".join(reversed(parts)), bool(parts)


def implementing_trait(ident, index, parent):
    """The trait an item is implementing, if it sits inside a trait impl."""
    holder = index.get(parent.get(ident, -1)) or {}
    inner = holder.get("inner") if isinstance(holder.get("inner"), dict) else {}
    return ((inner.get("impl") or {}).get("trait") or {}).get("path")


def walk(doc):
    """Every item rustdoc kept, as `path -> sorted primitives`.

    PUBLICNESS IS NOT COMPUTED HERE, and that is the whole design.

    It used to be. The walk queued a public container's children and applied its own inheritance
    rules: variants and struct fields inherit, everything else must say `pub`. That rule was
    incomplete — a trait's members inherit the trait's publicness too — which is a *second* place
    the same recogniser-versus-decider mistake had hidden, one level in from the source scanner
    this file replaced. Reimplementing rustdoc's visibility semantics is recognising; asking is
    deciding.

    So it asks. Run without `--document-private-items`, rustdoc has already dropped every private
    item: painty's `Source::floor`, `Source::ceil`, `fill` and the private `Location::source` field
    are simply not in `index`, verified. What is left is the public API, and it is walked flat —
    no queue, no visibility test, no inheritance to get wrong, and no traversal of children, which
    is where the previous round's category hid.

    Over-reporting is the safe direction and is what the residue of this design costs: an item that
    is somehow in `index` without being public would have to be recorded rather than being missed.
    """
    index = {int(k): v for k, v in doc["index"].items()}

    parent = {}
    for holder, item in index.items():
        for child in children(item):
            parent.setdefault(child, holder)

    surface, skipped, unnamed = {}, [], []
    for ident, item in index.items():
        inner = item.get("inner") if isinstance(item.get("inner"), dict) else {}
        kind = next(iter(inner), "")
        if kind in ("module", "use"):
            continue

        found = set()
        primitives(inner, found)
        if not found:
            continue

        path, named = qualify(ident, index, parent)

        # A member of a TRAIT IMPL takes its signature from the trait, so its widths are not
        # painty's to choose — `Iterator::size_hint` is `(usize, Option<usize>)` because `Iterator`
        # says so. Skipped by the rule rather than by a list of names, and reported, so the
        # exclusion is visible instead of assumed.
        #
        # An associated TYPE is the exception: `type Item = usize` is painty picking a width inside
        # somebody else's shape, so it stays in the surface.
        trait = implementing_trait(ident, index, parent)
        if trait and kind != "assoc_type":
            skipped.append(f"{path} (signature belongs to `{trait}`)")
            continue
        if not named:
            unnamed.append(f"{path} carrying {sorted(found)}")
            continue
        surface.setdefault(path, set()).update(found)

    if unnamed:
        # A label this walk cannot build is a member a reader cannot act on, and silently dropping
        # it would be the under-reporting this design is arranged to avoid.
        raise RuntimeError(
            "the walk found a public numeric it could not attribute to a member:\n  "
            + "\n  ".join(sorted(unnamed))
        )

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
    "impl Page": ["usize"],       # ...and the impl that inherits it
    "Defaulted": ["u16"],         # a defaulted type parameter
    "Bounded": ["u64"],           # a where-clause predicate
    "Fielded::width": ["i64"],    # a public struct field
    "Rows": ["i8"],               # a supertrait binding
    "Rows::STRIDE": ["usize"],    # a TRAIT associated constant — inherits the trait's publicness
    "Rows::rows": ["u16"],        # a trait method
    "Rows::Index": ["u32"],       # a trait associated type's bound
    "Holder::LIMIT": ["u128"],    # an inherent associated constant
    "Grid": ["isize"],            # a const generic on a generic type
    "impl Grid": ["isize"],       # an impl's own generics
    "Kind::Wide": ["i16"],        # a tuple payload on a public enum
    "hidden_width": ["u128"],     # `#[doc(hidden)]`, still callable and still a commitment
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
