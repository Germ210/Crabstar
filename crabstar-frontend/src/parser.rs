use rowan::GreenNodeBuilder;

use crate::{
  err::{Expected, ParseErr, Reason, Span},
  lexer::{Lexer, Token},
  syntax::SyntaxKind,
};

#[derive(Debug)]
enum Event {
  Open { kind: SyntaxKind },
  Close,
  Advance,
}

struct MarkOpened {
  index: usize,
}

struct MarkClosed {
  index: usize,
}

pub struct Parser {
  tokens: Vec<Token>,
  pos: usize,
  events: Vec<Event>,
  pub errs: Vec<ParseErr<SyntaxKind>>,
}

impl Parser {
  pub fn new(src: &str) -> Parser {
    let tokens = Lexer::new(src).tokenize();
    Parser {
      tokens,
      pos: 0,
      events: Vec::new(),
      errs: Vec::new(),
    }
  }

  fn current_span(&self) -> Span {
    if self.pos < self.tokens.len() {
      self.tokens[self.pos].span.clone()
    } else {
      let end = self.tokens.last().map_or(0, |t| t.span.end);
      end..end
    }
  }

  fn open(&mut self) -> MarkOpened {
    let mark = MarkOpened {
      index: self.events.len(),
    };
    self.events.push(Event::Open {
      kind: SyntaxKind::Error,
    });
    mark
  }

  fn open_before(&mut self, m: MarkClosed) -> MarkOpened {
    let mark = MarkOpened { index: m.index };
    self.events.insert(
      m.index,
      Event::Open {
        kind: SyntaxKind::Error,
      },
    );
    mark
  }

  fn close(&mut self, m: MarkOpened, kind: SyntaxKind) -> MarkClosed {
    self.events[m.index] = Event::Open { kind };
    self.events.push(Event::Close);
    MarkClosed { index: m.index }
  }

  fn advance(&mut self) {
    self.bump_raw();
  }

  fn advance_with_error(&mut self, error: &str) {
    self.errs.push(ParseErr {
      span: self.current_span(),
      reason: Box::new(Reason::Custom(error.to_string())),
      context: Vec::new(),
    });
    let m = self.open();
    self.bump_raw();
    self.close(m, SyntaxKind::Error);
  }

  fn bump_raw(&mut self) {
    while self.pos < self.tokens.len() && Self::is_trivia(self.tokens[self.pos].kind) {
      self.events.push(Event::Advance);
      self.pos += 1;
    }
    if !self.eof() {
      self.events.push(Event::Advance);
      self.pos += 1;
    }
  }

  pub fn build_tree(self) -> rowan::GreenNode {
    let mut builder = GreenNodeBuilder::new();
    let mut tokens = self.tokens.into_iter();
    let mut events = self.events.into_iter();

    while let Some(event) = events.next() {
      match event {
        Event::Open { kind } => builder.start_node(kind.into()),
        Event::Close => builder.finish_node(),
        Event::Advance => {
          let token = tokens.next().expect("event/token mismatch");
          builder.token(token.kind.into(), &token.text);
        }
      }
    }

    builder.finish()
  }

  fn eof(&self) -> bool {
    self.pos == self.tokens.len()
  }

  fn nth(&self, lookahead: usize) -> SyntaxKind {
    let mut i = self.pos;
    let mut seen = 0;
    while i < self.tokens.len() {
      let kind = self.tokens[i].kind;
      if !Self::is_trivia(kind) {
        if seen == lookahead {
          return kind;
        }
        seen += 1;
      }
      i += 1;
    }
    SyntaxKind::Eof
  }

  fn peek(&self) -> SyntaxKind {
    self.nth(0)
  }

  fn at(&self, kind: SyntaxKind) -> bool {
    self.nth(0) == kind
  }

  fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
    kinds.contains(&self.nth(0))
  }

  fn eat(&mut self, kind: SyntaxKind) -> bool {
    if self.at(kind) {
      self.advance();
      true
    } else {
      false
    }
  }

  fn expect(&mut self, kind: SyntaxKind) {
    if !self.eat(kind) {
      self.errs.push(ParseErr {
        span: self.current_span(),
        reason: Box::new(Reason::Expected {
          found: if self.eof() { None } else { Some(self.peek()) },
          expected: vec![Expected::Token(kind)],
        }),
        context: Vec::new(),
      });
    }
  }

  fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
  }
}

