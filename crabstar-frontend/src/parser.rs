use chumsky::{prelude::*, text::{ascii::ident, *}};
use crate::ast::{Ast, AstKind};
use crate::typechecker::Type;

fn make_ast(span: SimpleSpan, kind: AstKind) -> Ast {
  (span, crate::ast::TypedAst {
    ty: Type::Unknown,
    node: kind,
  })
}

fn make_typed_ast(span: SimpleSpan, kind: AstKind, ty: Type) -> Ast {
  (span, crate::ast::TypedAst {
    ty,
    node: kind,
  })
}

pub fn parser<'src>() -> impl Parser<'src, &'src str, Vec<Ast>, extra::Err<Rich<'src, char>>> {
  let let_parser = recursive(|let_parser| {
    let body_expr = block_expr_parser(let_parser.clone());

    let param_parser = ident()
      .map_with(|ident: &str, e| make_ast(e.span(), AstKind::Ident(ident.to_string())))
      .recover_with(via_parser(
        none_of(",)(:")
          .repeated()
          .at_least(1)
          .ignored()
          .map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
      ))
      .labelled("function parameter")
      .as_context();

    just("let")
      .ignore_then(ident().padded().recover_with(via_parser(
        none_of(" \t\n:=()")
          .repeated()
          .at_least(1)
          .to_slice()
          .map(|s: &str| if s.is_empty() { "error_name" } else { s })
      )))
      .then(choice((
        just("::")
          .ignore_then(
            param_parser
              .separated_by(just(",").padded())
              .allow_trailing()
              .collect::<Vec<Ast>>()
              .delimited_by(just("(").padded(), just(")").padded())
              .labelled("function parameters")
              .recover_with(via_parser(
                none_of(")")
                  .repeated()
                  .ignored()
                  .map_with(|_, e| vec![make_ast(e.span(), AstKind::Ident("error_params".to_string()))])
              ))
          )
          .then(body_expr.clone().recover_with(via_parser(
            any()
              .repeated()
              .ignored()
              .map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
          )))
          .map(|(args, value)| (Some(args), value)),
        just("=>")
          .padded()
          .ignore_then(expr_parser(let_parser.clone()).recover_with(via_parser(
            any()
              .repeated()
              .ignored()
              .map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
          )))
          .map(|expr| (Some(vec![]), expr)),
        body_expr.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
        )).map(|expr| (None, expr))
      )))
      .map_with(|(name, (args, value)), e| make_ast(e.span(), AstKind::Let {
        name: name.to_string(),
        args,
        value: Box::new(value),
        next: None
      }))
      .padded()
  });

  let_parser
    .repeated()
    .collect()
    .recover_with(skip_then_retry_until(any().ignored(), end()))
    .padded()
}

fn block_expr_parser<'src>(
  let_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  choice((
    just(":")
      .ignore_then(expr_parser(let_parser.clone())),
    expr_parser(let_parser)
      .separated_by(just(","))
      .allow_trailing()
      .collect()
      .delimited_by(just("("), just(")"))
      .map_with(|exprs, e| make_ast(e.span(), AstKind::Block(exprs)))
  ))
}

fn binary<'a, P, Q>(
  lhs: P,
  op_rhs: Q,
) -> impl Parser<'a, &'a str, Ast, extra::Err<Rich<'a, char>>> + Clone
where
  P: Parser<'a, &'a str, Ast, extra::Err<Rich<'a, char>>> + Clone,
  Q: Parser<'a, &'a str, (&'a str, Ast), extra::Err<Rich<'a, char>>> + Clone,
{
  lhs.clone().foldl_with(op_rhs.repeated(), |lhs, (op, rhs), e| {
    make_ast(e.span(), AstKind::Binary(op.to_string(), Box::new(lhs), Box::new(rhs)))
  })
}

fn bool_literal_parser<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  choice((
    just("true").to(true),
    just("false").to(false),
  ))
    .map_with(|b, e| make_typed_ast(e.span(), AstKind::Bool(b), Type::Bool))
    .labelled("true/false")
    .as_context()
}

fn number_literal_parser<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  let float_lit = digits(10)
    .then(just("."))
    .then(digits(10))
    .to_slice()
    .map_with(|s: &str, e| make_typed_ast(e.span(), AstKind::Float(s.parse().unwrap_or(0.0)), Type::Float));

  let int_lit = digits(10)
    .to_slice()
    .map_with(|s: &str, e| make_typed_ast(e.span(), AstKind::Int(s.parse::<u64>().unwrap_or(0)), Type::Int));

  choice((float_lit, int_lit))
    .labelled("number")
    .as_context()
}

fn identifier_parser<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  ident()
    .map_with(|name: &str, e| make_ast(e.span(), AstKind::Ident(name.to_string())))
    .labelled("identifier")
    .as_context()
}

fn heap_alloc_parser<'src>(
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  keyword("new")
    .padded()
    .ignore_then(ident())
    .padded()
    .then(expr.delimited_by(just("("), just(")")))
    .map_with(|(class, expr), e| {
      make_ast(e.span(), AstKind::HeapAlloc {
        class: class.to_string(),
        expr: Box::new(expr)
      })
    })
}

fn grouped_expr_parser<'src>(
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  expr
    .separated_by(just(",").padded())
    .allow_trailing()
    .collect::<Vec<_>>()
    .delimited_by(just("(").padded(), just(")").padded())
    .map_with(|exprs, e| {
      if exprs.len() == 1 {
        exprs.into_iter().next().unwrap()
      } else {
        make_ast(e.span(), AstKind::Block(exprs))
      }
    })
    .recover_with(via_parser(
      nested_delimiters('(', ')', [('[', ']')], |span| make_ast(span, AstKind::Dummy))
    ))
    .labelled("parenthesized expression")
    .as_context()
}

