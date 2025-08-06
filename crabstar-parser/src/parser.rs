use chumsky::{prelude::*, text::{ascii::ident, *}};
use crate::ast::Ast;

pub fn parser<'src>() -> impl Parser<'src, &'src str, Vec<Ast>, extra::Err<Rich<'src, char>>> {
  let let_parser = recursive(|let_parser| {
    let body_expr = choice((
      just(":")
        .ignore_then(expr_parser(let_parser.clone())),
      expr_parser(let_parser.clone())
        .separated_by(just(","))
        .allow_trailing()
        .collect()
        .delimited_by(just("("), just(")"))
        .map(Ast::Block)
    ));

    let param_parser = ident()
      .map(|ident: &str| Ast::Ident(ident.to_string()))
      .recover_with(via_parser(
        none_of(",)(:")
          .repeated()
          .at_least(1)
          .ignored()
          .map(|_| Ast::Dummy)
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
                  .map(|_| vec![Ast::Ident("error_params".to_string())])
              ))
          )
          .then(body_expr.clone().recover_with(via_parser(
            any()
              .repeated()
              .ignored()
              .map(|_| Ast::Dummy)
          )))
          .map(|(args, body_expr)| (Some(args), body_expr)),
        just("=>")
          .padded()
          .ignore_then(expr_parser(let_parser).recover_with(via_parser(
            any()
              .repeated()
              .ignored()
              .map(|_| Ast::Dummy)
          )))
          .map(|expr| (Some(vec![]), expr)),
        body_expr.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        )).map(|expr| (None, expr))
      )))
      .map(|(name, (args, value))| Ast::Let {
        name: name.to_string(),
        args,
        value: Box::new(value),
        next: None
      })
      .padded()
  });

  let_parser
    .repeated()
    .collect()
    .recover_with(skip_then_retry_until(any().ignored(), end()))
    .padded()
}

fn binary<'a, P, Q>(
  lhs: P,
  op_rhs: Q,
) -> impl Parser<'a, &'a str, Ast, extra::Err<Rich<'a, char>>> + Clone
where
  P: Parser<'a, &'a str, Ast, extra::Err<Rich<'a, char>>> + Clone,
  Q: Parser<'a, &'a str, (&'a str, Ast), extra::Err<Rich<'a, char>>> + Clone,
{
  lhs.clone().foldl(op_rhs.repeated(), |lhs, (op, rhs)| {
    Ast::Binary(op.to_string(), Box::new(lhs), Box::new(rhs))
  })
}

pub fn expr_parser<'src>(
  let_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|expr| {
    let bool_lit = choice((
      just("true").to(Ast::Bool(true)),
      just("false").to(Ast::Bool(false)),
    ))
      .labelled("true/false")
      .as_context();

    let float_lit = digits(10)
      .then(just("."))
      .then(digits(10))
      .to_slice()
      .map(|s: &str| Ast::Float(s.parse().unwrap_or(0.0)));

    let int_lit = digits(10)
      .to_slice()
      .map(|s: &str| Ast::Int(s.parse::<u64>().unwrap_or(0)));

    let number_lit = choice((float_lit, int_lit))
      .labelled("number")
      .as_context();

    let ident_expr = ident()
      .map(|name: &str| Ast::Ident(name.to_string()))
      .labelled("identifier")
      .as_context();

    let grouped_expr = expr
      .clone()
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
      .recover_with(via_parser(
        nested_delimiters('(', ')', [('[', ']')], |_| Ast::Dummy)
      ))
      .labelled("parenthesized expression")
      .as_context();

    let atom = choice((
      let_parser,
      bool_lit,
      number_lit,
      ident_expr,
      grouped_expr,
    ))
      .padded()
      .recover_with(via_parser(
        none_of("),")
          .repeated()
          .ignored()
          .map(|_| Ast::Dummy)
      ));

    let call = atom
      .clone()
      .then(just("(").padded().then(just(")").padded()).ignored())
      .map(|(callee, _)| Ast::Call {
        callee: Box::new(callee), args: vec![]
      })
      .or(atom.foldl(
          expr
          .clone()
          .separated_by(just(","))
          .collect()
          .delimited_by(just("("), just(")"))
          .repeated(),
          |callee, args| Ast::Call { 
            callee: Box::new(callee), 
            args
          }
      ))
      ;

    let prefix = recursive(|prefix| {
      choice((
        keyword("not")
          .padded()
          .then(prefix.clone())
          .map(|(_, rhs)| Ast::Unary("not".into(), Box::new(rhs)))
          .labelled("not")
          .as_context(),
        just("-")
          .padded()
          .then(prefix.clone())
          .map(|(_, rhs)| Ast::Unary("-".into(), Box::new(rhs)))
          .labelled("-"),
        call.clone(),
      ))
    });

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
    any().repeated().ignored().map(|_| Ast::Dummy)
  ))
}

