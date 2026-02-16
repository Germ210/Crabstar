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
  choice((
    any::<&'src str, extra::Err<Rich<'src, char>>>()
      .filter(|c: &char| c.is_whitespace())
      .map(|c| c.to_string()),
    just('#')
      .then(
        any()
          .filter(|c: &char| *c != '\n')
          .repeated()
          .collect::<String>(),
      )
      .then(just('\n').or_not())
      .map(
        |((_hash, comment), newline): ((char, String), Option<char>)| {
          format!("#{}{}", comment, newline.unwrap_or('\n'))
        },
      ),
  ))
  .repeated()
  .collect::<Vec<String>>()
  .map(|parts| parts.join(""))
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
          .map(|(ws1, _)| {
            GreenNode::new(
              SyntaxKind::Punctuation.into(),
              vec![
                NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
                NodeOrToken::Token(GreenToken::new(SyntaxKind::Arrow.into(), "->")),
              ],
            )
          })
          .then(type_expr())
          .or_not()
          .map(|opt| {
            opt.unwrap_or_else(|| {
              (
                GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
                GreenNode::new(SyntaxKind::TypeExpr.into(), vec![]),
              )
            })
          }),
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
          .or_not()
          .map(|opt| opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::InExpr.into(), vec![]))),
      )
      .map(
        move |(((((kw, id), (arrow, ty)), colon), value), in_expr)| {
          GreenNode::new(
            expr_kind.into(),
            vec![
              NodeOrToken::Node(kw),
              NodeOrToken::Node(id),
              NodeOrToken::Node(arrow),
              NodeOrToken::Node(ty),
              NodeOrToken::Node(colon),
              NodeOrToken::Node(value),
              NodeOrToken::Node(in_expr),
            ],
          )
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

fn field_access<'src>()
-> impl Parser<'src, &'src str, (Cst, Cst), extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(just('.'))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Dot.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Dot.into(), ".")),
        ],
      )
    })
    .then(ident())
}