fn parse_atom(parser: &mut Parser) -> MarkClosed {
  let m = parser.open();
  let mut lhs = match parser.peek() {
    SyntaxKind::Int | SyntaxKind::Float | SyntaxKind::String | SyntaxKind::KwNull => {
      parser.advance();
      parser.close(m, SyntaxKind::Literal)
    }
    SyntaxKind::Symbol => {
      if let SyntaxKind::Backslash = parser.nth(1) {
        parse_qualified_ident(parser, m)
      } else {
        parser.advance();
        parser.close(m, SyntaxKind::Ident)
      }
    }
    SyntaxKind::LParen => {
      parser.advance();
      parse_expr_prec(parser, 0);
      parser.expect(SyntaxKind::RParen);
      parser.close(m, SyntaxKind::ParenExpr)
    }
    SyntaxKind::KwDo => parse_do_then(parser, m),
    SyntaxKind::KwNew => parse_new_expr(parser, m),
    SyntaxKind::KwCause => {
      parser.advance();
      let m2 = parser.open();
      parse_new_expr(parser, m2);
      parser.close(m, SyntaxKind::CauseExpr)
    }
    SyntaxKind::LBracket => parse_array(parser, m),
    SyntaxKind::KwFn => parse_fn(parser, m),
    SyntaxKind::KwMatch => parse_match(parser, m),
    SyntaxKind::LBrace => parse_destructure_in(parser, m),
    SyntaxKind::KwWith => parse_with_do(parser, m),
    _ => {
      parser.advance_with_error("expected expression");
      parser.close(m, SyntaxKind::Error)
    }
  };

  loop {
    match parser.peek() {
      SyntaxKind::Dot => {
        let m = parser.open_before(lhs);
        parser.advance();
        parser.expect(SyntaxKind::Symbol);
        lhs = parser.close(m, SyntaxKind::FieldAccess);
      }
      SyntaxKind::Pipe => {
        let m = parser.open_before(lhs);
        parser.advance();
        if parser.at(SyntaxKind::Symbol) && parser.nth(1) == SyntaxKind::Backslash {
          let m2 = parser.open();
          parse_qualified_ident(parser, m2);
        } else {
          parser.expect(SyntaxKind::Symbol);
        }
        parse_call_args(parser);
        lhs = parser.close(m, SyntaxKind::MethodCall);
      }
      SyntaxKind::LParen => {
        let m = parser.open_before(lhs);
        parse_call_args(parser);
        lhs = parser.close(m, SyntaxKind::CallExpr);
      }
      SyntaxKind::KwWith => {
        let m = parser.open_before(lhs);
        parser.advance();
        parser.expect(SyntaxKind::Symbol);
        lhs = parser.close(m, SyntaxKind::WithExpr);
      }
      _ => break,
    }
  }

  lhs
}

fn parse_call_args(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::LParen);
  if !parser.at(SyntaxKind::RParen) {
    let arg_m = parser.open();
    parse_expr(parser);
    parser.close(arg_m, SyntaxKind::Arg);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RParen) {
        break;
      }
      let arg_m = parser.open();
      parse_expr(parser);
      parser.close(arg_m, SyntaxKind::Arg);
    }
  }
  parser.expect(SyntaxKind::RParen);
  parser.close(m, SyntaxKind::ArgList);
}

fn precedence(k: SyntaxKind) -> Option<(u8, bool)> {
  match k {
    SyntaxKind::KwOr => Some((1, true)),
    SyntaxKind::KwAnd => Some((2, true)),
    SyntaxKind::Eq | SyntaxKind::NotEq => Some((3, true)),
    SyntaxKind::Lt | SyntaxKind::Gt | SyntaxKind::LtEq | SyntaxKind::GtEq => Some((4, true)),
    SyntaxKind::Plus | SyntaxKind::Minus => Some((5, true)),
    SyntaxKind::Star | SyntaxKind::Slash => Some((6, true)),
    SyntaxKind::LeftArrow => Some((0, false)),
    _ => None,
  }
}

