use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Debug)]
pub struct Root(SyntaxNode);
impl Root {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::Root {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn children(&self) -> impl Iterator<Item = SyntaxNode> {
    self.0.children()
  }
  pub fn let_exprs(&self) -> impl Iterator<Item = LetExpr> {
    self.0.children().filter_map(LetExpr::cast)
  }
  pub fn type_decls(&self) -> impl Iterator<Item = TypeDecl> {
    self.0.children().filter_map(TypeDecl::cast)
  }
}

#[derive(Debug)]
pub struct LetExpr(SyntaxNode);
impl LetExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::LetExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn name(&self) -> Option<Ident> {
    self.0.children().nth(1).and_then(Ident::cast)
  }
  pub fn value(&self) -> Option<Expr> {
    self.0.children().nth(3).and_then(Expr::cast)
  }
  pub fn in_expr(&self) -> Option<InExpr> {
    self.0.children().nth(4).and_then(InExpr::cast)
  }
}

#[derive(Debug)]
pub struct InExpr(SyntaxNode);
impl InExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::InExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn expr(&self) -> Option<Expr> {
    self.0.children().nth(1).and_then(Expr::cast)
  }
}

#[derive(Debug)]
pub struct BinaryExpr(SyntaxNode);
impl BinaryExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::BinaryExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn lhs(&self) -> Option<Expr> {
    self.0.children().nth(0).and_then(Expr::cast)
  }
  pub fn operator(&self) -> Option<SyntaxNode> {
    self.0.children().nth(1)
  }
  pub fn rhs(&self) -> Option<Expr> {
    self.0.children().nth(2).and_then(Expr::cast)
  }
}

#[derive(Debug)]
pub struct PrefixExpr(SyntaxNode);
impl PrefixExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::PrefixExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn operator(&self) -> Option<SyntaxNode> {
    self.0.children().nth(0)
  }
  pub fn expr(&self) -> Option<Expr> {
    self.0.children().nth(1).and_then(Expr::cast)
  }
}

#[derive(Debug)]
pub struct LiteralExpr(SyntaxNode);
impl LiteralExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::Literal {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn value(&self) -> Option<SyntaxToken> {
    self.0.children().nth(1).and_then(|n| n.first_token())
  }
}

#[derive(Debug)]
pub struct Ident(SyntaxNode);
impl Ident {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::Ident {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn token(&self) -> Option<SyntaxToken> {
    self.0.children().nth(1).and_then(|n| n.first_token())
  }
  pub fn text(&self) -> Option<String> {
    self.token().map(|t| t.text().to_string())
  }
}

#[derive(Debug)]
pub struct FnExpr(SyntaxNode);
impl FnExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::FnExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn param_list(&self) -> Option<ParamList> {
    self.0.children().nth(2).and_then(ParamList::cast)
  }
  pub fn return_type(&self) -> Option<TypeExpr> {
    self.0.children().filter_map(TypeExpr::cast).next()
  }
  pub fn body(&self) -> Option<Expr> {
    self.0.children().filter_map(Expr::cast).next()
  }
}

#[derive(Debug)]
pub struct ParamList(SyntaxNode);
impl ParamList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::ParamList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn params(&self) -> impl Iterator<Item = Param> + '_ {
    self.0.children().filter_map(|n| {
      if n.kind() == SyntaxKind::Ident {
        Some(Param {
          name: Ident::cast(n.clone())?,
          type_expr: None,
        })
      } else {
        None
      }
    })
  }
}

#[derive(Debug)]
pub struct Param {
  pub name: Ident,
  pub type_expr: Option<TypeExpr>,
}

#[derive(Debug)]
pub struct CallExpr(SyntaxNode);
impl CallExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::CallExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn callee(&self) -> Option<Expr> {
    self.0.children().nth(0).and_then(Expr::cast)
  }
  pub fn arg_list(&self) -> Option<ArgList> {
    self.0.children().nth(1).and_then(ArgList::cast)
  }
}

#[derive(Debug)]
pub struct ArgList(SyntaxNode);
impl ArgList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::ArgList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn args(&self) -> impl Iterator<Item = Expr> {
    self.0.children().filter_map(Expr::cast)
  }
}

#[derive(Debug)]
pub struct ParenExpr(SyntaxNode);
impl ParenExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::ParenExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn expr(&self) -> Option<Expr> {
    self.0.children().nth(1).and_then(Expr::cast)
  }
}

#[derive(Debug)]
pub struct MatchExpr(SyntaxNode);
impl MatchExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::MatchExpr {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn target(&self) -> Option<Expr> {
    self.0.children().nth(1).and_then(Expr::cast)
  }
  pub fn branches(&self) -> impl Iterator<Item = MatchBranch> {
    self.0.children().filter_map(MatchBranch::cast)
  }
  pub fn else_clause(&self) -> Option<ElseClause> {
    self.0.children().filter_map(ElseClause::cast).next()
  }
}