fn method_call<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, (Cst, Cst, Cst, Cst, Cst), extra::Err<Rich<'src, char>>> + Clone {
  whitespace()
    .then(just("|>"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Pipe.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Pipe.into(), "|>")),
        ],
      )
    })
    .then(ident())
    .then(call_args(expr))
    .map(|((pipe, method), (lparen, args, rparen))| (pipe, method, lparen, args, rparen))
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
    .map(|((((new_kw, struct_name), lparen), fields), rparen)| {
      GreenNode::new(
        SyntaxKind::NewExpr.into(),
        vec![
          NodeOrToken::Node(new_kw),
          NodeOrToken::Node(struct_name),
          NodeOrToken::Node(lparen),
          NodeOrToken::Node(fields),
          NodeOrToken::Node(rparen),
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
      whitespace()
        .then(ident())
        .then(
          whitespace()
            .then(just(':'))
            .then(whitespace())
            .map(|((ws1, _), ws2)| {
              GreenNode::new(
                SyntaxKind::Punctuation.into(),
                vec![
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws2.leak())),
                ],
              )
            })
            .then(type_expr())
            .or_not()
            .map(|opt| {
              opt.unwrap_or_else(|| {
                (
                  GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
                  GreenNode::new(SyntaxKind::TypeExpr.into(), vec![]),
                )
              })
            }),
        )
        .map(|((ws, name), (colon, ty))| {
          GreenNode::new(
            SyntaxKind::Param.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Node(name),
              NodeOrToken::Node(colon),
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
                .then(
                  whitespace()
                    .then(just(':'))
                    .then(whitespace())
                    .map(|((ws1, _), ws2)| {
                      GreenNode::new(
                        SyntaxKind::Punctuation.into(),
                        vec![
                          NodeOrToken::Token(GreenToken::new(
                            SyntaxKind::Whitespace.into(),
                            ws1.leak(),
                          )),
                          NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
                          NodeOrToken::Token(GreenToken::new(
                            SyntaxKind::Whitespace.into(),
                            ws2.leak(),
                          )),
                        ],
                      )
                    })
                    .then(type_expr())
                    .or_not()
                    .map(|opt| {
                      opt.unwrap_or_else(|| {
                        (
                          GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
                          GreenNode::new(SyntaxKind::TypeExpr.into(), vec![]),
                        )
                      })
                    }),
                )
                .map(|((ws, name), (colon, ty))| {
                  GreenNode::new(
                    SyntaxKind::Param.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Node(name),
                      NodeOrToken::Node(colon),
                      NodeOrToken::Node(ty),
                    ],
                  )
                }),
            )
            .repeated()
            .collect::<Vec<_>>(),
        )
        .repeated()
        .collect::<Vec<_>>()
        .map(|params| {
          let mut children = vec![];
          for (first_param, rest) in params {
            children.push(NodeOrToken::Node(first_param));
            for (comma, param) in rest {
              children.push(NodeOrToken::Node(comma));
              children.push(NodeOrToken::Node(param));
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
        .map(|(ws1, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Arrow.into(), "->")),
            ],
          )
        })
        .then(type_expr())
        .or_not()
        .map(|opt| {
          opt.unwrap_or_else(|| {
            (
              GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
              GreenNode::new(SyntaxKind::TypeExpr.into(), vec![]),
            )
          })
        }),
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
      |((((((fn_kw, lparen), param_list), rparen), (arrow, return_type)), colon), body)| {
        GreenNode::new(
          SyntaxKind::FnExpr.into(),
          vec![
            NodeOrToken::Node(fn_kw),
            NodeOrToken::Node(lparen),
            NodeOrToken::Node(param_list),
            NodeOrToken::Node(rparen),
            NodeOrToken::Node(arrow),
            NodeOrToken::Node(return_type),
            NodeOrToken::Node(colon),
            NodeOrToken::Node(body),
          ],
        )
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
      whitespace()
        .then(ident())
        .then(
          whitespace()
            .then(just(':'))
            .then(whitespace())
            .map(|((ws1, _), ws2)| {
              GreenNode::new(
                SyntaxKind::Punctuation.into(),
                vec![
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
                  NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws2.leak())),
                ],
              )
            })
            .then(type_expr())
            .or_not()
            .map(|opt| {
              opt.unwrap_or_else(|| {
                (
                  GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
                  GreenNode::new(SyntaxKind::TypeExpr.into(), vec![]),
                )
              })
            }),
        )
        .map(|((ws, name), (colon, ty))| {
          GreenNode::new(
            SyntaxKind::Param.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
              NodeOrToken::Node(name),
              NodeOrToken::Node(colon),
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
                .then(
                  whitespace()
                    .then(just(':'))
                    .then(whitespace())
                    .map(|((ws1, _), ws2)| {
                      GreenNode::new(
                        SyntaxKind::Punctuation.into(),
                        vec![
                          NodeOrToken::Token(GreenToken::new(
                            SyntaxKind::Whitespace.into(),
                            ws1.leak(),
                          )),
                          NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
                          NodeOrToken::Token(GreenToken::new(
                            SyntaxKind::Whitespace.into(),
                            ws2.leak(),
                          )),
                        ],
                      )
                    })
                    .then(type_expr())
                    .or_not()
                    .map(|opt| {
                      opt.unwrap_or_else(|| {
                        (
                          GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
                          GreenNode::new(SyntaxKind::TypeExpr.into(), vec![]),
                        )
                      })
                    }),
                )
                .map(|((ws, name), (colon, ty))| {
                  GreenNode::new(
                    SyntaxKind::Param.into(),
                    vec![
                      NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                      NodeOrToken::Node(name),
                      NodeOrToken::Node(colon),
                      NodeOrToken::Node(ty),
                    ],
                  )
                }),
            )
            .repeated()
            .collect::<Vec<_>>(),
        )
        .repeated()
        .collect::<Vec<_>>()
        .map(|params| {
          let mut children = vec![];
          for (first_param, rest) in params {
            children.push(NodeOrToken::Node(first_param));
            for (comma, param) in rest {
              children.push(NodeOrToken::Node(comma));
              children.push(NodeOrToken::Node(param));
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
        .map(|(ws1, _)| {
          GreenNode::new(
            SyntaxKind::Punctuation.into(),
            vec![
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws1.leak())),
              NodeOrToken::Token(GreenToken::new(SyntaxKind::Arrow.into(), "->")),
            ],
          )
        })
        .then(type_expr())
        .or_not()
        .map(|opt| {
          opt.unwrap_or_else(|| {
            (
              GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
              GreenNode::new(SyntaxKind::TypeExpr.into(), vec![]),
            )
          })
        }),
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
      |(
        ((((((def_kw, name), lparen), param_list), rparen), (arrow, return_type)), colon),
        body,
      )| {
        GreenNode::new(
          SyntaxKind::MethodDef.into(),
          vec![
            NodeOrToken::Node(def_kw),
            NodeOrToken::Node(name),
            NodeOrToken::Node(lparen),
            NodeOrToken::Node(param_list),
            NodeOrToken::Node(rparen),
            NodeOrToken::Node(arrow),
            NodeOrToken::Node(return_type),
            NodeOrToken::Node(colon),
            NodeOrToken::Node(body),
          ],
        )
      },
    )
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
    .or_not()
    .map(|opt| {
      opt.unwrap_or_else(|| {
        (
          GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
          GreenNode::new(SyntaxKind::Root.into(), vec![]),
        )
      })
    })
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
    .then(pattern())
    .then(when_clause(expr.clone()))
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
    .map(|((((of_kw, pattern), when_clause), colon), branch_expr)| {
      GreenNode::new(
        SyntaxKind::MatchBranch.into(),
        vec![
          NodeOrToken::Node(of_kw),
          NodeOrToken::Node(pattern),
          NodeOrToken::Node(when_clause),
          NodeOrToken::Node(colon),
          NodeOrToken::Node(branch_expr),
        ],
      )
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
    .then(expr.clone())
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
      of_branch
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|branches| {
          GreenNode::new(
            SyntaxKind::MatchBranches.into(),
            branches
              .into_iter()
              .map(NodeOrToken::Node)
              .collect::<Vec<_>>(),
          )
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
        .or_not()
        .map(|opt| opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::ElseClause.into(), vec![]))),
    )
    .map(
      |(((((match_kw, target), lbrace), match_branches), rbrace), else_clause)| {
        GreenNode::new(
          SyntaxKind::MatchExpr.into(),
          vec![
            NodeOrToken::Node(match_kw),
            NodeOrToken::Node(target),
            NodeOrToken::Node(lbrace),
            NodeOrToken::Node(match_branches),
            NodeOrToken::Node(rbrace),
            NodeOrToken::Node(else_clause),
          ],
        )
      },
    )
}

fn call_args<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, (Cst, Cst, Cst), extra::Err<Rich<'src, char>>> + Clone {
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
            .or_not(),
        )
        .map(|(arg_expr, comma_opt)| {
          GreenNode::new(
            SyntaxKind::Arg.into(),
            vec![
              NodeOrToken::Node(arg_expr),
              NodeOrToken::Node(
                comma_opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::Punctuation.into(), vec![])),
              ),
            ],
          )
        })
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|args| {
          GreenNode::new(
            SyntaxKind::ArgList.into(),
            args.into_iter().map(NodeOrToken::Node).collect::<Vec<_>>(),
          )
        })
        .or_not()
        .map(|opt| opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::ArgList.into(), vec![]))),
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
    .map(|((lparen, args), rparen)| (lparen, args, rparen))
}

