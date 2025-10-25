use rowan::{GreenNode, GreenToken};
use untwine::parser;

use crate::ast::{SyntaxKind, SyntaxNode};

pub use untwine::{parse, parser_repl};

parser! {
    [recover = true]
    pub number: whole = <'0'-'9'+>
      frac = <'.' '0'-'9'+>?
      -> SyntaxNode {
        let mut text = String::from(whole);
        let kind = if let Some(frac_text) = frac {
            text.push_str(frac_text);
            SyntaxKind::Float
        } else {
            SyntaxKind::Int
        };

        let green = GreenNode::new(
            SyntaxKind::Root.into(),
            vec![GreenToken::new(kind.into(), &text).into()]
        );

        SyntaxNode(rowan::SyntaxNode::new_root(green))
    }
}

