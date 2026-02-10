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

fn binding<'src>(
  keyword_str: &'static str,
  keyword_kind: SyntaxKind,
  expr_kind: SyntaxKind,
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  recursive(move |binding_expr| {
    whitespace()
      .then(keyword(keyword_str))
      .map(move |(ws, _)| {
        GreenNode::new(
          SyntaxKind::Punctuation.into(),
          vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Token(GreenToken::new(keyword_kind.into(), keyword_str)),
          ],
        )
      })
      .then(ident())
      .then(
        whitespace()
          .then(just("->"))
          .then(whitespace())
          .then(type_expr())
          .map(|(((ws1, _), ws2), ty)| (ws1, ws2, ty))
          .or_not(),
      )
      .then(whitespace().then(just(':')).map(|(ws, _)| {
        GreenNode::new(
          SyntaxKind::Punctuation.into(),
          vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
          ],
        )
      }))
      .then(expr.clone())
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
          .then(choice((binding_expr.clone(), expr.clone())))
          .map(|(in_kw, in_expr)| {
            GreenNode::new(
              SyntaxKind::InExpr.into(),
              vec![NodeOrToken::Node(in_kw), NodeOrToken::Node(in_expr)],
            )
          })
          .or_not(),
      )
      .map(
        move |(((((kw, id), maybe_type), colon), value), maybe_in)| {
          let mut children = vec![NodeOrToken::Node(kw), NodeOrToken::Node(id)];

          if let Some((ws1, ws2, ty)) = maybe_type {
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws1.leak(),
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Arrow.into(),
              "->",
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws2.leak(),
            )));
            children.push(NodeOrToken::Node(ty));
          }

          children.push(NodeOrToken::Node(colon));
          children.push(NodeOrToken::Node(value));

          if let Some(in_expr) = maybe_in {
            children.push(NodeOrToken::Node(in_expr));
          }

          GreenNode::new(expr_kind.into(), children)
        },
      )
  })
}

fn let_in<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  binding("let", SyntaxKind::KwLet, SyntaxKind::LetExpr, expr)
}

fn ref_in<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  binding("ref", SyntaxKind::KwRef, SyntaxKind::RefBindingExpr, expr)
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

fn field_access<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(just('.'))
    .then(ident())
    .map(|((ws, _), field)| {
      GreenNode::new(
        SyntaxKind::FieldAccess.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Dot.into(), ".")),
          NodeOrToken::Node(field),
        ],
      )
    })
}

fn method_call<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(just("|>"))
    .then(ident())
    .then(call_args(expr))
    .map(|(((ws, _), method), args)| {
      GreenNode::new(
        SyntaxKind::MethodCall.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Pipe.into(), "|>")),
          NodeOrToken::Node(method),
          NodeOrToken::Node(args),
        ],
      )
    })
}

fn struct_def<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(keyword("struct"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwStruct.into(), "struct")),
        ],
      )
    })
    .then(whitespace().then(just('{')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::LBrace.into(), "{")),
        ],
      )
    }))
    .then(
      whitespace()
        .then(ident())
        .then(whitespace().then(just('=')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
            ],
          )
        }))
        .then(type_expr())
        .map(|(((ws, name), eq), ty)| {
          GreenNode::new(
            SyntaxKind::StructField.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Node(name),
              NodeOrToken::Node(eq),
              NodeOrToken::Node(ty),
            ],
          )
        })
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
            .then(
              whitespace()
                .then(ident())
                .then(whitespace().then(just('=')).map(|(ws, _)| {
                  GreenNode::new(
                    SyntaxKind::Punctuation.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
                    ],
                  )
                }))
                .then(type_expr())
                .map(|(((ws, name), eq), ty)| {
                  GreenNode::new(
                    SyntaxKind::StructField.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Node(name),
                      NodeOrToken::Node(eq),
                      NodeOrToken::Node(ty),
                    ],
                  )
                }),
            )
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(first_field, rest)| {
          let mut children = vec![NodeOrToken::Node(first_field)];
          for (comma, field) in rest {
            children.push(NodeOrToken::Node(comma));
            children.push(NodeOrToken::Node(field));
          }
          children
        }),
    )
    .then(whitespace().then(just('}')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::RBrace.into(), "}")),
        ],
      )
    }))
    .map(|(((struct_kw, lbrace), fields), rbrace)| {
      let mut children = vec![NodeOrToken::Node(struct_kw), NodeOrToken::Node(lbrace)];
      children.extend(fields);
      children.push(NodeOrToken::Node(rbrace));

      GreenNode::new(SyntaxKind::StructDef.into(), children)
    })
}

