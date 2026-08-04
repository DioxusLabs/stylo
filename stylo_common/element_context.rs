/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Element-level context needed during style computation, and support for
//! tracking attribute references and tree-counting function results.

use crate::atom_types::{LocalName, Namespace};
use rustc_hash::FxHashMap;
use selectors::OpaqueElement;
use smallvec::SmallVec;
use style_traits::dom::OpaqueNode;
use style_traits::precomputed_hash::PrecomputedHashMap;

/// Holds the resolved sibling-index() and sibling-count() values for an element.
#[derive(Clone, Copy, Debug)]
pub struct TreeCountingResult {
    /// The 1-based index of the element among its siblings.
    pub sibling_index: u32,
    /// The total number of siblings of the element, including itself.
    pub sibling_count: u32,
}

impl TreeCountingResult {
    /// Creates a new TreeCountingResult with the given index and count.
    pub fn new(sibling_index: u32, sibling_count: u32) -> Self {
        TreeCountingResult {
            sibling_index,
            sibling_count,
        }
    }

    /// Creates a default TreeCountingResult.
    pub fn default() -> Self {
        TreeCountingResult::new(1, 1)
    }
}

/// Caches to speed up evalution of tree-counting functions. Separate caches
/// for index and count are used so that they can be populated in a single
/// traversal of an element's siblings.
///
/// TODO(Bug 2046399) - Consider directly using the SelectorCaches instead.
#[derive(Default)]
pub struct TreeCountingCaches {
    /// A cache of element sibling-index() values.
    pub sibling_index: FxHashMap<OpaqueElement, u32>,
    /// A cache of element sibling-count() values, keyed by the element's parent node.
    pub sibling_count: FxHashMap<OpaqueNode, u32>,
}

impl TreeCountingCaches {
    /// Look up the tree-counting function values for the given element. If the element and
    /// its parent node are not cached, the values are computed and stored.
    pub fn get_or_compute(&mut self, element_context: &dyn ElementContext) -> TreeCountingResult {
        let (Some(target), Some(parent)) = (
            element_context.opaque_element(),
            element_context.opaque_parent(),
        ) else {
            return TreeCountingResult::default();
        };

        // Lookup from the index and count caches
        let cached_index = self.sibling_index.get(&target).copied();
        let cached_count = self.sibling_count.get(&parent).copied();
        if let (Some(index), Some(count)) = (cached_index, cached_count) {
            return TreeCountingResult::new(index, count);
        }

        // Compute the sibling index and sibling count for the element,
        // inserting into the caches as it traverses through its siblings.
        element_context.get_tree_counting_result(self)
    }
}

/// Provides element-level context needed during style computation.
pub trait ElementContext {
    /// Opaque handle to the element.
    fn opaque_element(&self) -> Option<OpaqueElement>;

    /// Opaque handle to the element's parent node.
    fn opaque_parent(&self) -> Option<OpaqueNode>;

    /// Return the value of the given custom attribute if it exists.
    fn get_attr(&self, attr: &LocalName, namespace: &Namespace) -> Option<String>;

    /// Traverse the siblings of the element, returning the element's sibling-index()
    /// and sibling-count(). Also populates `caches` with the sibling-index() and
    /// sibling-count() values for all siblings of this element.
    fn get_tree_counting_result(&self, caches: &mut TreeCountingCaches) -> TreeCountingResult;
}

/// A set of the attributes used to compute a style that uses `attr()`
pub type AttributeReferences = Option<Box<PrecomputedHashMap<LocalName, SmallVec<[Namespace; 1]>>>>;

/// A data structure to keep track of the names queried from an element.
pub struct AttributeTracker<'a> {
    /// The element that queries for attributes.
    pub context: &'a dyn ElementContext,
    /// The set of attributes we have queried.
    pub references: AttributeReferences,
}

impl<'a> AttributeTracker<'a> {
    /// Construct a new attribute tracker trivially.
    pub fn new(context: &'a dyn ElementContext) -> Self {
        Self {
            context,
            references: None,
        }
    }

    /// Construct a new dummy attribute tracker
    pub fn new_dummy() -> Self {
        Self {
            context: &DummyElementContext {},
            references: None,
        }
    }

    /// Extract the queried references and consume self
    pub fn finalize(self) -> AttributeReferences {
        self.references
    }

    /// Query the value and save the name of the attribtue.
    pub fn query(&mut self, name: &LocalName, namespace: &Namespace) -> Option<String> {
        // We need to save namespaces in case we are thinking of sharing this element's
        // style with another.
        // i.e if elment a has ns1::attr="blue"
        // and element b has ns2::attr="blue"
        // a and b can only share style if ns1 and ns2 resolve to the same namespace.
        self.references
            .get_or_insert_default()
            .entry(name.clone())
            .or_default()
            .push(namespace.clone());
        self.context.get_attr(name, namespace)
    }
}

/// A dummy ElementContext that returns default values to any query.
#[derive(Clone, Debug, PartialEq)]
pub struct DummyElementContext;

impl ElementContext for DummyElementContext {
    fn get_attr(&self, _attr: &LocalName, _namespace: &Namespace) -> Option<String> {
        None
    }

    fn opaque_element(&self) -> Option<OpaqueElement> {
        None
    }

    fn opaque_parent(&self) -> Option<OpaqueNode> {
        None
    }

    fn get_tree_counting_result(&self, _: &mut TreeCountingCaches) -> TreeCountingResult {
        TreeCountingResult::default()
    }
}
