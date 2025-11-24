use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
pub use chumsky::Parser;
use chumsky::pratt::*;
use chumsky::prelude::*;
use chumsky::text::keyword;
use rowan::{GreenNode, GreenToken, NodeOrToken};

type Cst = GreenNode;

fn whitespace<'src>() -> impl Parser<'src, &'src str, String, extra::Err<Rich<'src, char>>> + Clone
{
  any::<&'src str, extra::Err<Rich<'src, char>>>()
    .filter(|c: &char| c.is_whitespace())
    .repeated()
    .collect::<String>()
}

fn int<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(text::int::<&'src str, extra::Err<Rich<'src, char>>>(10))
    .map(|(ws, s)| {
      GreenNode::new(
        SyntaxKind::Literal.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), &ws)),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Int.into(), &s)),
        ],
      )
    })
}

fn float<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(text::int::<&'src str, extra::Err<Rich<'src, char>>>(10))
    .then_ignore(just('.'))
    .then(
      any::<&'src str, extra::Err<Rich<'src, char>>>()
        .filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect::<String>(),
    )
    .map(|((ws, a), b)| {
      let s = format!("{}.{}", a, b);
      GreenNode::new(
        SyntaxKind::Literal.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), &ws)),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Float.into(), &s)),
        ],
      )
    })
}

fn let_in<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|let_in_expr| {
    whitespace()
      .then(keyword("let"))
      .map(|(ws, _)| {
        GreenNode::new(
          SyntaxKind::Punctuation.into(),
          vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::KwLet.into(), "let")),
          ],
        )
      })
      .then(ident())
      .then(whitespace().then(just(':')).map(|(ws, _)| {
        GreenNode::new(
          SyntaxKind::Punctuation.into(),
          vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
          ],
        )
      }))
      .then(expr())
      .then(
        whitespace()
          .then(keyword("in"))
          .map(|(ws, _)| {
            GreenNode::new(
              SyntaxKind::Punctuation.into(),
              vec![
                NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                NodeOrToken::Token(GreenToken::new(SyntaxKind::KwIn.into(), "in")),
              ],
            )
          })
          .then(choice((let_in_expr.clone(), expr())))
          .map(|(in_kw, in_expr)| {
            GreenNode::new(
              SyntaxKind::InExpr.into(),
              vec![NodeOrToken::Node(in_kw), NodeOrToken::Node(in_expr)],
            )
          })
          .or_not(),
      )
      .map(|((((let_kw, id), colon), value), maybe_in)| {
        let mut children = vec![
          NodeOrToken::Node(let_kw),
          NodeOrToken::Node(id),
          NodeOrToken::Node(colon),
          NodeOrToken::Node(value),
        ];

        if let Some(in_expr) = maybe_in {
          children.push(NodeOrToken::Node(in_expr));
        }

        GreenNode::new(SyntaxKind::LetExpr.into(), children)
      })
  })
}

fn string<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(
      just('"')
        .ignore_then(none_of('"').repeated().collect::<String>())
        .then_ignore(just('"')),
    )
    .map(|(ws, s)| {
      let full = format!("\"{}\"", s);
      GreenNode::new(
        SyntaxKind::Literal.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), &ws)),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::String.into(), &full)),
        ],
      )
    })
}

fn bool<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(
      just("true")
        .to(SyntaxKind::KwTrue)
        .or(just("false").to(SyntaxKind::KwFalse)),
    )
    .map(|(ws, kind)| {
      let txt = if kind == SyntaxKind::KwTrue {
        "true"
      } else {
        "false"
      };
      GreenNode::new(
        SyntaxKind::Literal.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), &ws)),
          NodeOrToken::Token(GreenToken::new(kind.into(), txt)),
        ],
      )
    })
}

fn ident<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(text::ident::<&'src str, extra::Err<Rich<'src, char>>>())
    .map(|(ws, s)| {
      GreenNode::new(
        SyntaxKind::Ident.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), &ws)),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Ident.into(), &s)),
        ],
      )
    })
}

fn func<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(keyword("fn"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwFn.into(), "fn")),
        ],
      )
    })
    .then(whitespace().then(just('(')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::LParen.into(), "(")),
        ],
      )
    }))
    .then(
      ident()
        .then(
          whitespace()
            .then(just(','))
            .map(|(ws, _)| {
              GreenNode::new(
                SyntaxKind::Punctuation.into(),
                vec![
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Comma.into(), ",")),
                ],
              )
            })
            .then(ident())
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(first_param, rest)| {
          let mut children = vec![NodeOrToken::Node(first_param)];
          for (comma, param) in rest {
            children.push(NodeOrToken::Node(comma));
            children.push(NodeOrToken::Node(param));
          }
          GreenNode::new(SyntaxKind::ParamList.into(), children)
        })
        .or_not(),
    )
    .then(whitespace().then(just(')')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::RParen.into(), ")")),
        ],
      )
    }))
    .then(whitespace().then(just(':')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
        ],
      )
    }))
    .then(expr)
    .map(|(((((fn_kw, lparen), params), rparen), colon), body)| {
      let mut children = vec![NodeOrToken::Node(fn_kw), NodeOrToken::Node(lparen)];

      if let Some(param_list) = params {
        children.push(NodeOrToken::Node(param_list));
      }

      children.push(NodeOrToken::Node(rparen));
      children.push(NodeOrToken::Node(colon));
      children.push(NodeOrToken::Node(body));

      GreenNode::new(SyntaxKind::FnExpr.into(), children)
    })
}

