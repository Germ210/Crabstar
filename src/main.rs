mod ast;
use chumsky::{prelude::*, text::*, pratt::*};
use ast::Ast;

fn parse_expr<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|expr| {
    let atom = choice((
      just("true").to(Ast::Bool(true)),
      just("false").to(Ast::Bool(false)),
      digits(10)
        .to_slice()
        .map(|str: &str| Ast::Num(str.parse().unwrap())),
      ident().map(|name: &str| Ast::Ident(name.to_string())),
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
  ))
    .padded()
    .recover_with(skip_then_retry_until(any().ignored(), just(")").ignored()));

    atom.pratt((
        prefix(6, just("not"), |_, rhs, _| Ast::Unary("not".into(), Box::new(rhs))),
        prefix(6, just("-"), |_, rhs, _| Ast::Unary("-".into(), Box::new(rhs))),
        infix(left(5), just("."), |lhs, _, rhs, _| Ast::Binary(".".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(4), just("*"), |lhs, _, rhs, _| Ast::Binary("*".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(4), just("/"), |lhs, _, rhs, _| Ast::Binary("/".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(4), just("%"), |lhs, _, rhs, _| Ast::Binary("%".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(3), just("+"), |lhs, _, rhs, _| Ast::Binary("+".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(3), just("-"), |lhs, _, rhs, _| Ast::Binary("-".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(2), just("<="), |lhs, _, rhs, _| Ast::Binary("<=".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(2), just(">="), |lhs, _, rhs, _| Ast::Binary(">=".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(2), just("<"), |lhs, _, rhs, _| Ast::Binary("<".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(2), just(">"), |lhs, _, rhs, _| Ast::Binary(">".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(1), just("="), |lhs, _, rhs, _| Ast::Binary("=".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(1), just("!="), |lhs, _, rhs, _| Ast::Binary("!=".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(0), just("and"), |lhs, _, rhs, _| Ast::Binary("and".into(), Box::new(lhs), Box::new(rhs))),
        infix(left(0), just("or"), |lhs, _, rhs, _| Ast::Binary("or".into(), Box::new(lhs), Box::new(rhs))),
    ))
      .recover_with(skip_then_retry_until(any().ignored(), end()))
      .or_not()
      .map(|opt| opt.unwrap_or(Ast::Dummy))
  })
  .then_ignore(end().recover_with(skip_then_retry_until(any().ignored(), end())))
}

fn main() {
  let input = "(1, 2, 3 + 2)";

  let parser = parse_expr();
  let (ast, errs) = parser.parse(input).into_output_errors();

  println!("Ast: {:#?}", ast);
  println!("Errors: {:#?}", errs);
}