fn type_expr<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  recursive(|type_expr_rec| {
    let type_arg_list = whitespace()
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
      .then(
        type_expr_rec
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
              .or_not()
              .map(|opt| {
                opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::Punctuation.into(), vec![]))
              }),
          )
          .map(|(ty, comma)| {
            GreenNode::new(
              SyntaxKind::TypeArg.into(),
              vec![NodeOrToken::Node(ty), NodeOrToken::Node(comma)],
            )
          })
          .repeated()
          .at_least(1)
          .collect::<Vec<_>>(),
      )
      .or_not()
      .map(|opt| {
        opt.unwrap_or_else(|| {
          (
            GreenNode::new(SyntaxKind::Punctuation.into(), vec![]),
            vec![],
          )
        })
      });

    let type_app = ident().then(type_arg_list).map(|(base, (of_kw, args))| {
      let mut children = vec![NodeOrToken::Node(base), NodeOrToken::Node(of_kw)];

      let arg_list_children: Vec<_> = args.into_iter().map(NodeOrToken::Node).collect();
      children.push(NodeOrToken::Node(GreenNode::new(
        SyntaxKind::TypeArgList.into(),
        arg_list_children,
      )));

      GreenNode::new(SyntaxKind::TypeApp.into(), children)
    });

    let ref_prefix = whitespace().then(keyword("ref")).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwRef.into(), "ref")),
        ],
      )
    });

    choice((
      ref_prefix.then(type_app.clone()).map(|(ref_kw, app)| {
        GreenNode::new(
          SyntaxKind::TypeExpr.into(),
          vec![NodeOrToken::Node(GreenNode::new(
            SyntaxKind::RefType.into(),
            vec![NodeOrToken::Node(ref_kw), NodeOrToken::Node(app)],
          ))],
        )
      }),
      type_app.map(|app| GreenNode::new(SyntaxKind::TypeExpr.into(), vec![NodeOrToken::Node(app)])),
    ))
  })
}