fn new_expr<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(keyword("new"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwNew.into(), "new")),
        ],
      )
    })
    .then(ident())
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
      whitespace()
        .then(ident())
        .then(whitespace().then(just('=')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
            ],
          )
        }))
        .then(expr.clone())
        .map(|(((ws, name), eq), value)| {
          GreenNode::new(
            SyntaxKind::StructField.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Node(name),
              NodeOrToken::Node(eq),
              NodeOrToken::Node(value),
            ],
          )
        })
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
            .then(
              whitespace()
                .then(ident())
                .then(whitespace().then(just('=')).map(|(ws, _)| {
                  GreenNode::new(
                    SyntaxKind::Punctuation.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
                    ],
                  )
                }))
                .then(expr.clone())
                .map(|(((ws, name), eq), value)| {
                  GreenNode::new(
                    SyntaxKind::StructField.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Node(name),
                      NodeOrToken::Node(eq),
                      NodeOrToken::Node(value),
                    ],
                  )
                }),
            )
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(first_field, rest)| {
          let mut children = vec![NodeOrToken::Node(first_field)];
          for (comma, field) in rest {
            children.push(NodeOrToken::Node(comma));
            children.push(NodeOrToken::Node(field));
          }
          GreenNode::new(SyntaxKind::ArgList.into(), children)
        }),
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
    .then(
      whitespace()
        .then(keyword("with"))
        .map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::KwWith.into(), "with")),
            ],
          )
        })
        .then(ident())
        .map(|(with_kw, behavior)| {
          GreenNode::new(
            SyntaxKind::WithClause.into(),
            vec![NodeOrToken::Node(with_kw), NodeOrToken::Node(behavior)],
          )
        })
        .or_not(),
    )
    .map(
      |(((((new_kw, struct_name), lparen), fields), rparen), maybe_with)| {
        let mut children = vec![
          NodeOrToken::Node(new_kw),
          NodeOrToken::Node(struct_name),
          NodeOrToken::Node(lparen),
          NodeOrToken::Node(fields),
          NodeOrToken::Node(rparen),
        ];

        if let Some(with_clause) = maybe_with {
          children.push(NodeOrToken::Node(with_clause));
        }

        GreenNode::new(SyntaxKind::NewExpr.into(), children)
      },
    )
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
            .then(just(':'))
            .then(whitespace())
            .then(type_expr())
            .map(|(((ws1, _), ws2), ty)| (ws1, ws2, ty))
            .or_not(),
        )
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
            .then(
              ident().then(
                whitespace()
                  .then(just(':'))
                  .then(whitespace())
                  .then(type_expr())
                  .map(|(((ws1, _), ws2), ty)| (ws1, ws2, ty))
                  .or_not(),
              ),
            )
            .repeated()
            .collect::<Vec<_>>(),
        )
        .repeated()
        .collect::<Vec<_>>()
        .map(|params| {
          let mut children = vec![];
          for ((first_param, maybe_type), rest) in params {
            children.push(NodeOrToken::Node(first_param));
            if let Some((ws1, ws2, ty)) = maybe_type {
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Whitespace.into(),
                ws1.leak(),
              )));
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Colon.into(),
                ":",
              )));
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Whitespace.into(),
                ws2.leak(),
              )));
              children.push(NodeOrToken::Node(ty));
            }
            for (comma, (param, maybe_type)) in rest {
              children.push(NodeOrToken::Node(comma));
              children.push(NodeOrToken::Node(param));
              if let Some((ws1, ws2, ty)) = maybe_type {
                children.push(NodeOrToken::Token(GreenToken::new(
                  SyntaxKind::Whitespace.into(),
                  ws1.leak(),
                )));
                children.push(NodeOrToken::Token(GreenToken::new(
                  SyntaxKind::Colon.into(),
                  ":",
                )));
                children.push(NodeOrToken::Token(GreenToken::new(
                  SyntaxKind::Whitespace.into(),
                  ws2.leak(),
                )));
                children.push(NodeOrToken::Node(ty));
              }
            }
          }
          GreenNode::new(SyntaxKind::ParamList.into(), children)
        }),
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
    .then(
      whitespace()
        .then(just("->"))
        .then(whitespace())
        .then(type_expr())
        .map(|(((ws1, _), ws2), ty)| (ws1, ws2, ty))
        .or_not(),
    )
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
    .map(
      |((((((fn_kw, lparen), param_list), rparen), maybe_return), colon), body)| {
        let mut children = vec![
          NodeOrToken::Node(fn_kw),
          NodeOrToken::Node(lparen),
          NodeOrToken::Node(param_list),
          NodeOrToken::Node(rparen),
        ];

        if let Some((ws1, ws2, return_type)) = maybe_return {
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws1.leak(),
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Arrow.into(),
            "->",
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws2.leak(),
          )));
          children.push(NodeOrToken::Node(return_type));
        }

        children.push(NodeOrToken::Node(colon));
        children.push(NodeOrToken::Node(body));

        GreenNode::new(SyntaxKind::FnExpr.into(), children)
      },
    )
}

