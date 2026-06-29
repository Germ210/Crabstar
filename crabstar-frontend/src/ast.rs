use crate::{
  syntax::SyntaxKind,
  types::{FreshCounters, Type, TypeArena, TypeCons, TypeID, fn_type, ref_type},
};
use rowan::{SyntaxNode, SyntaxToken};

type Node = SyntaxNode<crate::syntax::CrabstarLang>;
type Token = SyntaxToken<crate::syntax::CrabstarLang>;

fn child_node<N: AstNode>(parent: &Node) -> Option<N> {
  parent.children().find_map(N::cast)
}

fn nth_child_node<N: AstNode>(parent: &Node, n: usize) -> Option<N> {
  parent.children().filter_map(N::cast).nth(n)
}

fn child_token(parent: &Node, kind: SyntaxKind) -> Option<Token> {
  parent
    .children_with_tokens()
    .filter_map(|it| it.into_token())
    .find(|it| it.kind() == kind)
}

fn child_nodes<'a, N: AstNode + 'a>(parent: &'a Node) -> impl Iterator<Item = N> + 'a {
  parent.children().filter_map(N::cast)
}

pub trait AstNode: Sized {
  fn cast(node: Node) -> Option<Self>;
  fn syntax(&self) -> &Node;
}

macro_rules! ast_node {
  ($name:ident, $kind:ident) => {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct $name(Node);

    impl AstNode for $name {
      fn cast(node: Node) -> Option<Self> {
        if node.kind() == SyntaxKind::$kind {
          Some(Self(node))
        } else {
          None
        }
      }
      fn syntax(&self) -> &Node {
        &self.0
      }
    }
  };
}

ast_node!(Root, Root);

impl Root {
  pub fn imports(&self) -> impl Iterator<Item = Import> + '_ {
    child_nodes(&self.0)
  }
  pub fn exports(&self) -> impl Iterator<Item = Export> + '_ {
    child_nodes(&self.0)
  }
  pub fn let_exprs(&self) -> impl Iterator<Item = LetExpr> + '_ {
    child_nodes(&self.0)
  }
  pub fn mut_bindings(&self) -> impl Iterator<Item = MutBindingExpr> + '_ {
    child_nodes(&self.0)
  }
  pub fn ref_bindings(&self) -> impl Iterator<Item = RefBindingExpr> + '_ {
    child_nodes(&self.0)
  }
  pub fn type_decls(&self) -> impl Iterator<Item = TypeDecl> + '_ {
    child_nodes(&self.0)
  }
  pub fn effect_defs(&self) -> impl Iterator<Item = EffectDef> + '_ {
    child_nodes(&self.0)
  }
  pub fn behavior_defs(&self) -> impl Iterator<Item = BehaviorDef> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(Import, Import);

impl Import {
  pub fn path(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::String)
  }
  pub fn with_clause(&self) -> Option<WithClause> {
    child_node(&self.0)
  }
}

ast_node!(WithClause, WithClause);

impl WithClause {
  pub fn alias_list(&self) -> Option<AliasList> {
    child_node(&self.0)
  }
}

ast_node!(AliasList, AliasList);

impl AliasList {
  pub fn aliases(&self) -> impl Iterator<Item = Alias> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(Alias, Alias);

impl Alias {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn alias(&self) -> Option<Token> {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .filter(|it| it.kind() == SyntaxKind::Symbol)
      .nth(1)
  }
}

ast_node!(Export, Export);

impl Export {
  pub fn export_list(&self) -> Option<ExportList> {
    child_node(&self.0)
  }
}

ast_node!(ExportList, ExportList);

impl ExportList {
  pub fn name_tokens(&self) -> impl Iterator<Item = Token> + '_ {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .filter(|it| it.kind() == SyntaxKind::Symbol)
  }
}

ast_node!(LetExpr, LetExpr);

