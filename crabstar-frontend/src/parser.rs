use chumsky::{prelude::*, text::{ascii::ident, *}};
use crate::ast::{Ast, AstKind};
use crate::types::Type;

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
  let decl_parser = recursive(|decl_parser| {
    let body_expr = block_expr_parser(decl_parser.clone());
    
    let param_parser = choice((
        keyword("mut").padded(),
        keyword("realloc").padded()
      ))
      .or_not()
      .then::<&str, _>(ident())
      .map_with(|(keyword, ident), e| {
        let base_type = match keyword {
          Some("mut") => Type::Mut(Box::new(Type::Unknown)),
          Some("realloc") => Type::Realloc(Box::new(Type::Unknown)),
          _ => Type::Unknown
        };
        make_typed_ast(e.span(), AstKind::Ident(ident.to_string()), base_type)
      })
      .recover_with(via_parser(
        none_of(",)(:")
          .repeated()
          .at_least(1)
          .ignored()
          .map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
      ))
      .labelled("function parameter")
      .as_context();

    let type_parser = ident()
      .map_with(|type_name: &str, e| make_ast(e.span(), AstKind::Ident(type_name.to_string())));

    // Let parser
    let let_decl = just("let")
      .ignore_then(keyword("mut").padded().or_not())
      .then(keyword("rec").padded().or_not())
      .then(ident().padded())
      .then(choice((
        just("::")
          .ignore_then(
            param_parser.clone()
              .separated_by(just(",").padded())
              .allow_trailing()
              .collect::<Vec<Ast>>()
              .delimited_by(just("(").padded(), just(")").padded())
          )
          .then(just("=>").padded().ignore_then(type_parser.clone()).or_not())
          .then(body_expr.clone())
          .map(|((args, ret_type), value)| (Some(args), ret_type, value)),
        just("=>")
          .padded()
          .ignore_then(expr_parser(decl_parser.clone()))
          .map(|expr| (Some(vec![]), None, expr)),
        body_expr.clone().map(|expr| (None, None, expr))
      )))
      .map_with(|(((mutable_kw, rec_kw), name), (args, ret_type, value)), e| {
        make_ast(e.span(), AstKind::Let {
          name: name.to_string(),
          mutable: mutable_kw.is_some(),
          recursive: rec_kw.is_some(),
          args,
          value: Box::new(value),
          ret_type: ret_type.map(Box::new),
          next: None,
          constraints: vec![]
        })
      });

    // Const parser
    let const_decl = just("const")
      .ignore_then(keyword("rec").padded().or_not())
      .then(ident().padded())
      .then(choice((
        just("::")
          .ignore_then(
            param_parser
              .separated_by(just(",").padded())
              .allow_trailing()
              .collect::<Vec<Ast>>()
              .delimited_by(just("(").padded(), just(")").padded())
          )
          .then(just("=>").padded().ignore_then(type_parser).or_not())
          .then(body_expr.clone())
          .map(|((args, ret_type), value)| (Some(args), ret_type, value)),
        just("=>")
          .padded()
          .ignore_then(expr_parser(decl_parser.clone()))
          .map(|expr| (Some(vec![]), None, expr)),
        body_expr.clone().map(|expr| (None, None, expr))
      )))
      .map_with(|((rec_kw, name), (args, ret_type, value)), e| {
        make_ast(e.span(), AstKind::Const {
          name: name.to_string(),
          recursive: rec_kw.is_some(),
          args,
          value: Box::new(value),
          ret_type: ret_type.map(Box::new),
          constraints: vec![]
        })
      });

    choice((let_decl, const_decl)).padded()
  });

  decl_parser
    .repeated()
    .collect()
    .then_ignore(end())
}

fn block_expr_parser<'src>(
  decl_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  choice((
    just(":")
      .ignore_then(expr_parser(decl_parser.clone())),
    expr_parser(decl_parser)
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

fn string_literal_parser<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  just('"')
    .ignore_then(none_of('"').repeated().to_slice())
    .then_ignore(just('"'))
    .map_with(|s: &str, e| make_typed_ast(e.span(), AstKind::String(s.to_string()), Type::Unknown))
    .labelled("string literal")
    .as_context()
}

fn identifier_parser<'src>() -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  ident()
    .map_with(|name: &str, e| make_ast(e.span(), AstKind::Ident(name.to_string())))
    .labelled("identifier")
    .as_context()
}

