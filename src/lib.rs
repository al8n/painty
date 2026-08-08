#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

// Empty on purpose. The repository is the scaffold — manifest, feature surface, CI gates — and
// the renderer is not written yet; see the three layers in README.md for what lands here first.
//
// There is deliberately no `extern crate alloc` and no `alloc` feature. Layer 2 borrows from the
// source and is meant to stay allocation-free, and whether elision can hold that in a
// caller-provided buffer is a measurement still to be taken, not a question to pre-answer by
// making a heap available.