fn if_expr_parser<'src>(
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  let block_expr = choice((
    just(":")
      .padded()
      .ignore_then(expr.clone()),
    expr.clone()
      .separated_by(just(","))
      .allow_trailing()
      .collect()
      .padded()
      .delimited_by(just("("), just(")"))
      .map_with(|exprs, e| make_ast(e.span(), AstKind::Block(exprs)))
      .padded()
  ));
  
  keyword("if")
    .padded()
    .ignore_then(expr.clone())
    .then(block_expr.clone()) 
    .padded()
    .map_with(|(cond, then_expr), e| make_ast(e.span(), AstKind::If {
      cond: Box::new(cond),
      then_expr: Box::new(then_expr),
      else_expr: None
    }))
    .foldl(
      keyword("elif")
      .padded()
      .ignore_then(expr.clone())
      .then(block_expr.clone())
      .repeated(), 
      |if_expr, (elif_cond, elif_expr)| {
        let (span, typed_ast) = if_expr;
        if let AstKind::If { cond, then_expr, .. } = typed_ast.node { 
          make_ast(span, AstKind::If {
            cond, 
            then_expr, 
            else_expr: Some(Box::new(make_ast(span, AstKind::If { 
              cond: Box::new(elif_cond), 
              then_expr: Box::new(elif_expr), 
              else_expr: None 
            })))
          })
        } else { 
          unreachable!() 
        }
      }
    )
    .then(
      keyword("else")
        .padded()
        .ignore_then(block_expr.map_with(|expr, e| (e.span(), Box::new(expr))))
        .or_not()
    )
    .map(|(if_expr, else_expr)| {
      let (span, typed_ast) = if_expr;
      match (typed_ast.node, else_expr) {
        (AstKind::If { cond, then_expr, else_expr: None }, Some((_, new_else))) => 
          make_ast(span, AstKind::If { cond, then_expr, else_expr: Some(new_else) }),
        (AstKind::If { cond, then_expr, else_expr: Some(nested) }, Some((_, new_else))) => 
          make_ast(span, AstKind::If { cond, then_expr, else_expr: Some(Box::new(set_innermost_else(*nested, new_else))) }),
        (if_node, None) => make_ast(span, if_node),
        _ => unreachable!()
      }
    })
}

#[inline(always)]
fn set_innermost_else(ast: Ast, new_else: Box<Ast>) -> Ast {
  let (span, typed_ast) = ast;
  match typed_ast.node {
    AstKind::If { cond, then_expr, else_expr: None } => 
      make_ast(span, AstKind::If { cond, then_expr, else_expr: Some(new_else) }),
    AstKind::If { cond, then_expr, else_expr: Some(nested) } => 
      make_ast(span, AstKind::If { cond, then_expr, else_expr: Some(Box::new(set_innermost_else(*nested, new_else))) }),
    _ => unreachable!()
  }
}

fn atom_parser<'src>(
  let_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src,
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  choice((
    if_expr_parser(expr.clone()),
    heap_alloc_parser(expr.clone()),
    let_parser,
    bool_literal_parser(),
    number_literal_parser(),
    identifier_parser(),
    grouped_expr_parser(expr),
  ))
    .padded()
    .recover_with(via_parser(
      none_of("),")
        .repeated()
        .ignored()
        .map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
    ))
}

fn call_parser<'src>(
  atom: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src,
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  choice((
    atom
      .clone()
      .then(just("(").padded().then(just(")").padded()).ignored())
      .map_with(|(callee, _), e| {
        // Function calls have unknown types until type checking  
        make_ast(e.span(), AstKind::Call {
          callee: Box::new(callee), args: vec![]
        })
      }),
    atom.foldl_with(
      expr
        .separated_by(just(","))
        .collect()
        .delimited_by(just("("), just(")"))
        .repeated(),
      |callee, args, e| {
        // Function calls have unknown types until type checking
        make_ast(e.span(), AstKind::Call { 
          callee: Box::new(callee), 
          args
        })
      }
    )
  ))
}

fn prefix_parser<'src>(
  call: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|prefix| {
    choice((
      keyword("not")
        .padded()
        .then(prefix.clone())
        .map_with(|(_, rhs), e| make_ast(e.span(), AstKind::Unary("not".into(), Box::new(rhs))))
        .labelled("not")
        .as_context(),
      just("-")
        .padded()
        .then(prefix.clone())
        .map_with(|(_, rhs), e| make_ast(e.span(), AstKind::Unary("-".into(), Box::new(rhs))))
        .labelled("-"),
      call,
    ))
  })
}

pub fn expr_parser<'src>(
  let_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|expr| {
    let atom = atom_parser(let_parser, expr.clone());
    let call = call_parser(atom, expr.clone());
    let prefix = prefix_parser(call);

    let product = binary(
      prefix.clone(),
      choice((just("*"), just("/"), just("%"))).then(prefix.clone()),
    );

    let sum = binary(
      product.clone(),
      choice((just("+"), just("-"))).then(product.clone()),
    );

    let comparison = binary(
      sum.clone(),
      choice((just("<="), just(">="), just("<"), just(">"))).then(sum.clone()),
    );

    let equality = binary(
      comparison.clone(),
      choice((just("="), just("!="))).then(comparison.clone()),
    );

    let and = binary(
      equality.clone(),
      keyword("and").then(equality.clone()),
    );

    let or = binary(
      and.clone(),
      keyword("or").then(and.clone()),
    );

    or
  })
  .recover_with(via_parser(
    any().repeated().ignored().map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
  ))
}