#[derive(Debug)]
pub struct MatchBranch(SyntaxNode);
impl MatchBranch {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::MatchBranch {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn pattern(&self) -> Option<Pattern> {
    self.0.children().nth(1).and_then(Pattern::cast)
  }
  pub fn when_clause(&self) -> Option<WhenClause> {
    self.0.children().nth(2).and_then(WhenClause::cast)
  }
  pub fn expr(&self) -> Option<Expr> {
    self.0.children().filter_map(Expr::cast).next()
  }
}

#[derive(Debug)]
pub struct WhenClause(SyntaxNode);
impl WhenClause {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::WhenClause {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn condition(&self) -> Option<Expr> {
    self.0.children().nth(1).and_then(Expr::cast)
  }
}

#[derive(Debug)]
pub struct ElseClause(SyntaxNode);
impl ElseClause {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::ElseClause {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn expr(&self) -> Option<Expr> {
    self.0.children().nth(2).and_then(Expr::cast)
  }
}

#[derive(Debug)]
pub struct TypeDecl(SyntaxNode);
impl TypeDecl {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::TypeDecl {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn name(&self) -> Option<Ident> {
    self.0.children().nth(2).and_then(Ident::cast)
  }
  pub fn type_params(&self) -> impl Iterator<Item = Ident> {
    let mut found_of = false;
    let mut found_colon = false;
    self.0.children().filter_map(move |n| {
      if n.kind() == SyntaxKind::Punctuation {
        if n.first_token()?.text() == "of" {
          found_of = true;
        } else if n.first_token()?.text() == ":" {
          found_colon = true;
        }
      }
      if found_of && !found_colon && n.kind() == SyntaxKind::Ident {
        Ident::cast(n)
      } else {
        None
      }
    })
  }
  pub fn constructors(&self) -> impl Iterator<Item = TypeConstructor> {
    self.0.children().filter_map(TypeConstructor::cast)
  }
}

#[derive(Debug)]
pub struct TypeConstructor(SyntaxNode);
impl TypeConstructor {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::TypeConstructor {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn name(&self) -> Option<Ident> {
    self.0.children().nth(1).and_then(Ident::cast)
  }
  pub fn params(&self) -> impl Iterator<Item = TypeExpr> {
    let mut found_of = false;
    let mut found_arrow = false;
    self.0.children().filter_map(move |n| {
      if n.kind() == SyntaxKind::Punctuation {
        if n.first_token()?.text() == "of" {
          found_of = true;
        } else if n.first_token()?.text() == "->" {
          found_arrow = true;
        }
      }
      if found_of && !found_arrow {
        TypeExpr::cast(n)
      } else {
        None
      }
    })
  }
  pub fn return_type(&self) -> Option<TypeExpr> {
    let mut found_arrow = false;
    self.0.children().find_map(|n| {
      if n.kind() == SyntaxKind::Punctuation && n.first_token()?.text() == "->" {
        found_arrow = true;
        None
      } else if found_arrow {
        TypeExpr::cast(n)
      } else {
        None
      }
    })
  }
}

#[derive(Debug, Clone)]
pub struct TypeExpr(SyntaxNode);
impl TypeExpr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    match node.kind() {
      SyntaxKind::TypeExpr | SyntaxKind::TypeApp => Some(Self(node)),
      _ => None,
    }
  }
  pub fn base(&self) -> Option<Ident> {
    self.0.children().nth(0).and_then(Ident::cast)
  }
  pub fn args(&self) -> impl Iterator<Item = TypeExpr> {
    if self.0.kind() == SyntaxKind::TypeApp {
      self
        .0
        .children()
        .filter_map(TypeExpr::cast)
        .collect::<Vec<_>>()
        .into_iter()
    } else {
      vec![].into_iter()
    }
  }
}

#[derive(Debug)]
pub enum Pattern {
  Ident(Ident),
  Literal(LiteralExpr),
}

impl Pattern {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    match node.kind() {
      SyntaxKind::Ident => Ident::cast(node).map(Pattern::Ident),
      SyntaxKind::Literal => LiteralExpr::cast(node).map(Pattern::Literal),
      _ => None,
    }
  }
}

#[derive(Debug)]
pub enum Expr {
  Binary(BinaryExpr),
  Prefix(PrefixExpr),
  Literal(LiteralExpr),
  Ident(Ident),
  Fn(FnExpr),
  Call(CallExpr),
  Paren(ParenExpr),
  Match(MatchExpr),
  Let(LetExpr),
}

impl Expr {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    match node.kind() {
      SyntaxKind::BinaryExpr => BinaryExpr::cast(node).map(Expr::Binary),
      SyntaxKind::PrefixExpr => PrefixExpr::cast(node).map(Expr::Prefix),
      SyntaxKind::Literal => LiteralExpr::cast(node).map(Expr::Literal),
      SyntaxKind::Ident => Ident::cast(node).map(Expr::Ident),
      SyntaxKind::FnExpr => FnExpr::cast(node).map(Expr::Fn),
      SyntaxKind::CallExpr => CallExpr::cast(node).map(Expr::Call),
      SyntaxKind::ParenExpr => ParenExpr::cast(node).map(Expr::Paren),
      SyntaxKind::MatchExpr => MatchExpr::cast(node).map(Expr::Match),
      SyntaxKind::LetExpr => LetExpr::cast(node).map(Expr::Let),
      _ => None,
    }
  }
}
