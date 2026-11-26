use visitor::{Visit, VisitTypes, Visitor};

mod visitor {
    bitflags::bitflags! {
        pub struct VisitTypes: u8 { const TOKEN = 1; }
    }

    pub trait Visitor<'i, R> {
        type Error;
        fn visit_types(&self) -> VisitTypes;
        fn token(&mut self, value: &mut super::Token) -> Result<(), Self::Error>;
    }

    pub trait Visit<'i, R, V: Visitor<'i, R> + ?Sized> {
        const CHILD_TYPES: VisitTypes;
        fn visit_children(&mut self, visitor: &mut V) -> Result<(), V::Error>;
        fn visit(&mut self, visitor: &mut V) -> Result<(), V::Error> {
            self.visit_children(visitor)
        }
    }

    impl<'i, V: Visitor<'i, ()> + ?Sized> Visit<'i, (), V> for () {
        const CHILD_TYPES: VisitTypes = VisitTypes::empty();
        fn visit_children(&mut self, _: &mut V) -> Result<(), V::Error> {
            Ok(())
        }
    }
}

#[derive(stylis_derive::Visit)]
#[visit(token, TOKEN)]
struct Token;

#[derive(stylis_derive::Visit)]
struct Node {
    pub token: Token,
}

struct Counter(usize);
impl Visitor<'_, ()> for Counter {
    type Error = std::convert::Infallible;
    fn visit_types(&self) -> VisitTypes {
        VisitTypes::TOKEN
    }
    fn token(&mut self, _: &mut Token) -> Result<(), Self::Error> {
        self.0 += 1;
        Ok(())
    }
}

#[test]
fn generated_traversal_uses_the_consumers_visitor() {
    let mut visitor = Counter(0);
    Node { token: Token }.visit(&mut visitor).unwrap();
    assert_eq!(visitor.0, 1);
}