fn parse_prefix(parser: &mut Parser) -> MarkClosed {
  match parser.peek() {
    SyntaxKind::Minus
    | SyntaxKind::KwNot
    | SyntaxKind::KwRef
    | SyntaxKind::KwMut
    | SyntaxKind::KwOwned => {
      let m = parser.open();
      parser.advance();
      parse_prefix(parser);
      parser.close(m, SyntaxKind::PrefixExpr)
    }
    _ => parse_atom(parser),
  }
}

fn parse_expr_prec(parser: &mut Parser, min_prec: u8) {
  let mut lhs = parse_prefix(parser);
  loop {
    let op = parser.peek();
    let Some((prec, left_assoc)) = precedence(op) else {
      break;
    };
    if prec <= min_prec {
      break;
    }
    let m = parser.open_before(lhs);
    parser.advance();
    let next_prec = if left_assoc { prec } else { prec - 1 };
    parse_expr_prec(parser, next_prec);
    let kind = if op == SyntaxKind::LeftArrow {
      SyntaxKind::AssignmentExpr
    } else {
      SyntaxKind::BinaryExpr
    };
    lhs = parser.close(m, kind);
  }
}

fn parse_expr(parser: &mut Parser) {
  parse_expr_prec(parser, 0);
}

fn parse_let(parser: &mut Parser) {
  parse_binding(parser, SyntaxKind::KwLet, SyntaxKind::LetExpr);
}

fn parse_ref(parser: &mut Parser) {
  parse_binding(parser, SyntaxKind::KwRef, SyntaxKind::RefBindingExpr);
}

fn parse_mut(parser: &mut Parser) {
  parse_binding(parser, SyntaxKind::KwMut, SyntaxKind::MutBindingExpr);
}

fn parse_binding_or_expr(parser: &mut Parser) {
  match parser.peek() {
    SyntaxKind::KwRef => parse_ref(parser),
    SyntaxKind::KwLet => parse_let(parser),
    SyntaxKind::KwMut => parse_mut(parser),
    SyntaxKind::LBrace => {
      let m = parser.open();
      parse_destructure_in(parser, m);
    }
    _ => parse_expr(parser),
  }
}

fn parse_do_then(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  assert!(parser.at(SyntaxKind::KwDo));
  parser.advance();
  parse_expr(parser);
  parser.expect(SyntaxKind::KwThen);
  parse_expr(parser);
  parser.close(m, SyntaxKind::DoThenExpr)
}

fn parse_binding(parser: &mut Parser, binding_kind: SyntaxKind, expr_kind: SyntaxKind) {
  assert!(parser.at(binding_kind));
  let m = parser.open();
  parser.expect(binding_kind);
  parser.expect(SyntaxKind::Symbol);
  if parser.at(SyntaxKind::Arrow) {
    parser.advance();
    parse_type_expr(parser);
  }
  parser.expect(SyntaxKind::Colon);
  parse_expr(parser);
  if parser.at(SyntaxKind::KwIn) {
    parser.advance();
    parse_binding_or_expr(parser);
  }
  parser.close(m, expr_kind);
}

fn parse_qualified_ident(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  assert!(parser.at(SyntaxKind::Symbol));
  parser.advance();
  parser.expect(SyntaxKind::Backslash);
  parser.expect(SyntaxKind::Symbol);
  while parser.at(SyntaxKind::Backslash) {
    parser.advance();
    parser.expect(SyntaxKind::Symbol);
  }
  parser.close(m, SyntaxKind::QualifiedIdent)
}

fn parse_new_expr(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  assert!(parser.at(SyntaxKind::KwNew));
  parser.advance();
  parser.expect(SyntaxKind::Symbol);
  parser.expect(SyntaxKind::LParen);
  if parser.at(SyntaxKind::Symbol) {
    let m = parser.open();
    parser.advance();
    parser.expect(SyntaxKind::Eq);
    parse_expr(parser);
    parser.close(m, SyntaxKind::StructField);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RParen) {
        break;
      }
      let m = parser.open();
      parser.advance();
      parser.expect(SyntaxKind::Eq);
      parse_expr(parser);
      parser.close(m, SyntaxKind::StructField);
    }
  }
  parser.expect(SyntaxKind::RParen);
  parser.close(m, SyntaxKind::NewExpr)
}