fn type_decl<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> {
  let type_param = whitespace()
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
    .map(|(comma, name)| {
      GreenNode::new(
        SyntaxKind::TypeParam.into(),
        vec![NodeOrToken::Node(comma), NodeOrToken::Node(name)],
      )
    });

  let type_param_list = whitespace()
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
    .then(ident())
    .then(type_param.clone().repeated().collect::<Vec<_>>())
    .map(|((of_kw, first), rest)| {
      let mut children = vec![
        NodeOrToken::Node(of_kw),
        NodeOrToken::Node(GreenNode::new(
          SyntaxKind::TypeParam.into(),
          vec![
            NodeOrToken::Node(GreenNode::new(SyntaxKind::Punctuation.into(), vec![])),
            NodeOrToken::Node(first),
          ],
        )),
      ];
      for param in rest {
        children.push(NodeOrToken::Node(param));
      }
      GreenNode::new(SyntaxKind::TypeParamList.into(), children)
    })
    .or_not()
    .map(|opt| opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::TypeParamList.into(), vec![])));

  let type_constructor = {
    let constructor_param = whitespace()
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
      .map(|(comma, ty)| {
        GreenNode::new(
          SyntaxKind::ConstructorParam.into(),
          vec![NodeOrToken::Node(comma), NodeOrToken::Node(ty)],
        )
      });

    let constructor_param_list = whitespace()
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
      .then(ident())
      .then(constructor_param.repeated().collect::<Vec<_>>())
      .map(|((of_kw, first), rest)| {
        let mut children = vec![
          NodeOrToken::Node(of_kw),
          NodeOrToken::Node(GreenNode::new(
            SyntaxKind::ConstructorParam.into(),
            vec![
              NodeOrToken::Node(GreenNode::new(SyntaxKind::Punctuation.into(), vec![])),
              NodeOrToken::Node(first),
            ],
          )),
        ];
        for param in rest {
          children.push(NodeOrToken::Node(param));
        }
        GreenNode::new(SyntaxKind::ConstructorParamList.into(), children)
      })
      .or_not()
      .map(|opt| {
        opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::ConstructorParamList.into(), vec![]))
      });

    let return_type_param = whitespace()
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
      .map(|(comma, ty)| {
        GreenNode::new(
          SyntaxKind::ConstructorParam.into(),
          vec![NodeOrToken::Node(comma), NodeOrToken::Node(ty)],
        )
      });

    let return_type_list = whitespace()
      .then(just("->"))
      .map(|(ws, _)| {
        GreenNode::new(
          SyntaxKind::Punctuation.into(),
          vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Arrow.into(), "->")),
          ],
        )
      })
      .then(ident())
      .then(return_type_param.repeated().collect::<Vec<_>>())
      .map(|((arrow, first), rest)| {
        let mut children = vec![
          NodeOrToken::Node(arrow),
          NodeOrToken::Node(GreenNode::new(
            SyntaxKind::ConstructorParam.into(),
            vec![
              NodeOrToken::Node(GreenNode::new(SyntaxKind::Punctuation.into(), vec![])),
              NodeOrToken::Node(first),
            ],
          )),
        ];
        for param in rest {
          children.push(NodeOrToken::Node(param));
        }
        GreenNode::new(SyntaxKind::ConstructorParamList.into(), children)
      })
      .or_not()
      .map(|opt| {
        opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::ConstructorParamList.into(), vec![]))
      });

    whitespace()
      .then(ident())
      .then(constructor_param_list)
      .then(return_type_list)
      .map(|(((ws, name), params), return_types)| {
        GreenNode::new(
          SyntaxKind::TypeConstructor.into(),
          vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Node(name),
            NodeOrToken::Node(params),
            NodeOrToken::Node(return_types),
          ],
        )
      })
  };

  let constructor = whitespace()
    .then(keyword("or"))
    .map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::KwOr.into(), "or")),
        ],
      )
    })
    .then(type_constructor.clone())
    .map(|(or_kw, ctor)| {
      GreenNode::new(
        SyntaxKind::Constructor.into(),
        vec![NodeOrToken::Node(or_kw), NodeOrToken::Node(ctor)],
      )
    });

  let constructor_list = type_constructor
    .clone()
    .then(constructor.repeated().collect::<Vec<_>>())
    .map(|(first, rest)| {
      let mut children = vec![NodeOrToken::Node(GreenNode::new(
        SyntaxKind::Constructor.into(),
        vec![
          NodeOrToken::Node(GreenNode::new(SyntaxKind::Punctuation.into(), vec![])),
          NodeOrToken::Node(first),
        ],
      ))];
      for ctor in rest {
        children.push(NodeOrToken::Node(ctor));
      }
      GreenNode::new(SyntaxKind::ConstructorList.into(), children)
    });

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
    .then(ident())
    .then(type_param_list.clone())
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
    .map(|((((type_kw, name), params), eq), struct_def)| {
      GreenNode::new(
        SyntaxKind::TypeDecl.into(),
        vec![
          NodeOrToken::Node(type_kw),
          NodeOrToken::Node(name),
          NodeOrToken::Node(params),
          NodeOrToken::Node(eq),
          NodeOrToken::Node(struct_def),
        ],
      )
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
    .then(ident())
    .then(type_param_list)
    .then(whitespace().then(just(':')).map(|(ws, _)| {
      GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
          NodeOrToken::Token(GreenToken::new(SyntaxKind::Colon.into(), ":")),
        ],
      )
    }))
    .then(constructor_list)
    .map(|((((type_kw, name), params), colon), constructors)| {
      GreenNode::new(
        SyntaxKind::TypeDecl.into(),
        vec![
          NodeOrToken::Node(type_kw),
          NodeOrToken::Node(name),
          NodeOrToken::Node(params),
          NodeOrToken::Node(colon),
          NodeOrToken::Node(constructors),
        ],
      )
    });

  choice((struct_type, union_type))
}

