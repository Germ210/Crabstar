use rowan::GreenNodeBuilder;
use untwine::recoverable::Recoverable;

#[derive(Debug, Copy, Clone, PartialEq, Hash, Ord, PartialOrd, Eq)]
#[repr(u16)]
pub enum SyntaxKind {
  Root = 0,
  Indent,
  Dedent,
  Whitespace,
  Int,
  Float,
  Ident,
  LParen,
  RParen,
  Invalid,
}

impl From<SyntaxKind> for rowan::SyntaxKind {
  fn from(value: SyntaxKind) -> Self {
    unsafe { rowan::SyntaxKind(std::mem::transmute(value)) }
  }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum CrabstarLang {}

impl rowan::Language for CrabstarLang {
  type Kind = SyntaxKind;

  fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
    unsafe { std::mem::transmute(raw.0 as u16) }
  }

  fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
    unsafe { rowan::SyntaxKind(std::mem::transmute(kind)) }
  }
}

#[derive(Debug)]
pub struct SyntaxNode(pub rowan::SyntaxNode<CrabstarLang>);

impl Recoverable for SyntaxNode {
  fn error_value(_range: std::ops::Range<usize>) -> Self {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Invalid.into());
    builder.finish_node();
    let green = builder.finish();
    let node = rowan::SyntaxNode::new_root(green);
    SyntaxNode(node)
  }
}