fn array_literal_parser<'src>(
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  expr
    .separated_by(just(",").padded())
    .allow_trailing()
    .collect::<Vec<_>>()
    .delimited_by(just("[").padded(), just("]").padded())
    .map_with(|exprs, e| make_typed_ast(e.span(), AstKind::Array(exprs), Type::Array(Box::new(Type::Unknown))))
    .recover_with(via_parser(
      nested_delimiters('[', ']', [('(', ')')], |span| make_ast(span, AstKind::Dummy))
    ))
    .labelled("array literal")
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
      make_typed_ast(e.span(), AstKind::HeapAlloc {
        class: class.to_string(),
        expr: Box::new(expr)
      },
      Type::Heap(Box::new(Type::Unknown)))
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
    .then(
      keyword("else")
      .padded()
      .ignore_then(block_expr.map_with(|expr, e| (e.span(), Box::new(expr))))
      .or_not()
    )
    .map_with(|((cond, then_expr), else_expr), e| {
      make_ast(
        e.span(),
        AstKind::If {
          cond: Box::new(cond),
          then_expr: Box::new(then_expr),
          else_expr: else_expr.map(|(_, else_box)| else_box),
        },
      )
    })
}

fn match_expr_parser<'src>(
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  keyword("match")
    .padded()
    .ignore_then(expr.clone())
    .then(
      match_branch(expr.clone())
        .separated_by(just(",").padded())
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just("(").padded(), just(")").padded())
        .recover_with(via_parser(
          nested_delimiters('(', ')', [('[', ']')], |span| vec![make_ast(span, AstKind::Dummy)])
        ))
    )
    .then(
      keyword("else")
        .padded()
        .ignore_then(choice((
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
        )))
        .or_not()
    )
    .map_with(|((scrutinee, branches), else_expr), e| {
      make_ast(e.span(), AstKind::Match {
        scrutinee: Box::new(scrutinee),
        branches,
        else_expr: else_expr.map(Box::new),
      })
    })
    .labelled("match expression")
    .as_context()
}

fn match_branch<'src>(
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  keyword("of")
    .padded()
    .ignore_then(expr.clone())
    .then(
      choice((
        expr.clone()
          .separated_by(just(",").padded())
          .allow_trailing()
          .collect()
          .delimited_by(just("(").padded(), just(")").padded())
          .map_with(|exprs, e| make_ast(e.span(), AstKind::Block(exprs)))
          .recover_with(via_parser(
            nested_delimiters('(', ')', [('[', ']')], |span| make_ast(span, AstKind::Dummy))
          )),
        just(":")
          .padded()
          .ignore_then(expr.clone())
          .recover_with(via_parser(
            none_of(",)")
              .repeated()
              .ignored()
              .map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
          ))
      ))
    )
    .map_with(|(pattern, body), e| {
      make_ast(e.span(), AstKind::MatchBranch {
        match_guard: None,
        expr: Box::new(pattern),
        body: Box::new(body),
      })
    })
    .labelled("match branch")
    .as_context()
}

fn atom_parser<'src>(
  decl_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src,
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  choice((
    if_expr_parser(expr.clone()),
    match_expr_parser(expr.clone()),
    array_literal_parser(expr.clone()),
    heap_alloc_parser(expr.clone()),
    decl_parser,
    bool_literal_parser(),
    number_literal_parser(),
    string_literal_parser(),
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
        make_ast(e.span(), AstKind::Call { 
          callee: Box::new(callee), 
          args
        })
      }
    )
  ))
}

fn index_access_parser<'src>(
  field_access: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src,
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  field_access.foldl_with(
    expr.delimited_by(just("["), just("]")).repeated(),
    |array, index, e| {
      make_ast(e.span(), AstKind::Index {
        array: Box::new(array),
        index: Box::new(index)
      })
    }
  )
}

fn prefix_parser<'src>(
  index_access: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
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
      index_access,
    ))
  })
}

fn assign_parser<'src>(
  or: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  or.clone()
    .separated_by(just(":").padded())
    .collect::<Vec<_>>()
    .map_with(|mut exprs, e| {
      if exprs.len() == 1 {
        exprs.into_iter().next().unwrap()
      } else {
        let last = exprs.pop().unwrap();
        exprs.into_iter().rev().fold(last, |acc, target| {
          make_ast(e.span(), AstKind::Assign {
            target: Box::new(target),
            value: Box::new(acc)
          })
        })
      }
    })
}

fn method_call_parser<'src>(
  base: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src,
  expr: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  base.foldl_with(
    just("|>")
      .padded()
      .ignore_then(ident())
      .then(
        expr
          .separated_by(just(","))
          .collect()
          .delimited_by(just("("), just(")"))
          .or(empty().to(vec![]))
      )
      .repeated(),
    |object, (method_name, args), e| {
      make_ast(e.span(), AstKind::MethodCall {
        object: Box::new(object),
        method: method_name.to_string(),
        args
      })
    }
  )
}

pub fn expr_parser<'src>(
  decl_parser: impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone + 'src
) -> impl Parser<'src, &'src str, Ast, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|expr| {
    let atom = atom_parser(decl_parser, expr.clone());
    let call = call_parser(atom, expr.clone());
    let index_access = index_access_parser(call, expr.clone());
    let prefix = prefix_parser(index_access);

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

    let method_call = method_call_parser(or, expr.clone());

    assign_parser(method_call)
  })
  .recover_with(via_parser(
    any().repeated().ignored().map_with(|_, e| make_ast(e.span(), AstKind::Dummy))
  ))
}