fn parse_array(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  assert!(parser.at(SyntaxKind::LBracket));
  parser.advance();

  if parser.at(SyntaxKind::RBracket) {
    parser.advance();
    return parser.close(m, SyntaxKind::Array);
  }

  let mut element_mark = parser.open();
  parse_expr(parser);

  loop {
    match parser.peek() {
      SyntaxKind::Comma => {
        parser.advance();
        parser.close(element_mark, SyntaxKind::ArrayElement);
        if parser.at(SyntaxKind::RBracket) {
          parser.advance();
          break;
        }
        element_mark = parser.open();
        parse_expr(parser);
      }
      SyntaxKind::RBracket => {
        parser.advance();
        parser.close(element_mark, SyntaxKind::ArrayElement);
        break;
      }
      _ => {
        parser.errs.push(ParseErr {
          span: parser.current_span(),
          reason: Box::new(Reason::Custom(
            "unexpected token in array, expected ',' or ']'".to_string(),
          )),
          context: Vec::new(),
        });
        parser.close(element_mark, SyntaxKind::Error);
        while !parser.at(SyntaxKind::RBracket) && !parser.eof() {
          parser.advance();
        }
        if parser.at(SyntaxKind::RBracket) {
          parser.advance();
        }
        break;
      }
    }
  }

  parser.close(m, SyntaxKind::Array)
}

fn parse_fn(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  assert!(parser.at(SyntaxKind::KwFn));
  parser.advance();
  parser.expect(SyntaxKind::LParen);
  if parser.at(SyntaxKind::RParen) {
    parser.advance();
  } else {
    parse_param(parser);
    loop {
      match parser.peek() {
        SyntaxKind::Comma => {
          parser.advance();
          if parser.at(SyntaxKind::RParen) {
            parser.advance();
            break;
          }
          parse_param(parser);
        }
        SyntaxKind::RParen => {
          parser.advance();
          break;
        }
        _ => {
          parser.errs.push(ParseErr {
            span: parser.current_span(),
            reason: Box::new(Reason::Custom(
              "unexpected token in parameter list, expected ',' or ')'".to_string(),
            )),
            context: Vec::new(),
          });
          while !parser.at(SyntaxKind::RParen) && !parser.eof() {
            parser.advance();
          }
          if parser.at(SyntaxKind::RParen) {
            parser.advance();
          }
          break;
        }
      }
    }
  }
  if parser.at(SyntaxKind::Arrow) {
    parser.advance();
    parse_type_expr(parser);
  }
  if parser.at(SyntaxKind::KwUses) {
    let uses_m = parser.open();
    parser.advance();
    let list_m = parser.open();
    parser.expect(SyntaxKind::Symbol);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      parser.expect(SyntaxKind::Symbol);
    }
    parser.close(list_m, SyntaxKind::UsesList);
    parser.close(uses_m, SyntaxKind::UsesClause);
  }
  parser.expect(SyntaxKind::Colon);
  parse_binding_or_expr(parser);
  parser.close(m, SyntaxKind::FnExpr)
}

fn parse_param(parser: &mut Parser) {
  let m = parser.open();
  if parser.at_any(&[SyntaxKind::KwRef, SyntaxKind::KwMut, SyntaxKind::KwOwned]) {
    parser.advance();
  }
  parser.expect(SyntaxKind::Symbol);
  if parser.at(SyntaxKind::Colon) {
    parser.advance();
    parse_type_expr(parser);
  }
  parser.close(m, SyntaxKind::Param);
}