fn method_def<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(keyword("def"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwDef.into(), "def")),
        ],
      )
    })
    .then(ident())
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
            .then(just(':'))
            .then(whitespace())
            .then(type_expr())
            .map(|(((ws1, _), ws2), ty)| (ws1, ws2, ty))
            .or_not(),
        )
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
            .then(
              ident().then(
                whitespace()
                  .then(just(':'))
                  .then(whitespace())
                  .then(type_expr())
                  .map(|(((ws1, _), ws2), ty)| (ws1, ws2, ty))
                  .or_not(),
              ),
            )
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|((first_param, maybe_type), rest)| {
          let mut children = vec![NodeOrToken::Node(first_param)];
          if let Some((ws1, ws2, ty)) = maybe_type {
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws1.leak(),
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Colon.into(),
              ":",
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws2.leak(),
            )));
            children.push(NodeOrToken::Node(ty));
          }
          for (comma, (param, maybe_type)) in rest {
            children.push(NodeOrToken::Node(comma));
            children.push(NodeOrToken::Node(param));
            if let Some((ws1, ws2, ty)) = maybe_type {
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Whitespace.into(),
                ws1.leak(),
              )));
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Colon.into(),
                ":",
              )));
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Whitespace.into(),
                ws2.leak(),
              )));
              children.push(NodeOrToken::Node(ty));
            }
          }
          GreenNode::new(SyntaxKind::ParamList.into(), children)
        })
        .or_not()
        .map(|p| p.unwrap_or_else(|| GreenNode::new(SyntaxKind::ParamList.into(), vec![]))),
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
    .then(
      whitespace()
        .then(just("->"))
        .then(whitespace())
        .then(type_expr())
        .map(|(((ws1, _), ws2), ty)| (ws1, ws2, ty))
        .or_not(),
    )
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
    .map(
      |(((((((def_kw, name), lparen), param_list), rparen), maybe_return), colon), body)| {
        let mut children = vec![
          NodeOrToken::Node(def_kw),
          NodeOrToken::Node(name),
          NodeOrToken::Node(lparen),
          NodeOrToken::Node(param_list),
          NodeOrToken::Node(rparen),
        ];

        if let Some((ws1, ws2, return_type)) = maybe_return {
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws1.leak(),
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Arrow.into(),
            "->",
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws2.leak(),
          )));
          children.push(NodeOrToken::Node(return_type));
        }

        children.push(NodeOrToken::Node(colon));
        children.push(NodeOrToken::Node(body));

        GreenNode::new(SyntaxKind::MethodDef.into(), children)
      },
    )
}

