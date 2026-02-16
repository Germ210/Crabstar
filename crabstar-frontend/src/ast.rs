use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::NodeOrToken;

macro_rules! make_ast {
  ($name:ident { $($fields:ident),+ $(,)? }) => {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct $name {
      syntax: SyntaxNode,
    }

    impl $name {
      pub fn syntax(&self) -> &SyntaxNode { &self.syntax }

      pub fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::$name {
          Some(Self { syntax })
        } else {
          None
        }
      }

      make_ast!(@parse_fields 0; $($fields),+);
    }
  };

  (@parse_fields $idx:expr; $head:ident, $($tail:ident),*) => {
    pub fn $head(&self) -> NodeOrToken<SyntaxNode, SyntaxToken> {
      self.syntax.children_with_tokens().nth($idx).unwrap()
    }
    make_ast!(@parse_fields $idx + 1; $($tail),*);
  };

  (@parse_fields $idx:expr; $head:ident) => {
    pub fn $head(&self) -> NodeOrToken<SyntaxNode, SyntaxToken> {
      self.syntax.children_with_tokens().nth($idx).unwrap()
    }
  };
}

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

make_ast!(Literal {
  whitespace,
  literal
});

make_ast!(Ident { whitespace, name });

make_ast!(LetExpr {
  let_kw,
  name,
  arrow,
  type_expr,
  colon,
  expr,
  in_expr
});

make_ast!(RefBindingExpr {
  let_kw,
  name,
  arrow,
  type_expr,
  colon,
  expr,
  in_expr
});

make_ast!(FieldAccess {
  structure,
  dot,
  field,
});

make_ast!(MethodCall {
  lhs,
  pipe,
  method_name,
  lparen,
  args,
  rparen
});

make_ast!(Param {
  whitespace,
  param_name,
  colon,
  type_expr
});

make_ast!(FnExpr {
  fn_kw,
  lparen,
  param_list,
  rparen,
  arrow,
  return_type,
  colon,
  body
});

make_ast!(WhenClause {
  when_kw,
  guard_clause,
});

pub struct MatchBranches(SyntaxNode);
impl MatchBranches {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::MatchBranches {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn children(&self) -> impl Iterator<Item = SyntaxNode> {
    self.0.children()
  }
}

make_ast!(MatchBranch {
  of_kw,
  pattern,
  when_clause,
  colon,
  expr
});

make_ast!(MatchExpr {
  match_kw,
  discriminant,
  lbrace,
  match_branches,
  rbrace,
  else_clause
});

make_ast!(Arg { arg_expr, comma });

pub struct ArgList(SyntaxNode);
impl ArgList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::ArgList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn args(&self) -> impl Iterator<Item = Arg> {
    self.0.children().filter_map(Arg::cast)
  }
}

make_ast!(CallExpr {
  callee,
  lparen,
  args,
  rparen
});

make_ast!(TypeExpr { inner_type });

make_ast!(RefType {
  ref_keyword,
  type_app
});

make_ast!(TypeApp {
  base_type,
  of_keyword,
  type_args
});

pub struct TypeArgList(SyntaxNode);
impl TypeArgList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::TypeArgList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn children(&self) -> impl Iterator<Item = SyntaxNode> {
    self.0.children()
  }
}

make_ast!(TypeArg { type_expr, comma });

make_ast!(TypeDecl {
  type_keyword,
  name,
  type_params,
  colon_or_equals,
  body
});

make_ast!(TypeParam { comma, name });

pub struct TypeParamList(SyntaxNode);
impl TypeParamList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::TypeParamList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn of_keyword(&self) -> SyntaxNode {
    self.0.children().nth(0).unwrap()
  }
  pub fn params(&self) -> impl Iterator<Item = TypeParam> + '_ {
    self.0.children().skip(1).filter_map(TypeParam::cast)
  }
}

make_ast!(Constructor {
  or_keyword,
  type_constructor
});

pub struct ConstructorList(SyntaxNode);
impl ConstructorList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::ConstructorList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn constructors(&self) -> impl Iterator<Item = Constructor> + '_ {
    self.0.children().filter_map(Constructor::cast)
  }
}

make_ast!(TypeConstructor {
  whitespace,
  name,
  params,
  return_types
});

make_ast!(ConstructorParam { comma, type_name });

pub struct ConstructorParamList(SyntaxNode);
impl ConstructorParamList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::ConstructorParamList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn of_or_arrow(&self) -> SyntaxNode {
    self.0.children().nth(0).unwrap()
  }
  pub fn params(&self) -> impl Iterator<Item = ConstructorParam> + '_ {
    self.0.children().skip(1).filter_map(ConstructorParam::cast)
  }
}

make_ast!(BehaviorDef {
  concept_keyword,
  name,
  requires_keyword,
  lbrace1,
  requirements,
  rbrace1,
  with_keyword,
  lbrace2,
  methods,
  rbrace2
});