fn parse_destructure_field(parser: &mut Parser) {
  let m = parser.open();
  if !parser.at_any(&[SyntaxKind::KwLet, SyntaxKind::KwRef, SyntaxKind::KwMut]) {
    parser.advance_with_error("expected 'let', 'mut' or 'ref'");
    parser.close(m, SyntaxKind::Error);
    return;
  }
  parser.advance();
  parser.expect(SyntaxKind::Symbol);
  parser.expect(SyntaxKind::LeftArrow);
  parse_expr(parser);
  parser.close(m, SyntaxKind::DestructureField);
}

fn parse_destructure(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::LBrace);
  parse_destructure_field(parser);
  while parser.at(SyntaxKind::Comma) {
    parser.advance();
    parse_destructure_field(parser);
  }
  parser.expect(SyntaxKind::RBrace);
  parser.close(m, SyntaxKind::DestructureStruct);
}

fn parse_destructure_in(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  parse_destructure(parser);
  parser.expect(SyntaxKind::Colon);
  parse_expr(parser);
  if parser.at(SyntaxKind::KwElse) {
    let else_m = parser.open();
    parser.advance();
    parser.expect(SyntaxKind::Colon);
    let cause_m = parser.open();
    parser.advance();
    let new_m = parser.open();
    parse_new_expr(parser, new_m);
    parser.close(cause_m, SyntaxKind::CauseExpr);
    parser.close(else_m, SyntaxKind::ElseClause);
  }
  if parser.at(SyntaxKind::KwIn) {
    parser.advance();
    parse_binding_or_expr(parser);
  }
  parser.close(m, SyntaxKind::DestructureExpr)
}

fn parse_pattern(parser: &mut Parser) {
  match parser.peek() {
    SyntaxKind::LBrace => parse_destructure(parser),
    SyntaxKind::Int
    | SyntaxKind::Float
    | SyntaxKind::String
    | SyntaxKind::KwTrue
    | SyntaxKind::KwFalse => {
      let m = parser.open();
      parser.advance();
      parser.close(m, SyntaxKind::Literal);
    }
    SyntaxKind::Symbol => {
      let m = parser.open();
      parser.advance();
      if parser.at(SyntaxKind::LParen) {
        parser.advance();
        let bindings_m = parser.open();
        if !parser.at(SyntaxKind::RParen) {
          let b = parser.open();
          parse_pattern(parser);
          parser.close(b, SyntaxKind::VariantBinding);
          while parser.at(SyntaxKind::Comma) {
            parser.advance();
            if parser.at(SyntaxKind::RParen) {
              break;
            }
            let b = parser.open();
            parse_pattern(parser);
            parser.close(b, SyntaxKind::VariantBinding);
          }
        }
        parser.close(bindings_m, SyntaxKind::VariantBindingList);
        parser.expect(SyntaxKind::RParen);
        parser.close(m, SyntaxKind::VariantPattern);
      } else {
        parser.close(m, SyntaxKind::Ident);
      }
    }
    _ => {
      parser.advance_with_error("expected pattern");
    }
  }
}

fn parse_match_branch(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwOf);
  parse_pattern(parser);
  if parser.at(SyntaxKind::KwWhen) {
    let when_m = parser.open();
    parser.advance();
    parse_expr(parser);
    parser.close(when_m, SyntaxKind::WhenClause);
  }
  parser.expect(SyntaxKind::Colon);
  parse_binding_or_expr(parser);
  parser.close(m, SyntaxKind::MatchBranch);
}

fn parse_match(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  assert!(parser.at(SyntaxKind::KwMatch));
  parser.advance();
  parse_expr(parser);
  if parser.at(SyntaxKind::Colon) {
    parser.advance();
    parse_type_expr(parser);
  }
  parser.expect(SyntaxKind::LBrace);
  while parser.at(SyntaxKind::KwOf) {
    parse_match_branch(parser);
  }
  parser.expect(SyntaxKind::RBrace);
  if parser.at(SyntaxKind::KwElse) {
    let else_m = parser.open();
    parser.advance();
    parser.expect(SyntaxKind::Colon);
    parse_binding_or_expr(parser);
    parser.close(else_m, SyntaxKind::ElseClause);
  }
  parser.close(m, SyntaxKind::MatchExpr)
}