impl LetExpr {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
  pub fn value(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn in_expr(&self) -> Option<Expr> {
    nth_child_node(&self.0, 1)
  }
}

ast_node!(RefBindingExpr, RefBindingExpr);

impl RefBindingExpr {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
  pub fn value(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn body(&self) -> Option<Expr> {
    nth_child_node(&self.0, 1)
  }
}

ast_node!(MutBindingExpr, MutBindingExpr);

impl MutBindingExpr {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
  pub fn value(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn body(&self) -> Option<Expr> {
    nth_child_node(&self.0, 1)
  }
}

ast_node!(WithExpr, WithExpr);

impl WithExpr {
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn behavior(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
  Literal(Literal),
  Ident(Ident),
  QualifiedIdent(QualifiedIdent),
  ParenExpr(ParenExpr),
  BinaryExpr(BinaryExpr),
  AssignmentExpr(AssignmentExpr),
  PrefixExpr(PrefixExpr),
  FieldAccess(FieldAccess),
  MethodCall(MethodCall),
  CallExpr(CallExpr),
  DoThenExpr(DoThenExpr),
  NewExpr(NewExpr),
  CauseExpr(CauseExpr),
  Array(Array),
  FnExpr(FnExpr),
  MatchExpr(MatchExpr),
  DestructureExpr(DestructureExpr),
  LetExpr(LetExpr),
  RefBindingExpr(RefBindingExpr),
  MutBindingExpr(MutBindingExpr),
  WithExpr(WithExpr),
  WithDoExpr(WithDoExpr),
}

impl AstNode for Expr {
  fn cast(node: Node) -> Option<Self> {
    match node.kind() {
      SyntaxKind::Literal => Literal::cast(node.clone()).map(Expr::Literal),
      SyntaxKind::Ident => Ident::cast(node.clone()).map(Expr::Ident),
      SyntaxKind::QualifiedIdent => QualifiedIdent::cast(node.clone()).map(Expr::QualifiedIdent),
      SyntaxKind::ParenExpr => ParenExpr::cast(node.clone()).map(Expr::ParenExpr),
      SyntaxKind::BinaryExpr => BinaryExpr::cast(node.clone()).map(Expr::BinaryExpr),
      SyntaxKind::AssignmentExpr => AssignmentExpr::cast(node.clone()).map(Expr::AssignmentExpr),
      SyntaxKind::PrefixExpr => PrefixExpr::cast(node.clone()).map(Expr::PrefixExpr),
      SyntaxKind::FieldAccess => FieldAccess::cast(node.clone()).map(Expr::FieldAccess),
      SyntaxKind::MethodCall => MethodCall::cast(node.clone()).map(Expr::MethodCall),
      SyntaxKind::CallExpr => CallExpr::cast(node.clone()).map(Expr::CallExpr),
      SyntaxKind::DoThenExpr => DoThenExpr::cast(node.clone()).map(Expr::DoThenExpr),
      SyntaxKind::NewExpr => NewExpr::cast(node.clone()).map(Expr::NewExpr),
      SyntaxKind::CauseExpr => CauseExpr::cast(node.clone()).map(Expr::CauseExpr),
      SyntaxKind::Array => Array::cast(node.clone()).map(Expr::Array),
      SyntaxKind::FnExpr => FnExpr::cast(node.clone()).map(Expr::FnExpr),
      SyntaxKind::MatchExpr => MatchExpr::cast(node.clone()).map(Expr::MatchExpr),
      SyntaxKind::DestructureExpr => DestructureExpr::cast(node.clone()).map(Expr::DestructureExpr),
      SyntaxKind::LetExpr => LetExpr::cast(node.clone()).map(Expr::LetExpr),
      SyntaxKind::RefBindingExpr => RefBindingExpr::cast(node.clone()).map(Expr::RefBindingExpr),
      SyntaxKind::MutBindingExpr => MutBindingExpr::cast(node.clone()).map(Expr::MutBindingExpr),
      SyntaxKind::WithExpr => WithExpr::cast(node.clone()).map(Expr::WithExpr),
      SyntaxKind::WithDoExpr => WithDoExpr::cast(node.clone()).map(Expr::WithDoExpr),
      _ => None,
    }
  }
  fn syntax(&self) -> &Node {
    match self {
      Expr::Literal(n) => n.syntax(),
      Expr::Ident(n) => n.syntax(),
      Expr::QualifiedIdent(n) => n.syntax(),
      Expr::ParenExpr(n) => n.syntax(),
      Expr::BinaryExpr(n) => n.syntax(),
      Expr::AssignmentExpr(n) => n.syntax(),
      Expr::PrefixExpr(n) => n.syntax(),
      Expr::FieldAccess(n) => n.syntax(),
      Expr::MethodCall(n) => n.syntax(),
      Expr::CallExpr(n) => n.syntax(),
      Expr::DoThenExpr(n) => n.syntax(),
      Expr::NewExpr(n) => n.syntax(),
      Expr::CauseExpr(n) => n.syntax(),
      Expr::Array(n) => n.syntax(),
      Expr::FnExpr(n) => n.syntax(),
      Expr::MatchExpr(n) => n.syntax(),
      Expr::DestructureExpr(n) => n.syntax(),
      Expr::LetExpr(n) => n.syntax(),
      Expr::RefBindingExpr(n) => n.syntax(),
      Expr::MutBindingExpr(n) => n.syntax(),
      Expr::WithExpr(n) => n.syntax(),
      Expr::WithDoExpr(n) => n.syntax(),
    }
  }
}

ast_node!(Literal, Literal);

impl Literal {
  pub fn token(&self) -> Option<Token> {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .find(|it| {
        matches!(
          it.kind(),
          SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::String
            | SyntaxKind::KwNull
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
        )
      })
  }
}

ast_node!(Ident, Ident);

impl Ident {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
}

ast_node!(QualifiedIdent, QualifiedIdent);

impl QualifiedIdent {
  pub fn segments(&self) -> impl Iterator<Item = Token> + '_ {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .filter(|it| it.kind() == SyntaxKind::Symbol)
  }
}

ast_node!(ParenExpr, ParenExpr);

impl ParenExpr {
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(BinaryExpr, BinaryExpr);

impl BinaryExpr {
  pub fn lhs(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn rhs(&self) -> Option<Expr> {
    nth_child_node(&self.0, 1)
  }
  pub fn op(&self) -> Option<Token> {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .find(|it| {
        matches!(
          it.kind(),
          SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Eq
            | SyntaxKind::NotEq
            | SyntaxKind::Lt
            | SyntaxKind::Gt
            | SyntaxKind::LtEq
            | SyntaxKind::GtEq
            | SyntaxKind::KwOr
            | SyntaxKind::KwAnd
        )
      })
  }
}

ast_node!(AssignmentExpr, AssignmentExpr);

impl AssignmentExpr {
  pub fn lhs(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn rhs(&self) -> Option<Expr> {
    nth_child_node(&self.0, 1)
  }
}

ast_node!(PrefixExpr, PrefixExpr);

impl PrefixExpr {
  pub fn op(&self) -> Option<Token> {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .find(|it| {
        matches!(
          it.kind(),
          SyntaxKind::Minus
            | SyntaxKind::KwNot
            | SyntaxKind::KwRef
            | SyntaxKind::KwMut
            | SyntaxKind::KwOwned
        )
      })
  }
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(FieldAccess, FieldAccess);

impl FieldAccess {
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn field(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
}

ast_node!(MethodCall, MethodCall);

impl MethodCall {
  pub fn receiver(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn method(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn qualified_ident(&self) -> Option<QualifiedIdent> {
    child_node(&self.0)
  }
  pub fn arg_list(&self) -> Option<ArgList> {
    child_node(&self.0)
  }
}

ast_node!(CallExpr, CallExpr);

impl CallExpr {
  pub fn callee(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn arg_list(&self) -> Option<ArgList> {
    child_node(&self.0)
  }
}

ast_node!(ArgList, ArgList);

impl ArgList {
  pub fn args(&self) -> impl Iterator<Item = Arg> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(Arg, Arg);

impl Arg {
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(DoThenExpr, DoThenExpr);

impl DoThenExpr {
  pub fn do_expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn then_expr(&self) -> Option<Expr> {
    nth_child_node(&self.0, 1)
  }
}

ast_node!(NewExpr, NewExpr);

impl NewExpr {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn fields(&self) -> impl Iterator<Item = StructField> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(StructField, StructField);

impl StructField {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn value(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(CauseExpr, CauseExpr);

impl CauseExpr {
  pub fn new_expr(&self) -> Option<NewExpr> {
    child_node(&self.0)
  }
}

ast_node!(Array, Array);

impl Array {
  pub fn elements(&self) -> impl Iterator<Item = ArrayElement> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(ArrayElement, ArrayElement);

impl ArrayElement {
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(FnExpr, FnExpr);

impl FnExpr {
  pub fn params(&self) -> impl Iterator<Item = Param> + '_ {
    child_nodes(&self.0)
  }
  pub fn return_type(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
  pub fn uses_clause(&self) -> Option<UsesClause> {
    child_node(&self.0)
  }
  pub fn body(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(Param, Param);

impl Param {
  pub fn modifier(&self) -> Option<Token> {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .find(|it| {
        matches!(
          it.kind(),
          SyntaxKind::KwRef | SyntaxKind::KwMut | SyntaxKind::KwOwned
        )
      })
  }
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
}

ast_node!(UsesClause, UsesClause);

impl UsesClause {
  pub fn uses_list(&self) -> Option<UsesList> {
    child_node(&self.0)
  }
}

ast_node!(UsesList, UsesList);

impl UsesList {
  pub fn name_tokens(&self) -> impl Iterator<Item = Token> + '_ {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .filter(|it| it.kind() == SyntaxKind::Symbol)
  }
}

ast_node!(WithDoExpr, WithDoExpr);
impl WithDoExpr {
  pub fn effect_list(&self) -> Option<EffectNameList> {
    child_node(&self.0)
  }
  pub fn override_list(&self) -> Option<OverrideList> {
    child_node(&self.0)
  }
  pub fn body(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(EffectNameList, EffectNameList);
impl EffectNameList {
  pub fn effects(&self) -> impl Iterator<Item = Token> + '_ {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .filter(|it| it.kind() == SyntaxKind::Symbol)
  }
}

ast_node!(OverrideList, OverrideList);
impl OverrideList {
  pub fn entries(&self) -> impl Iterator<Item = OverrideEntry> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(OverrideEntry, OverrideEntry);
impl OverrideEntry {
  pub fn handler(&self) -> Option<QualifiedIdent> {
    child_node(&self.0)
  }
  pub fn value(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(MatchExpr, MatchExpr);

impl MatchExpr {
  pub fn scrutinee(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn type_annotation(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
  pub fn branches(&self) -> impl Iterator<Item = MatchBranch> + '_ {
    child_nodes(&self.0)
  }
  pub fn else_clause(&self) -> Option<ElseClause> {
    child_node(&self.0)
  }
}

ast_node!(MatchBranch, MatchBranch);

impl MatchBranch {
  pub fn pattern(&self) -> Option<Pattern> {
    child_node(&self.0)
  }
  pub fn when_clause(&self) -> Option<WhenClause> {
    child_node(&self.0)
  }
  pub fn body(&self) -> Option<Expr> {
    self.0.children().filter_map(Expr::cast).last()
  }
}

ast_node!(WhenClause, WhenClause);

impl WhenClause {
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(ElseClause, ElseClause);

impl ElseClause {
  pub fn body(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pattern {
  DestructureStruct(DestructureStruct),
  Literal(Literal),
  VariantPattern(VariantPattern),
  Ident(Ident),
}

impl AstNode for Pattern {
  fn cast(node: Node) -> Option<Self> {
    match node.kind() {
      SyntaxKind::DestructureStruct => {
        DestructureStruct::cast(node).map(Pattern::DestructureStruct)
      }
      SyntaxKind::Literal => Literal::cast(node).map(Pattern::Literal),
      SyntaxKind::VariantPattern => VariantPattern::cast(node).map(Pattern::VariantPattern),
      SyntaxKind::Ident => Ident::cast(node).map(Pattern::Ident),
      _ => None,
    }
  }
  fn syntax(&self) -> &Node {
    match self {
      Pattern::DestructureStruct(n) => n.syntax(),
      Pattern::Literal(n) => n.syntax(),
      Pattern::VariantPattern(n) => n.syntax(),
      Pattern::Ident(n) => n.syntax(),
    }
  }
}

ast_node!(VariantPattern, VariantPattern);

impl VariantPattern {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn binding_list(&self) -> Option<VariantBindingList> {
    child_node(&self.0)
  }
}

ast_node!(VariantBindingList, VariantBindingList);

impl VariantBindingList {
  pub fn bindings(&self) -> impl Iterator<Item = VariantBinding> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(VariantBinding, VariantBinding);

impl VariantBinding {
  pub fn pattern(&self) -> Option<Pattern> {
    child_node(&self.0)
  }
}

ast_node!(DestructureExpr, DestructureExpr);

impl DestructureExpr {
  pub fn destructure_struct(&self) -> Option<DestructureStruct> {
    child_node(&self.0)
  }
  pub fn expr(&self) -> Option<Expr> {
    child_node(&self.0)
  }
  pub fn else_clause(&self) -> Option<ElseClause> {
    child_node(&self.0)
  }
  pub fn body(&self) -> Option<Expr> {
    nth_child_node(&self.0, 1)
  }
}

ast_node!(DestructureStruct, DestructureStruct);

impl DestructureStruct {
  pub fn fields(&self) -> impl Iterator<Item = DestructureField> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(DestructureField, DestructureField);

impl DestructureField {
  pub fn modifier(&self) -> Option<Token> {
    self
      .0
      .children_with_tokens()
      .filter_map(|it| it.into_token())
      .find(|it| {
        matches!(
          it.kind(),
          SyntaxKind::KwLet | SyntaxKind::KwRef | SyntaxKind::KwMut
        )
      })
  }
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn value(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeExpr {
  RefType(RefType),
  FnType(FnType),
  TypeApp(TypeApp),
}

impl TypeExpr {
  pub fn lower(&self, arena: &mut TypeArena, counters: &mut FreshCounters) -> TypeID {
    match self {
      TypeExpr::TypeApp(app) => {
        let name = app.name().unwrap().text().to_string();
        if let Some(arg_list) = app.arg_list() {
          let args = arg_list
            .args()
            .filter_map(|a| a.type_expr())
            .map(|t| t.lower(arena, counters))
            .collect();
          let head = arena.alloc(Type::TypeCons(TypeCons::new(name)));
          arena.alloc(Type::TypeApp(crate::types::TypeApp::new(head, args)))
        } else {
          arena.alloc(Type::TypeCons(TypeCons::new(name)))
        }
      }
      TypeExpr::RefType(r) => {
        let inner = r
          .inner()
          .map(|t| TypeExpr::TypeApp(t).lower(arena, counters))
          .unwrap_or_else(|| arena.alloc(Type::Error));
        ref_type(arena, inner)
      }
      TypeExpr::FnType(f) => {
        let params = f
          .params()
          .filter_map(|p| p.type_expr())
          .map(|t| t.lower(arena, counters))
          .collect();
        let ret = f
          .return_type()
          .map(|t| t.lower(arena, counters))
          .unwrap_or_else(|| arena.alloc(Type::Error));
        fn_type(arena, params, ret)
      }
    }
  }
}

impl AstNode for TypeExpr {
  fn cast(node: Node) -> Option<Self> {
    match node.kind() {
      SyntaxKind::TypeExpr => {
        let inner = node.children().next()?;
        match inner.kind() {
          SyntaxKind::RefType => RefType::cast(inner).map(TypeExpr::RefType),
          SyntaxKind::FnType => FnType::cast(inner).map(TypeExpr::FnType),
          SyntaxKind::TypeApp => TypeApp::cast(inner).map(TypeExpr::TypeApp),
          _ => None,
        }
      }
      _ => None,
    }
  }
  fn syntax(&self) -> &Node {
    match self {
      TypeExpr::RefType(n) => n.syntax(),
      TypeExpr::FnType(n) => n.syntax(),
      TypeExpr::TypeApp(n) => n.syntax(),
    }
  }
}

ast_node!(RefType, RefType);

impl RefType {
  pub fn inner(&self) -> Option<TypeApp> {
    child_node(&self.0)
  }
}

ast_node!(FnType, FnType);

impl FnType {
  pub fn params(&self) -> impl Iterator<Item = Param> + '_ {
    child_nodes(&self.0)
  }
  pub fn return_type(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
}

ast_node!(TypeApp, TypeApp);

impl TypeApp {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn arg_list(&self) -> Option<TypeArgList> {
    child_node(&self.0)
  }
}

ast_node!(TypeArgList, TypeArgList);

impl TypeArgList {
  pub fn args(&self) -> impl Iterator<Item = TypeArg> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(TypeArg, TypeArg);

impl TypeArg {
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
}

ast_node!(TypeDecl, TypeDecl);

impl TypeDecl {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn type_param_list(&self) -> Option<TypeParamList> {
    child_node(&self.0)
  }
  pub fn struct_body(&self) -> Option<StructFieldList> {
    child_node(&self.0)
  }
  pub fn variant_body(&self) -> Option<ConstructorList> {
    child_node(&self.0)
  }
}

ast_node!(TypeParamList, TypeParamList);

impl TypeParamList {
  pub fn params(&self) -> impl Iterator<Item = TypeParam> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(TypeParam, TypeParam);

impl TypeParam {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
}

ast_node!(StructFieldList, StructFieldList);

impl StructFieldList {
  pub fn fields(&self) -> impl Iterator<Item = StructTypeField> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(StructTypeField, StructField);

impl StructTypeField {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
}

ast_node!(Constructor, Constructor);

impl Constructor {
  pub fn type_constructor(&self) -> Option<TypeConstructor> {
    child_node(&self.0)
  }
}

ast_node!(ConstructorList, ConstructorList);

impl ConstructorList {
  pub fn constructors(&self) -> impl Iterator<Item = TypeConstructor> + '_ {
    child_nodes::<Constructor>(&self.0).filter_map(|c| c.type_constructor())
  }
}

ast_node!(TypeConstructor, TypeConstructor);

impl TypeConstructor {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn param_list(&self) -> Option<ConstructorParamList> {
    child_node(&self.0)
  }
}

ast_node!(ConstructorParamList, ConstructorParamList);

impl ConstructorParamList {
  pub fn params(&self) -> impl Iterator<Item = ConstructorParam> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(ConstructorParam, ConstructorParam);

impl ConstructorParam {
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
}

ast_node!(EffectDef, EffectDef);

impl EffectDef {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn method_list(&self) -> Option<MethodList> {
    child_node(&self.0)
  }
}

ast_node!(MethodList, MethodList);

impl MethodList {
  pub fn methods(&self) -> impl Iterator<Item = MethodDef> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(MethodDef, MethodDef);

impl MethodDef {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn params(&self) -> impl Iterator<Item = Param> + '_ {
    child_nodes(&self.0)
  }
  pub fn return_type(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
  pub fn uses_clause(&self) -> Option<UsesClause> {
    child_node(&self.0)
  }
  pub fn body(&self) -> Option<Expr> {
    child_node(&self.0)
  }
}

ast_node!(BehaviorDef, BehaviorDef);

impl BehaviorDef {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn requirement_list(&self) -> Option<RequirementList> {
    child_node(&self.0)
  }
  pub fn method_list(&self) -> Option<MethodList> {
    child_node(&self.0)
  }
}

ast_node!(RequirementList, RequirementList);

impl RequirementList {
  pub fn fields(&self) -> impl Iterator<Item = RequirementField> + '_ {
    child_nodes(&self.0)
  }
}

ast_node!(RequirementField, RequirementField);

impl RequirementField {
  pub fn name(&self) -> Option<Token> {
    child_token(&self.0, SyntaxKind::Symbol)
  }
  pub fn type_expr(&self) -> Option<TypeExpr> {
    child_node(&self.0)
  }
}