pub struct RequirementList(SyntaxNode);
impl RequirementList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::RequirementList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn fields(&self) -> impl Iterator<Item = RequirementField> + '_ {
    self.0.children().filter_map(RequirementField::cast)
  }
}

make_ast!(RequirementField { comma, inner });

pub struct MethodList(SyntaxNode);
impl MethodList {
  pub fn cast(node: SyntaxNode) -> Option<Self> {
    if node.kind() == SyntaxKind::MethodList {
      Some(Self(node))
    } else {
      None
    }
  }
  pub fn methods(&self) -> impl Iterator<Item = MethodDef> + '_ {
    self.0.children().filter_map(MethodDef::cast)
  }
}

make_ast!(MethodDef {
  def_keyword,
  name,
  lparen,
  param_list,
  rparen,
  arrow,
  return_type,
  colon,
  body
});

make_ast!(ParenExpr {
  lparen,
  inner,
  rparen
});

make_ast!(WithClause {
  with_keyword,
  behavior
});

make_ast!(WithExpr { lhs, with_clause });

make_ast!(PrefixExpr { operator, rhs });

make_ast!(BinaryExpr { lhs, operator, rhs });

make_ast!(NewExpr {
  new_keyword,
  struct_name,
  lparen,
  fields,
  rparen
});

make_ast!(InExpr { in_keyword, inner });

make_ast!(ElseClause {
  else_keyword,
  colon,
  expr
});

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
    self.0.children().filter_map(Param::cast)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstNode {
  Literal(Literal),
  Ident(Ident),

  LetExpr(LetExpr),
  RefBindingExpr(RefBindingExpr),
  FieldAccess(FieldAccess),
  MethodCall(MethodCall),
  FnExpr(FnExpr),
  MatchExpr(MatchExpr),
  CallExpr(CallExpr),
  ParenExpr(ParenExpr),
  WithExpr(WithExpr),
  PrefixExpr(PrefixExpr),
  BinaryExpr(BinaryExpr),
  NewExpr(NewExpr),

  TypeExpr(TypeExpr),
  TypeApp(TypeApp),
  RefType(RefType),
  TypeDecl(TypeDecl),

  BehaviorDef(BehaviorDef),
  MethodDef(MethodDef),

  MatchBranch(MatchBranch),
  WhenClause(WhenClause),

  Param(Param),
  Arg(Arg),
  TypeArg(TypeArg),
  TypeParam(TypeParam),
  Constructor(Constructor),
  TypeConstructor(TypeConstructor),
  ConstructorParam(ConstructorParam),
  RequirementField(RequirementField),
  InExpr(InExpr),
  ElseClause(ElseClause),
  WithClause(WithClause),
}

impl AstNode {
  pub fn cast(syntax: SyntaxNode) -> Option<Self> {
    match syntax.kind() {
      SyntaxKind::Literal => Some(AstNode::Literal(Literal::cast(syntax)?)),
      SyntaxKind::Ident => Some(AstNode::Ident(Ident::cast(syntax)?)),
      SyntaxKind::LetExpr => Some(AstNode::LetExpr(LetExpr::cast(syntax)?)),
      SyntaxKind::RefBindingExpr => Some(AstNode::RefBindingExpr(RefBindingExpr::cast(syntax)?)),
      SyntaxKind::FieldAccess => Some(AstNode::FieldAccess(FieldAccess::cast(syntax)?)),
      SyntaxKind::MethodCall => Some(AstNode::MethodCall(MethodCall::cast(syntax)?)),
      SyntaxKind::FnExpr => Some(AstNode::FnExpr(FnExpr::cast(syntax)?)),
      SyntaxKind::MatchExpr => Some(AstNode::MatchExpr(MatchExpr::cast(syntax)?)),
      SyntaxKind::CallExpr => Some(AstNode::CallExpr(CallExpr::cast(syntax)?)),
      SyntaxKind::ParenExpr => Some(AstNode::ParenExpr(ParenExpr::cast(syntax)?)),
      SyntaxKind::WithExpr => Some(AstNode::WithExpr(WithExpr::cast(syntax)?)),
      SyntaxKind::PrefixExpr => Some(AstNode::PrefixExpr(PrefixExpr::cast(syntax)?)),
      SyntaxKind::BinaryExpr => Some(AstNode::BinaryExpr(BinaryExpr::cast(syntax)?)),
      SyntaxKind::NewExpr => Some(AstNode::NewExpr(NewExpr::cast(syntax)?)),
      SyntaxKind::TypeExpr => Some(AstNode::TypeExpr(TypeExpr::cast(syntax)?)),
      SyntaxKind::TypeApp => Some(AstNode::TypeApp(TypeApp::cast(syntax)?)),
      SyntaxKind::RefType => Some(AstNode::RefType(RefType::cast(syntax)?)),
      SyntaxKind::TypeDecl => Some(AstNode::TypeDecl(TypeDecl::cast(syntax)?)),
      SyntaxKind::BehaviorDef => Some(AstNode::BehaviorDef(BehaviorDef::cast(syntax)?)),
      SyntaxKind::MethodDef => Some(AstNode::MethodDef(MethodDef::cast(syntax)?)),
      SyntaxKind::MatchBranch => Some(AstNode::MatchBranch(MatchBranch::cast(syntax)?)),
      SyntaxKind::WhenClause => Some(AstNode::WhenClause(WhenClause::cast(syntax)?)),
      SyntaxKind::Param => Some(AstNode::Param(Param::cast(syntax)?)),
      SyntaxKind::Arg => Some(AstNode::Arg(Arg::cast(syntax)?)),
      SyntaxKind::TypeArg => Some(AstNode::TypeArg(TypeArg::cast(syntax)?)),
      SyntaxKind::TypeParam => Some(AstNode::TypeParam(TypeParam::cast(syntax)?)),
      SyntaxKind::Constructor => Some(AstNode::Constructor(Constructor::cast(syntax)?)),
      SyntaxKind::TypeConstructor => Some(AstNode::TypeConstructor(TypeConstructor::cast(syntax)?)),
      SyntaxKind::ConstructorParam => {
        Some(AstNode::ConstructorParam(ConstructorParam::cast(syntax)?))
      }
      SyntaxKind::RequirementField => {
        Some(AstNode::RequirementField(RequirementField::cast(syntax)?))
      }
      SyntaxKind::InExpr => Some(AstNode::InExpr(InExpr::cast(syntax)?)),
      SyntaxKind::ElseClause => Some(AstNode::ElseClause(ElseClause::cast(syntax)?)),
      SyntaxKind::WithClause => Some(AstNode::WithClause(WithClause::cast(syntax)?)),
      _ => None,
    }
  }