fn parse_with_do(parser: &mut Parser, m: MarkOpened) -> MarkClosed {
  assert!(parser.at(SyntaxKind::KwWith));
  parser.advance();

  let list_m = parser.open();
  parser.expect(SyntaxKind::Symbol);
  while parser.at(SyntaxKind::Comma) {
    parser.advance();
    parser.expect(SyntaxKind::Symbol);
  }
  parser.close(list_m, SyntaxKind::EffectNameList);

  parser.expect(SyntaxKind::Eq);

  parser.expect(SyntaxKind::LBrace);
  let overrides_m = parser.open();
  if !parser.at(SyntaxKind::RBrace) {
    let entry_m = parser.open();
    let m = parser.open();
    parse_qualified_ident(parser, m);
    parser.expect(SyntaxKind::Eq);
    parse_expr(parser);
    parser.close(entry_m, SyntaxKind::OverrideEntry);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RBrace) {
        break;
      }
      let entry_m = parser.open();
      let m = parser.open();
      parse_qualified_ident(parser, m);
      parser.expect(SyntaxKind::Eq);
      parse_expr(parser);
      parser.close(entry_m, SyntaxKind::OverrideEntry);
    }
  }
  parser.close(overrides_m, SyntaxKind::OverrideList);
  parser.expect(SyntaxKind::RBrace);

  parser.expect(SyntaxKind::KwDo);
  parse_expr(parser);

  parser.close(m, SyntaxKind::WithDoExpr)
}

fn parse_import(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwImport);
  parser.expect(SyntaxKind::String);
  if parser.at(SyntaxKind::KwWith) {
    let with_m = parser.open();
    parser.advance();
    parser.expect(SyntaxKind::LBrace);
    let list_m = parser.open();
    parse_alias(parser);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RBrace) {
        break;
      }
      parse_alias(parser);
    }
    parser.close(list_m, SyntaxKind::AliasList);
    parser.expect(SyntaxKind::RBrace);
    parser.close(with_m, SyntaxKind::WithClause);
  }
  parser.close(m, SyntaxKind::Import);
}

fn parse_alias(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::Symbol);
  if parser.at(SyntaxKind::Eq) {
    parser.advance();
    parser.expect(SyntaxKind::Symbol);
  }
  parser.close(m, SyntaxKind::Alias);
}

fn parse_export(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwExport);
  parser.expect(SyntaxKind::LBrace);
  let list_m = parser.open();
  if parser.at(SyntaxKind::Symbol) {
    parser.expect(SyntaxKind::Symbol);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RBrace) {
        break;
      }
      parser.expect(SyntaxKind::Symbol);
    }
  }
  parser.close(list_m, SyntaxKind::ExportList);
  parser.expect(SyntaxKind::RBrace);
  parser.close(m, SyntaxKind::Export);
}

fn parse_type_expr(parser: &mut Parser) {
  let m = parser.open();
  match parser.peek() {
    SyntaxKind::KwRef => {
      let ref_m = parser.open();
      parser.advance();
      parse_type_app(parser);
      parser.close(ref_m, SyntaxKind::RefType);
    }
    SyntaxKind::KwFn => {
      parse_fn_type(parser);
    }
    _ => {
      parse_type_app(parser);
    }
  }
  parser.close(m, SyntaxKind::TypeExpr);
}

fn parse_fn_type(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwFn);
  parser.expect(SyntaxKind::LParen);
  if !parser.at(SyntaxKind::RParen) {
    parse_fn_type_param(parser);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RParen) {
        break;
      }
      parse_fn_type_param(parser);
    }
  }
  parser.expect(SyntaxKind::RParen);
  if parser.at(SyntaxKind::Arrow) {
    parser.advance();
    parse_type_expr(parser);
  }
  parser.close(m, SyntaxKind::FnType);
}

fn parse_fn_type_param(parser: &mut Parser) {
  let m = parser.open();
  if parser.at_any(&[SyntaxKind::KwRef, SyntaxKind::KwMut, SyntaxKind::KwOwned]) {
    parser.advance();
  }
  if parser.at(SyntaxKind::Symbol) && parser.nth(1) == SyntaxKind::Colon {
    parser.advance();
    parser.advance();
    parse_type_expr(parser);
  } else {
    parse_type_expr(parser);
  }
  parser.close(m, SyntaxKind::Param);
}

