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
    .separated_by(just(",").padded())
    .collect()
    .recover_with(skip_then_retry_until(any().ignored(), end()))
    .padded()
}

pub fn expr_parser<'src>(
  let_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|expr| {
    let atom = choice((
      just("true").to(Ast::Bool(true)).labelled("true").as_context(),
      just("false").to(Ast::Bool(false)).labelled("false").as_context(),
      choice((
        digits(10).then(just(".")).then(digits(10)).to_slice().map(|str: &str|
          Ast::Float(str.parse().unwrap_or(0.0))
        ),
        digits(10).to_slice().map(|str: &str|
          Ast::Int(str.parse::<u64>().unwrap_or(0))
        ),
        let_parser
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
        .as_context()
    ))
      .padded()
      .recover_with(via_parser(
        none_of("),")
          .repeated()
          .ignored()
          .map(|_| Ast::Dummy)
      ));

    let prefix = recursive(|prefix| {
      choice((
        just("not").padded().then(prefix.clone()).map(|(_, rhs)| {
          Ast::Unary("not".into(), Box::new(rhs))
        }).labelled("not").as_context(),
        just("-").padded().then(prefix.clone()).map(|(_, rhs)| {
          Ast::Unary("-".into(), Box::new(rhs))
        }).labelled("-"),
        atom.clone()
      ))
        .recover_with(via_parser(
          none_of("*/%+-<>=!(),")
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        ))
    });

    let multiplicative = prefix.clone().foldl(
      choice((just("*"), just("/"), just("%")))
        .recover_with(via_parser(
          none_of(" \t\n+-<>=!(),")
            .repeated()
            .at_least(1)
            .ignored()
            .map(|_| "*")
        ))
        .padded()
        .then(prefix.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        )))
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs))
    );

    let additive = multiplicative.clone().foldl(
      choice((just("+"), just("-")))
        .recover_with(via_parser(
          none_of(" \t\n*/%<>=!(),")
            .repeated()
            .at_least(1)
            .ignored()
            .map(|_| "+")
        ))
        .padded()
        .then(multiplicative.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        )))
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs))
    );

    let comparison = additive.clone().foldl(
      choice((just("<="), just(">="), just("<"), just(">")))
        .recover_with(via_parser(
          none_of(" \t\n=!(),")
            .repeated()
            .at_least(1)
            .ignored()
            .map(|_| "<")
        ))
        .padded()
        .then(additive.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        )))
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs))
    );

    let equality = comparison.clone().foldl(
      choice((just("="), just("!=")))
        .recover_with(via_parser(
          none_of(" \t\n(),")
            .repeated()
            .at_least(1)
            .ignored()
            .map(|_| "=")
        ))
        .padded()
        .then(comparison.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        )))
        .repeated(),
      |lhs, (op, rhs)| Ast::Binary(op.into(), Box::new(lhs), Box::new(rhs))
    );

    let logical_and = equality.clone().foldl(
      just("and")
        .recover_with(via_parser(
          none_of(" \t\n(),")
            .repeated()
            .at_least(1)
            .ignored()
            .map(|_| "and")
        ))
        .padded()
        .then(equality.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        )))
        .repeated(),
      |lhs, (_, rhs)| Ast::Binary("and".into(), Box::new(lhs), Box::new(rhs))
    );

    let logical_or = logical_and.clone().foldl(
      just("or")
        .recover_with(via_parser(
          none_of(" \t\n(),")
            .repeated()
            .at_least(1)
            .ignored()
            .map(|_| "or")
        ))
        .padded()
        .then(logical_and.clone().recover_with(via_parser(
          any()
            .repeated()
            .ignored()
            .map(|_| Ast::Dummy)
        )))
        .repeated(),
      |lhs, (_, rhs)| Ast::Binary("or".into(), Box::new(lhs), Box::new(rhs))
    );

    logical_or.recover_with(skip_then_retry_until(any().ignored(), end()))
  })
}

