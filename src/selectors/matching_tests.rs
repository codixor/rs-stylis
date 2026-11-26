use super::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use super::matching::*;
use super::parser::tests::{
    DummyAtom, DummyAttrValue, DummyParser, DummySelectorImpl, PseudoClass, PseudoElement,
};
use super::parser::{NestingRequirement, ParseErrorRecovery, SelectorList};
use super::{Element, OpaqueElement};
use cssparser::{Parser, ParserInput};

// A flat row of four siblings. Only elements 0, 2 and 3 have class "on".
#[derive(Clone, Debug)]
struct Sibling(usize);

impl<'i> Element<'i> for Sibling {
    type Impl = DummySelectorImpl;
    fn opaque(&self) -> OpaqueElement {
        static IDS: [u8; 4] = [0, 1, 2, 3];
        OpaqueElement::new(&IDS[self.0])
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }
    fn next_sibling_element(&self) -> Option<Self> {
        (self.0 < 3).then(|| Self(self.0 + 1))
    }
    fn has_class(&self, name: &DummyAtom, _: CaseSensitivity) -> bool {
        self.0 != 1 && name == &DummyAtom::from("on")
    }
    fn parent_element(&self) -> Option<Self> {
        None
    }
    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }
    fn is_pseudo_element(&self) -> bool {
        false
    }
    fn is_html_element_in_html_document(&self) -> bool {
        true
    }
    fn has_local_name(&self, _: &DummyAtom) -> bool {
        false
    }
    fn has_namespace(&self, _: &DummyAtom) -> bool {
        false
    }
    fn is_same_type(&self, _: &Self) -> bool {
        true
    }
    fn attr_matches(
        &self,
        _: &NamespaceConstraint<&DummyAtom>,
        _: &DummyAtom,
        _: &AttrSelectorOperation<&DummyAttrValue>,
    ) -> bool {
        false
    }
    fn match_non_ts_pseudo_class<F>(
        &self,
        _: &PseudoClass,
        _: &mut MatchingContext<'_, 'i, Self::Impl>,
        _: &mut F,
    ) -> bool
    where
        F: FnMut(&Self, ElementSelectorFlags),
    {
        false
    }
    fn match_pseudo_element(
        &self,
        _: &PseudoElement,
        _: &mut MatchingContext<'_, 'i, Self::Impl>,
    ) -> bool {
        false
    }
    fn is_link(&self) -> bool {
        false
    }
    fn is_html_slot_element(&self) -> bool {
        false
    }
    fn has_id(&self, _: &DummyAtom, _: CaseSensitivity) -> bool {
        false
    }
    fn imported_part(&self, _: &DummyAtom) -> Option<DummyAtom> {
        None
    }
    fn is_part(&self, _: &DummyAtom) -> bool {
        false
    }
    fn is_empty(&self) -> bool {
        true
    }
    fn is_root(&self) -> bool {
        false
    }
}

fn parse(css: &str) -> SelectorList<'_, DummySelectorImpl> {
    let mut input = ParserInput::new(css);
    SelectorList::parse(
        &DummyParser::default(),
        &mut Parser::new(&mut input),
        ParseErrorRecovery::DiscardList,
        NestingRequirement::None,
    )
    .unwrap()
}

#[test]
fn filtered_nth_counts_only_matching_siblings_in_each_direction() {
    let mut ctx = MatchingContext::new(MatchingMode::Normal, None, None, QuirksMode::NoQuirks);
    for (css, expected) in [
        (":nth-child(2 of .on)", vec![2]),
        (":nth-last-child(1 of .on)", vec![3]),
        (":nth-child(-n+2 of .on)", vec![0, 2]),
    ] {
        let selectors = parse(css);
        assert!(check_matching_support(&selectors.0[0]).is_ok());
        let matched: Vec<_> = (0..4)
            .filter(|i| matches_selector_list(&selectors, &Sibling(*i), &mut ctx))
            .collect();
        assert_eq!(matched, expected, "{css}");
    }
}

#[test]
fn unsupported_selector_features_are_explicit_and_do_not_match_under_negation() {
    let mut ctx = MatchingContext::new(MatchingMode::Normal, None, None, QuirksMode::NoQuirks);
    for (css, reason) in [
        (":has(.on)", UnsupportedSelector::Has),
        (":not(:has(.on))", UnsupportedSelector::Has),
        (":nth-col(2)", UnsupportedSelector::Column),
    ] {
        let selectors = parse(css);
        assert_eq!(check_matching_support(&selectors.0[0]), Err(reason));
        assert_eq!(
            try_matches_selector(
                &selectors.0[0],
                0,
                None,
                &Sibling(0),
                &mut ctx,
                &mut |_, _| {}
            ),
            Err(reason)
        );
        assert!(!matches_selector_list(&selectors, &Sibling(0), &mut ctx));
    }
}