fn behavior_def<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(keyword("concept"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwConcept.into(), "concept")),
        ],
      )
    })
    .then(ident())
    .then(whitespace().then(keyword("requires")).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwRequires.into(), "requires")),
        ],
      )
    }))
    .then(whitespace().then(just('{')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::LBrace.into(), "{")),
        ],
      )
    }))
    .then(
      whitespace()
        .then(ident())
        .then(whitespace().then(just('=')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
            ],
          )
        }))
        .then(type_expr())
        .map(|(((ws, name), eq), ty)| {
          GreenNode::new(
            SyntaxKind::StructField.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Node(name),
              NodeOrToken::Node(eq),
              NodeOrToken::Node(ty),
            ],
          )
        })
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
            .then(
              whitespace()
                .then(ident())
                .then(whitespace().then(just('=')).map(|(ws, _)| {
                  GreenNode::new(
                    SyntaxKind::Punctuation.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
                    ],
                  )
                }))
                .then(type_expr())
                .map(|(((ws, name), eq), ty)| {
                  GreenNode::new(
                    SyntaxKind::StructField.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Node(name),
                      NodeOrToken::Node(eq),
                      NodeOrToken::Node(ty),
                    ],
                  )
                }),
            )
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(first_field, rest)| {
          let mut children = vec![NodeOrToken::Node(first_field)];
          for (comma, field) in rest {
            children.push(NodeOrToken::Node(comma));
            children.push(NodeOrToken::Node(field));
          }
          children
        }),
    )
    .then(whitespace().then(just('}')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::RBrace.into(), "}")),
        ],
      )
    }))
    .then(whitespace().then(keyword("with")).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwWith.into(), "with")),
        ],
      )
    }))
    .then(whitespace().then(just('{')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::LBrace.into(), "{")),
        ],
      )
    }))
    .then(method_def(expr.clone()).repeated().collect::<Vec<_>>())
    .then(whitespace().then(just('}')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::RBrace.into(), "}")),
        ],
      )
    }))
    .map(
      |(
        (
          (
            ((((((concept_kw, name), requires_kw), lbrace1), req_fields), rbrace1), with_kw),
            lbrace2,
          ),
          methods,
        ),
        rbrace2,
      )| {
        let mut children = vec![
          NodeOrToken::Node(concept_kw),
          NodeOrToken::Node(name),
          NodeOrToken::Node(requires_kw),
          NodeOrToken::Node(lbrace1),
        ];

        children.extend(req_fields);

        children.push(NodeOrToken::Node(rbrace1));
        children.push(NodeOrToken::Node(with_kw));
        children.push(NodeOrToken::Node(lbrace2));

        for method in methods {
          children.push(NodeOrToken::Node(method));
        }

        children.push(NodeOrToken::Node(rbrace2));

        GreenNode::new(SyntaxKind::BehaviorDef.into(), children)
      },
    )
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

fn pattern<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  choice((ident(), int(), float(), string(), bool()))
}

fn when_clause<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(keyword("when"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwWhen.into(), "when")),
        ],
      )
    })
    .then(expr)
    .map(|(when_kw, condition)| {
      GreenNode::new(
        SyntaxKind::WhenClause.into(),
        vec![NodeOrToken::Node(when_kw), NodeOrToken::Node(condition)],
      )
    })
}