fn parse_type_app(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::Symbol);
  let args_m = parser.open();
  if parser.at(SyntaxKind::LParen) {
    parser.advance();
    if !parser.at(SyntaxKind::RParen) {
      parse_type_arg(parser);
      while parser.at(SyntaxKind::Comma) {
        parser.advance();
        if parser.at(SyntaxKind::RParen) {
          break;
        }
        parse_type_arg(parser);
      }
    }
    parser.expect(SyntaxKind::RParen);
  }
  parser.close(args_m, SyntaxKind::TypeArgList);
  parser.close(m, SyntaxKind::TypeApp);
}

fn parse_type_arg(parser: &mut Parser) {
  let m = parser.open();
  parse_type_expr(parser);
  parser.close(m, SyntaxKind::TypeArg);
}

fn parse_method_def(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwDef);
  parser.expect(SyntaxKind::Symbol);
  parser.expect(SyntaxKind::LParen);
  if parser.at(SyntaxKind::RParen) {
    parser.advance();
  } else {
    parse_param(parser);
    loop {
      match parser.peek() {
        SyntaxKind::Comma => {
          parser.advance();
          if parser.at(SyntaxKind::RParen) {
            parser.advance();
            break;
          }
          parse_param(parser);
        }
        SyntaxKind::RParen => {
          parser.advance();
          break;
        }
        _ => {
          parser.errs.push(ParseErr {
            span: parser.current_span(),
            reason: Box::new(Reason::Custom(
              "unexpected token in parameter list, expected ',' or ')'".to_string(),
            )),
            context: Vec::new(),
          });
          while !parser.at(SyntaxKind::RParen) && !parser.eof() {
            parser.advance();
          }
          if parser.at(SyntaxKind::RParen) {
            parser.advance();
          }
          break;
        }
      }
    }
  }
  if parser.at(SyntaxKind::Arrow) {
    parser.advance();
    parse_type_expr(parser);
  }
  if parser.at(SyntaxKind::KwUses) {
    let uses_m = parser.open();
    parser.advance();
    let list_m = parser.open();
    parser.expect(SyntaxKind::Symbol);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      parser.expect(SyntaxKind::Symbol);
    }
    parser.close(list_m, SyntaxKind::UsesList);
    parser.close(uses_m, SyntaxKind::UsesClause);
  }
  parser.expect(SyntaxKind::Colon);
  parse_binding_or_expr(parser);
  parser.close(m, SyntaxKind::MethodDef);
}

fn parse_type_param_list(parser: &mut Parser) {
  let m = parser.open();
  if parser.at(SyntaxKind::KwOf) {
    parser.advance();
    let p = parser.open();
    parser.expect(SyntaxKind::Symbol);
    parser.close(p, SyntaxKind::TypeParam);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      let p = parser.open();
      parser.expect(SyntaxKind::Symbol);
      parser.close(p, SyntaxKind::TypeParam);
    }
  }
  parser.close(m, SyntaxKind::TypeParamList);
}

fn parse_type_constructor(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::Symbol);
  if parser.at(SyntaxKind::LParen) {
    parser.advance();
    let params_m = parser.open();
    let p = parser.open();
    parse_type_expr(parser);
    parser.close(p, SyntaxKind::ConstructorParam);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RParen) {
        break;
      }
      let p = parser.open();
      parse_type_expr(parser);
      parser.close(p, SyntaxKind::ConstructorParam);
    }
    parser.expect(SyntaxKind::RParen);
    parser.close(params_m, SyntaxKind::ConstructorParamList);
  }
  parser.close(m, SyntaxKind::TypeConstructor);
}