fn behavior_def<'src>(
  expr: impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone + 'src,
) -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  let requirement_inner = whitespace()
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
      vec![
        NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
        NodeOrToken::Node(name),
        NodeOrToken::Node(eq),
        NodeOrToken::Node(ty),
      ]
    });

  let requirement_field = whitespace()
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
    .then(requirement_inner.clone())
    .map(|(comma, inner)| {
      let mut children = vec![NodeOrToken::Node(comma)];
      children.extend(inner);
      GreenNode::new(SyntaxKind::RequirementField.into(), children)
    });

  let requirement_list = requirement_inner
    .then(requirement_field.repeated().collect::<Vec<_>>())
    .map(|(first, rest)| {
      let mut first_children = vec![NodeOrToken::Node(GreenNode::new(
        SyntaxKind::Punctuation.into(),
        vec![],
      ))];
      first_children.extend(first);

      let mut children = vec![NodeOrToken::Node(GreenNode::new(
        SyntaxKind::RequirementField.into(),
        first_children,
      ))];
      for field in rest {
        children.push(NodeOrToken::Node(field));
      }
      GreenNode::new(SyntaxKind::RequirementList.into(), children)
    })
    .or_not()
    .map(|opt| opt.unwrap_or_else(|| GreenNode::new(SyntaxKind::RequirementList.into(), vec![])));

  let method_list = method_def(expr.clone())
    .repeated()
    .collect::<Vec<_>>()
    .map(|methods| {
      GreenNode::new(
        SyntaxKind::MethodList.into(),
        methods
          .into_iter()
          .map(NodeOrToken::Node)
          .collect::<Vec<_>>(),
      )
    });

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
    .then(requirement_list)
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
    .then(method_list)
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
          (((((((concept_kw, name), requires_kw), lbrace1), req_list), rbrace1), with_kw), lbrace2),
          methods,
        ),
        rbrace2,
      )| {
        GreenNode::new(
          SyntaxKind::BehaviorDef.into(),
          vec![
            NodeOrToken::Node(concept_kw),
            NodeOrToken::Node(name),
            NodeOrToken::Node(requires_kw),
            NodeOrToken::Node(lbrace1),
            NodeOrToken::Node(req_list),
            NodeOrToken::Node(rbrace1),
            NodeOrToken::Node(with_kw),
            NodeOrToken::Node(lbrace2),
            NodeOrToken::Node(methods),
            NodeOrToken::Node(rbrace2),
          ],
        )
      },
    )
}