fn match_expr<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  let of_branch = whitespace()
    .then(keyword("of"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwOf.into(), "of")),
        ],
      )
    })
    .then(choice((
      keyword("else").map(|_| {
        GreenNode::new(
          SyntaxKind::Punctuation.into(),
          vec![NodeOrToken::Token(GreenToken::new(
            SyntaxKind::KwElse.into(),
            "else",
          ))],
        )
      }),
      pattern(),
    )))
    .then(when_clause(expr.clone()).or_not())
    .then(whitespace().then(just(':')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
        ],
      )
    }))
    .then(choice((let_in(expr.clone()), expr.clone())))
    .map(|((((of_kw, pattern), maybe_when), colon), branch_expr)| {
      let mut children = vec![NodeOrToken::Node(of_kw), NodeOrToken::Node(pattern)];
      if let Some(when_clause) = maybe_when {
        children.push(NodeOrToken::Node(when_clause));
      } else {
        children.push(NodeOrToken::Node(GreenNode::new(
          SyntaxKind::WhenClause.into(),
          vec![],
        )));
      }
      children.push(NodeOrToken::Node(colon));
      children.push(NodeOrToken::Node(branch_expr));

      GreenNode::new(SyntaxKind::MatchBranch.into(), children)
    });

  whitespace()
    .then(keyword("match"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwMatch.into(), "match")),
        ],
      )
    })
    .then(expr.clone().or_not())
    .then(whitespace().then(just('{')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::LBrace.into(), "{")),
        ],
      )
    }))
    .then(of_branch.repeated().at_least(1).collect::<Vec<_>>())
    .then(whitespace().then(just('}')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::RBrace.into(), "}")),
        ],
      )
    }))
    .then(
      whitespace()
        .then(keyword("else"))
        .map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::KwElse.into(), "else")),
            ],
          )
        })
        .then(whitespace().then(just(':')).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
            ],
          )
        }))
        .then(choice((let_in(expr.clone()), expr.clone())))
        .map(|((else_kw, colon), else_expr)| {
          GreenNode::new(
            SyntaxKind::ElseClause.into(),
            vec![
              NodeOrToken::Node(else_kw),
              NodeOrToken::Node(colon),
              NodeOrToken::Node(else_expr),
            ],
          )
        })
        .or_not(),
    )
    .map(
      |(((((match_kw, maybe_target), lbrace), branches), rbrace), maybe_else)| {
        let mut children = vec![NodeOrToken::Node(match_kw)];

        if let Some(target) = maybe_target {
          children.push(NodeOrToken::Node(target));
        } else {
          children.push(NodeOrToken::Node(GreenNode::new(
            SyntaxKind::MatchTarget.into(),
            vec![],
          )));
        }

        children.push(NodeOrToken::Node(lbrace));

        for branch in branches {
          children.push(NodeOrToken::Node(branch));
        }

        children.push(NodeOrToken::Node(rbrace));

        if let Some(else_clause) = maybe_else {
          children.push(NodeOrToken::Node(else_clause));
        } else {
          children.push(NodeOrToken::Node(GreenNode::new(
            SyntaxKind::ElseClause.into(),
            vec![],
          )));
        }

        GreenNode::new(SyntaxKind::MatchExpr.into(), children)
      },
    )
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
    let match_expr = match_expr(expr.clone());
    let new_expr = new_expr(expr.clone());
    let atom = choice((float, int, string, boot, match_expr, func, new_expr, ident));

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
    let field_access = field_access();
    let method_call = method_call(expr.clone());

    primary.pratt((
      postfix(8, field_access, |lhs, field, _| {
        GreenNode::new(
          SyntaxKind::FieldAccess.into(),
          vec![NodeOrToken::Node(lhs), NodeOrToken::Node(field)],
        )
      }),
      postfix(8, method_call, |lhs, method, _| {
        GreenNode::new(
          SyntaxKind::MethodCall.into(),
          vec![NodeOrToken::Node(lhs), NodeOrToken::Node(method)],
        )
      }),
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
      prefix(
        6,
        whitespace.clone().then(just("ref")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::KwRef.into(), "ref")),
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
        whitespace.clone().then(just("=")).map(|(ws, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
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

fn type_expr<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|type_expr_rec| {
    let prefix = keyword("ref")
      .then(whitespace())
      .map(|(kw, ws)| (kw, ws))
      .or_not();

    let type_app = whitespace()
      .then(keyword("of"))
      .then(whitespace())
      .then(type_expr_rec.clone())
      .then(
        whitespace()
          .then(just(','))
          .then(whitespace())
          .then(type_expr_rec.clone())
          .repeated()
          .collect::<Vec<_>>(),
      )
      .map(|((((ws1, _), ws2), first), rest)| (ws1, ws2, first, rest))
      .or_not();

    prefix
      .then(ident().then(type_app))
      .map(|(maybe_prefix, (base, maybe_app))| {
        let inner = match maybe_app {
          Some((ws1, ws2, first_arg, rest_args)) => {
            let mut children = vec![NodeOrToken::Node(base)];
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws1.leak(),
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::KwOf.into(),
              "of",
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws2.leak(),
            )));
            children.push(NodeOrToken::Node(first_arg));

            for (((ws_comma, _), ws_after), arg) in rest_args {
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Whitespace.into(),
                ws_comma.leak(),
              )));
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Comma.into(),
                ",",
              )));
              children.push(NodeOrToken::Token(GreenToken::new(
                SyntaxKind::Whitespace.into(),
                ws_after.leak(),
              )));
              children.push(NodeOrToken::Node(arg));
            }

            GreenNode::new(SyntaxKind::TypeApp.into(), children)
          }
          None => GreenNode::new(SyntaxKind::TypeExpr.into(), vec![NodeOrToken::Node(base)]),
        };

        let wrapped = if let Some((kw, ws)) = maybe_prefix {
          let children = vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::KwRef.into(), kw)),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Node(inner),
          ];
          GreenNode::new(SyntaxKind::RefType.into(), children)
        } else {
          inner
        };

        GreenNode::new(
          SyntaxKind::TypeExpr.into(),
          vec![NodeOrToken::Node(wrapped)],
        )
      })
  })
}

