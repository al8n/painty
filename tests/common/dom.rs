//! The tree the HTML gallery has html5ever build, in a sink of the gallery's own.
//!
//! html5ever's tests keep their trees in `markup5ever_rcdom`, which is published as an unofficial
//! crate that trails html5ever, and a sink built against one html5ever's `TreeSink` cannot be
//! handed to another's parser. So this is the part of rcdom the gallery uses, built against the
//! html5ever in `Cargo.toml`: the same reference-counted tree, the parse errors, and the `TreeSink`
//! methods a fragment parse depends on — but no serialiser, because nothing here writes the tree
//! back out.
//!
//! Adjacent text merges where rcdom merges it, into a text node that already ends the parent or
//! sits just before the insertion point, so the gallery's whitespace check sees the node
//! boundaries it was written against.

use std::{
  borrow::Cow,
  cell::RefCell,
  rc::{Rc, Weak},
};

use html5ever::{
  Attribute, ExpandedName, QualName,
  tendril::StrTendril,
  tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
};

/// A node, as html5ever hands it back and forth.
pub type Handle = Rc<Node>;

#[derive(Debug)]
pub struct Node {
  /// Weak, so that a parent and its children do not keep each other alive.
  parent: RefCell<Option<Weak<Node>>>,
  pub children: RefCell<Vec<Handle>>,
  pub data: NodeData,
}

// The gallery reads the `Doctype`, `Comment` and `ProcessingInstruction` payloads only through
// `Debug`, in the message it fails with, and dead-code analysis does not count a derived `Debug`
// as a read.
#[derive(Debug)]
pub enum NodeData {
  Document,
  #[allow(dead_code)]
  Doctype {
    name: StrTendril,
    public_id: StrTendril,
    system_id: StrTendril,
  },
  Text {
    contents: RefCell<StrTendril>,
  },
  #[allow(dead_code)]
  Comment {
    contents: StrTendril,
  },
  Element {
    name: QualName,
    attrs: RefCell<Vec<Attribute>>,
    /// A `<template>`'s contents, which hang here rather than among its children.
    template_contents: Option<Handle>,
    /// Recorded at creation because html5ever asks the sink rather than working it out again.
    mathml_annotation_xml_integration_point: bool,
  },
  #[allow(dead_code)]
  ProcessingInstruction {
    target: StrTendril,
    contents: StrTendril,
  },
}

/// The sink a parse writes into, and what the parse hands back.
pub struct Dom {
  pub document: Handle,
  /// Every parse error html5ever reported, in the order it reported them.
  pub errors: RefCell<Vec<Cow<'static, str>>>,
}

impl Default for Dom {
  fn default() -> Self {
    Self {
      document: Node::new(NodeData::Document),
      errors: RefCell::default(),
    }
  }
}

impl Node {
  fn new(data: NodeData) -> Handle {
    Rc::new(Self {
      parent: RefCell::new(None),
      children: RefCell::new(Vec::new()),
      data,
    })
  }
}

// The provided methods keep their defaults, which are right for a sink that only stores a tree.
// `mark_script_already_started`, `pop`, `associate_with_form` and `set_current_line` tell a DOM
// about scripts, forms and lines it does not have. The declarative-shadow-root pair leaves a
// `<template shadowrootmode>` in the tree as an ordinary `<template>`, where the census sees it.
// And `maybe_clone_an_option_into_selectedcontent` only copies an `<option>`'s children into a
// `<selectedcontent>`, two elements painty never writes: the census rejects either one whatever it
// holds, and the step reports no parse error, so leaving it out cannot change a verdict.
impl TreeSink for Dom {
  type Handle = Handle;
  type Output = Self;
  type ElemName<'a>
    = ExpandedName<'a>
  where
    Self: 'a;

  fn finish(self) -> Self::Output {
    self
  }

  fn parse_error(&self, msg: Cow<'static, str>) {
    self.errors.borrow_mut().push(msg);
  }

  fn get_document(&self) -> Self::Handle {
    Rc::clone(&self.document)
  }

  fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> Self::ElemName<'a> {
    let NodeData::Element { name, .. } = &target.data else {
      panic!("html5ever asks only an element for its name");
    };
    name.expanded()
  }

  fn create_element(
    &self,
    name: QualName,
    attrs: Vec<Attribute>,
    flags: ElementFlags,
  ) -> Self::Handle {
    Node::new(NodeData::Element {
      name,
      attrs: RefCell::new(attrs),
      template_contents: flags.template.then(|| Node::new(NodeData::Document)),
      mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
    })
  }

  fn create_comment(&self, text: StrTendril) -> Self::Handle {
    Node::new(NodeData::Comment { contents: text })
  }

  fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle {
    Node::new(NodeData::ProcessingInstruction {
      target,
      contents: data,
    })
  }

  fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
    let child = match child {
      NodeOrText::AppendNode(node) => node,
      NodeOrText::AppendText(text) => {
        if parent
          .children
          .borrow()
          .last()
          .is_some_and(|last| merge_text(last, &text))
        {
          return;
        }
        Node::new(NodeData::Text {
          contents: RefCell::new(text),
        })
      }
    };
    push_child(parent, child);
  }

  fn append_based_on_parent_node(
    &self,
    element: &Self::Handle,
    prev_element: &Self::Handle,
    child: NodeOrText<Self::Handle>,
  ) {
    if element.parent.borrow().is_some() {
      self.append_before_sibling(element, child);
    } else {
      self.append(prev_element, child);
    }
  }

  fn append_doctype_to_document(
    &self,
    name: StrTendril,
    public_id: StrTendril,
    system_id: StrTendril,
  ) {
    push_child(
      &self.document,
      Node::new(NodeData::Doctype {
        name,
        public_id,
        system_id,
      }),
    );
  }

  fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
    let NodeData::Element {
      template_contents: Some(contents),
      ..
    } = &target.data
    else {
      panic!("html5ever asks only a `<template>` for its contents");
    };
    Rc::clone(contents)
  }

  fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
    Rc::ptr_eq(x, y)
  }

  // The tree builder parses under the mode it keeps itself. Telling the sink is for a DOM that has
  // a use for the mode, and nothing here reads it.
  fn set_quirks_mode(&self, _mode: QuirksMode) {}

  fn append_before_sibling(&self, sibling: &Self::Handle, new_node: NodeOrText<Self::Handle>) {
    // A node may still hang under an old parent, even this one. It leaves before `sibling` is
    // found, so that its leaving cannot shift `sibling` and put it on the wrong side.
    if let NodeOrText::AppendNode(node) = &new_node {
      detach(node);
    }
    let (parent, index) =
      parent_and_index(sibling).expect("html5ever inserts only beside a node that has a parent");
    let child = match new_node {
      NodeOrText::AppendNode(node) => node,
      // The tree builder promises that `sibling` is not text, so the only text this can merge
      // into is the node before it.
      NodeOrText::AppendText(text) => {
        if index > 0 && merge_text(&parent.children.borrow()[index - 1], &text) {
          return;
        }
        Node::new(NodeData::Text {
          contents: RefCell::new(text),
        })
      }
    };
    *child.parent.borrow_mut() = Some(Rc::downgrade(&parent));
    parent.children.borrow_mut().insert(index, child);
  }

  fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>) {
    let NodeData::Element {
      attrs: existing, ..
    } = &target.data
    else {
      panic!("html5ever adds attributes only to an element");
    };
    let mut existing = existing.borrow_mut();
    for attr in attrs {
      if !existing.iter().any(|present| present.name == attr.name) {
        existing.push(attr);
      }
    }
  }

  fn remove_from_parent(&self, target: &Self::Handle) {
    detach(target);
  }

  fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle) {
    let mut children = node.children.borrow_mut();
    for child in children.iter() {
      *child.parent.borrow_mut() = Some(Rc::downgrade(new_parent));
    }
    new_parent.children.borrow_mut().append(&mut children);
  }

  fn is_mathml_annotation_xml_integration_point(&self, handle: &Self::Handle) -> bool {
    matches!(
      handle.data,
      NodeData::Element {
        mathml_annotation_xml_integration_point: true,
        ..
      }
    )
  }
}

/// Hangs `child`, which has no parent yet, last under `parent`.
fn push_child(parent: &Handle, child: Handle) {
  let previous = child.parent.replace(Some(Rc::downgrade(parent)));
  assert!(
    previous.is_none(),
    "html5ever appends only a node that has no parent"
  );
  parent.children.borrow_mut().push(child);
}

/// Takes `node` out of its parent's children, if it has a parent.
fn detach(node: &Handle) {
  if let Some((parent, index)) = parent_and_index(node) {
    parent.children.borrow_mut().remove(index);
    *node.parent.borrow_mut() = None;
  }
}

/// Where `node` sits among its parent's children, if it has a parent.
fn parent_and_index(node: &Handle) -> Option<(Handle, usize)> {
  let parent = node
    .parent
    .borrow()
    .as_ref()?
    .upgrade()
    .expect("a node's parent outlives it");
  let index = parent
    .children
    .borrow()
    .iter()
    .position(|child| Rc::ptr_eq(child, node))
    .expect("a node is among its parent's children");
  Some((parent, index))
}

/// Appends `text` to `node` if `node` is text, which is how adjacent text stays one node.
fn merge_text(node: &Handle, text: &str) -> bool {
  let NodeData::Text { contents } = &node.data else {
    return false;
  };
  contents.borrow_mut().push_slice(text);
  true
}