fn pattern<'src>() -> impl Parser<'src, &'src str, Cst, extra::Err<Rich<'src, char>>> + Clone {
  choice((ident(), int(), float(), string(), bool()))
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
    let atom = choice((
      float,
      int,
      string,
      boot,
      match_expr,
      func,
      new_expr,
      ident.clone(),
    ));

    let paren_expr = whitespace
      .clone()
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
      .then(expr.clone())
      .then(whitespace.clone().then(just(')')).map(|(ws, _)| {
        GreenNode::new(
          SyntaxKind::Punctuation.into(),
          vec![
            NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
            NodeOrToken::Token(GreenToken::new(SyntaxKind::RParen.into(), ")")),
          ],
        )
      }))
      .map(|((lparen, inner), rparen)| {
        GreenNode::new(
          SyntaxKind::ParenExpr.into(),
          vec![
            NodeOrToken::Node(lparen),
            NodeOrToken::Node(inner),
            NodeOrToken::Node(rparen),
          ],
        )
      });
    let primary = choice((paren_expr, atom.clone()));
    let call_args = call_args(expr.clone());
    let field_access = field_access();
    let method_call = method_call(expr.clone());

    primary.pratt((
      postfix(
        8,
        whitespace
          .clone()
          .then(keyword("with"))
          .then(ident.clone())
          .map(|((ws, _), behavior)| {
            GreenNode::new(
              SyntaxKind::WithClause.into(),
              vec![
                NodeOrToken::Node(GreenNode::new(
                  SyntaxKind::Punctuation.into(),
                  vec![
                    NodeOrToken::Token(GreenToken::new(SyntaxKind::Whitespace.into(), ws.leak())),
                    NodeOrToken::Token(GreenToken::new(SyntaxKind::KwWith.into(), "with")),
                  ],
                )),
                NodeOrToken::Node(behavior),
              ],
            )
          }),
        |lhs, with_clause, _| {
          GreenNode::new(
            SyntaxKind::WithExpr.into(),
            vec![NodeOrToken::Node(lhs), NodeOrToken::Node(with_clause)],
          )
        },
      ),
      postfix(8, field_access, |lhs, (dot, field), _| {
        GreenNode::new(
          SyntaxKind::FieldAccess.into(),
          vec![
            NodeOrToken::Node(lhs),
            NodeOrToken::Node(dot),
            NodeOrToken::Node(field),
          ],
        )
      }),
      postfix(
        8,
        method_call,
        |lhs, (pipe, method, lparen, args, rparen), _| {
          GreenNode::new(
            SyntaxKind::MethodCall.into(),
            vec![
              NodeOrToken::Node(lhs),
              NodeOrToken::Node(pipe),
              NodeOrToken::Node(method),
              NodeOrToken::Node(lparen),
              NodeOrToken::Node(args),
              NodeOrToken::Node(rparen),
            ],
          )
        },
      ),
      postfix(7, call_args, |lhs, (lparen, args, rparen), _| {
        GreenNode::new(
          SyntaxKind::CallExpr.into(),
          vec![
            NodeOrToken::Node(lhs),
            NodeOrToken::Node(lparen),
            NodeOrToken::Node(args),
            NodeOrToken::Node(rparen),
          ],
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

fn top_level_whitespace<'src>()
-> impl Parser<'src, &'src str, GreenNode, extra::Err<Rich<'src, char>>> + Clone {
  choice((
    any::<&'src str, extra::Err<Rich<'src, char>>>()
      .filter(|c: &char| c.is_whitespace())
      .map(|c| c.to_string()),
    just('#')
      .then(
        any()
          .filter(|c: &char| *c != '\n')
          .repeated()
          .collect::<String>(),
      )
      .then(just('\n').or_not())
      .map(
        |((_hash, comment), newline): ((char, String), Option<char>)| {
          format!("#{}{}", comment, newline.unwrap_or('\n'))
        },
      ),
  ))
  .repeated()
  .at_least(1)
  .collect::<Vec<String>>()
  .map(|parts| {
    let ws = parts.join("");
    GreenNode::new(
      SyntaxKind::Whitespace.into(),
      vec![NodeOrToken::Token(GreenToken::new(
        SyntaxKind::Whitespace.into(),
        ws.leak(),
      ))],
    )
  })
}

pub fn top_level_expr<'src>()
-> impl Parser<'src, &'src str, SyntaxNode, extra::Err<Rich<'src, char>>> {
  choice((
    type_decl(),
    behavior_def(expr()),
    let_in(expr()),
    ref_in(expr()),
    top_level_whitespace(),
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
  top_level_expr().then_ignore(end())
}