pub fn type_decl<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> {
  let struct_type = whitespace()
    .then(keyword("type"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwType.into(), "type")),
        ],
      )
    })
    .then(whitespace().then(ident()))
    .then(whitespace().then(just('=')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Eq.into(), "=")),
        ],
      )
    }))
    .then(struct_def())
    .map(|(((type_kw, (ws, name)), eq), struct_def)| {
      GreenNode::new(
        SyntaxKind::TypeDecl.into(),
        vec![
          NodeOrToken::Node(type_kw),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Node(name),
          NodeOrToken::Node(eq),
          NodeOrToken::Node(struct_def),
        ],
      )
    });

  let type_constructor = whitespace()
    .then(ident())
    .then(
      whitespace()
        .then(keyword("of"))
        .then(whitespace())
        .then(type_expr())
        .then(
          whitespace()
            .then(just(','))
            .then(whitespace())
            .then(type_expr())
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|((((ws_of, _), ws1), first), rest)| (ws_of, ws1, first, rest))
        .or_not(),
    )
    .then(
      whitespace()
        .then(just("->"))
        .then(whitespace())
        .then(type_expr())
        .or_not(),
    )
    .map(|(((ws1, name), maybe_params), maybe_arrow)| {
      let mut children = vec![
        NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
        NodeOrToken::Node(name),
      ];

      if let Some((ws_of, ws2, first_param, rest_params)) = maybe_params {
        children.push(NodeOrToken::Token(GreenToken::new(
          SyntaxKind::Whitespace.into(),
          ws_of.leak(),
        )));
        children.push(NodeOrToken::Token(GreenToken::new(
          SyntaxKind::KwOf.into(),
          "of",
        )));
        children.push(NodeOrToken::Token(GreenToken::new(
          SyntaxKind::Whitespace.into(),
          ws2.leak(),
        )));
        children.push(NodeOrToken::Node(first_param));

        for (((ws_comma, _), ws_after), param) in rest_params {
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws_comma.leak(),
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Comma.into(),
            ",",
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws_after.leak(),
          )));
          children.push(NodeOrToken::Node(param));
        }
      }

      if let Some((((ws3, _), ws4), return_type)) = maybe_arrow {
        children.push(NodeOrToken::Token(GreenToken::new(
          SyntaxKind::Whitespace.into(),
          ws3.leak(),
        )));
        children.push(NodeOrToken::Token(GreenToken::new(
          SyntaxKind::Arrow.into(),
          "->",
        )));
        children.push(NodeOrToken::Token(GreenToken::new(
          SyntaxKind::Whitespace.into(),
          ws4.leak(),
        )));
        children.push(NodeOrToken::Node(return_type));
      }

      GreenNode::new(SyntaxKind::TypeConstructor.into(), children)
    });

  let union_type = whitespace()
    .then(keyword("type"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwType.into(), "type")),
        ],
      )
    })
    .then(whitespace().then(ident()))
    .then(
      whitespace()
        .then(keyword("of"))
        .then(whitespace())
        .then(ident())
        .then(
          whitespace()
            .then(just(','))
            .then(whitespace())
            .then(ident())
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|((((ws_of, _), ws1), first), rest)| (ws_of, ws1, first, rest))
        .or_not(),
    )
    .then(whitespace().then(just(':')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
        ],
      )
    }))
    .then(
      type_constructor
        .clone()
        .then(
          whitespace()
            .then(keyword("or"))
            .then(type_constructor)
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
          let mut all = vec![first];
          all.extend(rest.into_iter().map(|((_, _), c)| c));
          all
        }),
    )
    .map(
      |((((type_kw, (ws1, name)), type_params), colon), constructors)| {
        let mut children = vec![
          NodeOrToken::Node(type_kw),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
          NodeOrToken::Node(name),
        ];

        if let Some((ws2, ws3, param, rest)) = type_params {
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws2.leak(),
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::KwOf.into(),
            "of",
          )));
          children.push(NodeOrToken::Token(GreenToken::new(
            SyntaxKind::Whitespace.into(),
            ws3.leak(),
          )));
          children.push(NodeOrToken::Node(param));

          for (((ws_comma, _), ws_after), p) in rest {
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws_comma.leak(),
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Comma.into(),
              ",",
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              ws_after.leak(),
            )));
            children.push(NodeOrToken::Node(p));
          }
        }

        children.push(NodeOrToken::Node(colon));

        for (i, constructor) in constructors.into_iter().enumerate() {
          if i > 0 {
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::Whitespace.into(),
              " ".to_string().leak(),
            )));
            children.push(NodeOrToken::Token(GreenToken::new(
              SyntaxKind::KwOr.into(),
              "or",
            )));
          }
          children.push(NodeOrToken::Node(constructor));
        }

        GreenNode::new(SyntaxKind::TypeDecl.into(), children)
      },
    );

  choice((struct_type, union_type))
}

pub fn top_level_expr<'src>()
-> impl Parser<'src, &'src str, SyntaxNode, extra::Err<Rich<'src, char>>> {
  choice((
    type_decl(),
    behavior_def(expr()),
    let_in(expr()),
    ref_in(expr()),
  ))
  .repeated()
  .at_least(1)
  .collect::<Vec<_>>()
  .map(|items| {
    SyntaxNode::new_root(GreenNode::new(
      SyntaxKind::Root.into(),
      items
        .into_iter()
        .map(|node| NodeOrToken::Node(node))
        .collect::<Vec<_>>(),
    ))
  })
}

pub fn parser<'src>() -> impl Parser<'src, &'src str, SyntaxNode, extra::Err<Rich<'src, char>>> {
  top_level_expr()
}