fn call_args<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(just('('))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::LParen.into(), "(")),
        ],
      )
    })
    .then(
      expr
        .clone()
        .then(
          whitespace()
            .then(just(','))
            .map(|(ws, _)| {
              GreenNode::new(
                SyntaxKind::Punctuation.into(),
                vec![
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Comma.into(), ",")),
                ],
              )
            })
            .then(expr.clone())
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(first_arg, rest)| {
          let mut children = vec![NodeOrToken::Node(first_arg)];
          for (comma, arg) in rest {
            children.push(NodeOrToken::Node(comma));
            children.push(NodeOrToken::Node(arg));
          }
          GreenNode::new(SyntaxKind::ArgList.into(), children)
        })
        .or_not(),
    )
    .then(whitespace().then(just(')')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::RParen.into(), ")")),
        ],
      )
    }))
    .map(|((lparen, args), rparen)| {
      let mut children = vec![NodeOrToken::Node(lparen)];
      if let Some(arg_list) = args {
        children.push(NodeOrToken::Node(arg_list));
      }
      children.push(NodeOrToken::Node(rparen));

      GreenNode::new(SyntaxKind::CallExpr.into(), children)
    })
}

pub fn expr<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  let whitespace = whitespace();

  let int = int();

  let float = float();

  let string = string();

  let boot = bool();

  let ident = ident();

  let expr = recursive(|expr| {
    let func = func(expr.clone());
    let atom = choice((float, int, string, boot, func, ident));
    let paren_expr = whitespace
      .clone()
      .then(just('('))
      .then(expr.clone())
      .then(whitespace.clone())
      .then(just(')'))
      .map(|((((ws1, _), inner), ws2), _)| {
        GreenNode::new(
          SyntaxKind::ParenExpr.into(),
          vec![
            NodeOrToken::Node(GreenNode::new(
              SyntaxKind::Punctuation.into(),
              vec![
                NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
                NodeOrToken::Token(GreenToken::new(SyntaxKind::LParen.into(), "(")),
              ],
            )),
            NodeOrToken::Node(inner),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws2.leak())),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::RParen.into(), ")")),
          ],
        )
      });

    let primary = choice((paren_expr, atom.clone()));
    let call_args = call_args(expr.clone());

    primary.pratt((
      postfix(7, call_args, |lhs, args, _| {
        GreenNode::new(
          SyntaxKind::CallExpr.into(),
          vec![NodeOrToken::Node(lhs), NodeOrToken::Node(args)],
        )
      }),
      prefix(
        6,
        whitespace.clone().then(just('-')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Minus.into(), "-")),
            ],
          )
        }),
        |op, rhs: Cst, _| {
          GreenNode::new(
            SyntaxKind::PrefixExpr.into(),
            vec![NodeOrToken::Node(op), NodeOrToken::Node(rhs)],
          )
        },
      ),
      prefix(
        6,
        whitespace.clone().then(just("not")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::KwNot.into(), "not")),
            ],
          )
        }),
        |op, rhs: Cst, _| {
          GreenNode::new(
            SyntaxKind::PrefixExpr.into(),
            vec![NodeOrToken::Node(op), NodeOrToken::Node(rhs)],
          )
        },
      ),
      infix(
        left(5),
        whitespace.clone().then(just('*')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Star.into(), "*")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(5),
        whitespace.clone().then(just('/')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Slash.into(), "/")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(4),
        whitespace.clone().then(just('+')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Plus.into(), "+")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(4),
        whitespace.clone().then(just('-')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Minus.into(), "-")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(3),
        whitespace.clone().then(just("==")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "==")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(3),
        whitespace.clone().then(just("!=")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::NotEq.into(), "!=")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(3),
        whitespace.clone().then(just("<=")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::LtEq.into(), "<=")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(3),
        whitespace.clone().then(just(">=")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::GtEq.into(), ">=")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(3),
        whitespace.clone().then(just('<')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Lt.into(), "<")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(3),
        whitespace.clone().then(just('>')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Gt.into(), ">")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(2),
        whitespace.clone().then(just("and")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::KwAnd.into(), "and")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
      infix(
        left(1),
        whitespace.clone().then(just("or")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::KwOr.into(), "or")),
            ],
          )
        }),
        |lhs, op, rhs, _| {
          GreenNode::new(
            SyntaxKind::BinaryExpr.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(op),
              NodeOrToken::Node(rhs),
            ],
          )
        },
      ),
    ))
  });

  expr
}

pub fn top_level_expr<'src>()
-> impl Parser<'src, &'src str, SyntaxNode, extra::Err<Rich<'src, char>>> {
  let_in()
    .repeated()
    .at_least(1)
    .collect::<Vec<_>>()
    .map(|lets| {
      SyntaxNode::new_root(GreenNode::new(
        SyntaxKind::Root.into(),
        lets
          .into_iter()
          .map(|node| NodeOrToken::Node(node))
          .collect::<Vec<_>>(),
      ))
    })
}

pub fn parser<'src>() -> impl Parser<'src, &'src str, SyntaxNode, extra::Err<Rich<'src, char>>> {
  top_level_expr()
}
