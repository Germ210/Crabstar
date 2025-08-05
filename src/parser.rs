use chumsky::{prelude::*, text::{ascii::ident, *}};
use crate::ast::Ast;

pub fn parser<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> {
  let body_expr = choice((
    just(":")
      .ignore_then(expr_parser()),
    expr_parser()
      .separated_by(just(","))
      .allow_trailing()  
      .collect()
      .map(|mut nodes: Vec<Ast>| if nodes.len() == 1 { std::mem::take(&mut nodes[0]) } else { unreachable!() })
  ));
  
  just("let")
    .ignore_then(ident().padded())
    .then(choice((
      just("::")
        .ignore_then(
          ident().map(|ident: &str| Ast::Ident(ident.to_string()))
          .separated_by(just(",").padded())
          .allow_trailing()
          .collect::<Vec<Ast>>()
          .delimited_by(just("(").padded(), just(")").padded())
        )
        .then(body_expr.clone())
        .map(|(args, body_expr)| (Some(args), body_expr)),
      body_expr.map(|expr| (None, expr))
    )))
    .map(|(name, (args, value))| Ast::Let {
      name: name.to_string(),
      args,
      value: Box::new(value),
      next: None
    })
    .then_ignore(end())
    .recover_with(skip_then_retry_until(any().ignored(), end()))
}

pub fn expr_parser<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|expr| {
    let atom = choice((
      just("true").to(Ast::Bool(true)).labelled("true").as_context(),
      just("false").to(Ast::Bool(false)).labelled("false").as_context(),
      choice((
        digits(10).then(just(".")).then(digits(10)).to_slice().map(|str: &str| Ast::Float(str.parse().unwrap())),
        digits(10).to_slice().map(|str: &str| Ast::Int(str.parse::<u64>().unwrap()))
      ))
      .labelled("number")
      .as_context(),
      ident()
        .map(|name: &str| Ast::Ident(name.to_string()))
        .labelled("identifier")
        .as_context(),
      expr.clone()
        .separated_by(just(",").padded())
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just("(").padded(), just(")").padded())
        .map(|exprs| {
          if exprs.len() == 1 {
            exprs.into_iter().next().unwrap()
          } else {
            Ast::Block(exprs)
          }
        })
        .labelled("parenthesized expression")
        .as_context(),
    ))
    .padded()
    .recover_with(via_parser(
      none_of("),")
        .repeated()
        .ignored()
        .map(|_| Ast::Dummy),
    ));
    
    let prefix = recursive(|prefix| {
      choice((
        just("not").padded().then(prefix.clone()).map(|(_, rhs)| {
          Ast::Unary("not".into(), Box::new(rhs))
        }),
        just("-").padded().then(prefix.clone()).map(|(_, rhs)| {
          Ast::Unary("-".into(), Box::new(rhs))
        }),
        atom.clone(),
      ))
    });
    
    let multiplicative = prefix.clone().foldl(
      choice((just("*"), just("/"), just("%")))
        .padded()
        .then(prefix.clone())
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs)),
    );
    
    let additive = multiplicative.clone().foldl(
      choice((just("+"), just("-")))
        .padded()
        .then(multiplicative.clone())
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs)),
    );
    
    let comparison = additive.clone().foldl(
      choice((just("<="), just(">="), just("<"), just(">")))
        .padded()
        .then(additive.clone())
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs)),
    );
    
    let equality = comparison.clone().foldl(
      choice((just("="), just("!=")))
        .padded()
        .then(comparison.clone())
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs)),
    );
    
    let logical_and = equality.clone().foldl(
      just("and")
        .padded()
        .then(equality.clone())
        .repeated(),
      |lhs, (_, rhs)| Ast::Binary("and".into(), Box::new(lhs), Box::new(rhs)),
    );
    
    let logical_or = logical_and.clone().foldl(
      just("or")
        .padded()
        .then(logical_and.clone())
        .repeated(),
      |lhs, (_, rhs)| Ast::Binary("or".into(), Box::new(lhs), Box::new(rhs)),
    );
    
    logical_or.recover_with(skip_then_retry_until(any().ignored(), end()))
  })
}