fn parse_type_decl(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwType);
  parser.expect(SyntaxKind::Symbol);
  parse_type_param_list(parser);
  match parser.peek() {
    SyntaxKind::Eq => {
      parser.advance();
      parser.expect(SyntaxKind::KwStruct);
      parser.expect(SyntaxKind::LBrace);
      let fields_m = parser.open();
      if parser.at(SyntaxKind::Symbol) {
        let f = parser.open();
        parser.advance();
        parser.expect(SyntaxKind::Eq);
        parse_type_expr(parser);
        parser.close(f, SyntaxKind::StructField);
        while parser.at(SyntaxKind::Comma) {
          parser.advance();
          if parser.at(SyntaxKind::RBrace) {
            break;
          }
          let f = parser.open();
          parser.advance();
          parser.expect(SyntaxKind::Eq);
          parse_type_expr(parser);
          parser.close(f, SyntaxKind::StructField);
        }
      }
      parser.close(fields_m, SyntaxKind::StructFieldList);
      parser.expect(SyntaxKind::RBrace);
    }
    SyntaxKind::Colon => {
      parser.advance();
      let list_m = parser.open();
      let c = parser.open();
      parse_type_constructor(parser);
      parser.close(c, SyntaxKind::Constructor);
      while parser.at(SyntaxKind::KwOr) {
        parser.advance();
        let c = parser.open();
        parse_type_constructor(parser);
        parser.close(c, SyntaxKind::Constructor);
      }
      parser.close(list_m, SyntaxKind::ConstructorList);
    }
    _ => {
      parser.advance_with_error("expected '=' or ':' in type declaration");
    }
  }
  parser.close(m, SyntaxKind::TypeDecl);
}

fn parse_effect_def(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwEffect);
  parser.expect(SyntaxKind::Symbol);
  parser.expect(SyntaxKind::LBrace);
  let methods_m = parser.open();
  while parser.at(SyntaxKind::KwDef) {
    parse_method_def(parser);
  }
  parser.close(methods_m, SyntaxKind::MethodList);
  parser.expect(SyntaxKind::RBrace);
  parser.close(m, SyntaxKind::EffectDef);
}

fn parse_behavior_def(parser: &mut Parser) {
  let m = parser.open();
  parser.expect(SyntaxKind::KwConcept);
  parser.expect(SyntaxKind::Symbol);
  parser.expect(SyntaxKind::KwRequires);
  parser.expect(SyntaxKind::LBrace);
  let req_m = parser.open();
  if parser.at(SyntaxKind::Symbol) {
    let f = parser.open();
    parser.advance();
    parser.expect(SyntaxKind::Eq);
    parse_type_expr(parser);
    parser.close(f, SyntaxKind::RequirementField);
    while parser.at(SyntaxKind::Comma) {
      parser.advance();
      if parser.at(SyntaxKind::RBrace) {
        break;
      }
      let f = parser.open();
      parser.advance();
      parser.expect(SyntaxKind::Eq);
      parse_type_expr(parser);
      parser.close(f, SyntaxKind::RequirementField);
    }
  }
  parser.close(req_m, SyntaxKind::RequirementList);
  parser.expect(SyntaxKind::RBrace);
  parser.expect(SyntaxKind::KwWith);
  parser.expect(SyntaxKind::LBrace);
  let methods_m = parser.open();
  while parser.at(SyntaxKind::KwDef) {
    parse_method_def(parser);
  }
  parser.close(methods_m, SyntaxKind::MethodList);
  parser.expect(SyntaxKind::RBrace);
  parser.close(m, SyntaxKind::BehaviorDef);
}

pub fn parse(parser: &mut Parser) {
  let m = parser.open();
  while !parser.eof() {
    match parser.peek() {
      SyntaxKind::KwImport => parse_import(parser),
      SyntaxKind::KwExport => parse_export(parser),
      SyntaxKind::KwLet => parse_let(parser),
      SyntaxKind::KwMut => parse_mut(parser),
      SyntaxKind::KwRef => parse_ref(parser),
      SyntaxKind::KwType => parse_type_decl(parser),
      SyntaxKind::KwEffect => parse_effect_def(parser),
      SyntaxKind::KwConcept => parse_behavior_def(parser),
      SyntaxKind::Eof => break,
      _ => parser.advance_with_error("expected top level declaration"),
    }
  }
  parser.close(m, SyntaxKind::Root);
}