  pub fn syntax(&self) -> &SyntaxNode {
    match self {
      AstNode::Literal(n) => n.syntax(),
      AstNode::Ident(n) => n.syntax(),
      AstNode::LetExpr(n) => n.syntax(),
      AstNode::RefBindingExpr(n) => n.syntax(),
      AstNode::FieldAccess(n) => n.syntax(),
      AstNode::MethodCall(n) => n.syntax(),
      AstNode::FnExpr(n) => n.syntax(),
      AstNode::MatchExpr(n) => n.syntax(),
      AstNode::CallExpr(n) => n.syntax(),
      AstNode::ParenExpr(n) => n.syntax(),
      AstNode::WithExpr(n) => n.syntax(),
      AstNode::PrefixExpr(n) => n.syntax(),
      AstNode::BinaryExpr(n) => n.syntax(),
      AstNode::NewExpr(n) => n.syntax(),
      AstNode::TypeExpr(n) => n.syntax(),
      AstNode::TypeApp(n) => n.syntax(),
      AstNode::RefType(n) => n.syntax(),
      AstNode::TypeDecl(n) => n.syntax(),
      AstNode::BehaviorDef(n) => n.syntax(),
      AstNode::MethodDef(n) => n.syntax(),
      AstNode::MatchBranch(n) => n.syntax(),
      AstNode::WhenClause(n) => n.syntax(),
      AstNode::Param(n) => n.syntax(),
      AstNode::Arg(n) => n.syntax(),
      AstNode::TypeArg(n) => n.syntax(),
      AstNode::TypeParam(n) => n.syntax(),
      AstNode::Constructor(n) => n.syntax(),
      AstNode::TypeConstructor(n) => n.syntax(),
      AstNode::ConstructorParam(n) => n.syntax(),
      AstNode::RequirementField(n) => n.syntax(),
      AstNode::InExpr(n) => n.syntax(),
      AstNode::ElseClause(n) => n.syntax(),
      AstNode::WithClause(n) => n.syntax(),
    }
  }

  pub fn is_expr(&self) -> bool {
    matches!(
      self,
      AstNode::LetExpr(_)
        | AstNode::RefBindingExpr(_)
        | AstNode::FieldAccess(_)
        | AstNode::MethodCall(_)
        | AstNode::FnExpr(_)
        | AstNode::MatchExpr(_)
        | AstNode::CallExpr(_)
        | AstNode::ParenExpr(_)
        | AstNode::WithExpr(_)
        | AstNode::PrefixExpr(_)
        | AstNode::BinaryExpr(_)
        | AstNode::NewExpr(_)
        | AstNode::Literal(_)
        | AstNode::Ident(_)
    )
  }

  pub fn is_type(&self) -> bool {
    matches!(
      self,
      AstNode::TypeExpr(_) | AstNode::TypeApp(_) | AstNode::RefType(_)
    )
  }
}
