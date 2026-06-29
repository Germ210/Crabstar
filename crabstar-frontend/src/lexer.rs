use crate::{err::Span, syntax::SyntaxKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
  pub kind: SyntaxKind,
  pub text: String,
  pub span: Span,
}

pub struct Lexer<'src> {
  src: &'src str,
  pos: usize,
}

impl<'src> Lexer<'src> {
  pub fn new(src: &'src str) -> Self {
    Self { src, pos: 0 }
  }

  fn peek(&self) -> Option<char> {
    self.src[self.pos..].chars().next()
  }

  fn peek2(&self) -> Option<char> {
    let mut chars = self.src[self.pos..].chars();
    chars.next();
    chars.next()
  }

  fn advance(&mut self) -> char {
    let c = self.peek().unwrap();
    self.pos += c.len_utf8();
    c
  }

  fn eat_while(&mut self, mut pred: impl FnMut(char) -> bool) {
    while self.peek().is_some_and(|c| pred(c)) {
      self.advance();
    }
  }

  fn make_token(&self, kind: SyntaxKind, start: usize) -> Token {
    Token {
      kind,
      text: self.src[start..self.pos].to_string(),
      span: start..self.pos,
    }
  }

  pub fn tokenize(mut self) -> Vec<Token> {
    let mut tokens = Vec::new();

    while let Some(c) = self.peek() {
      let start = self.pos;

      let kind = match c {
        c if c.is_whitespace() => {
          self.eat_while(|c| c.is_whitespace());
          SyntaxKind::Whitespace
        }

        '#' => {
          self.eat_while(|c| c != '\n');
          SyntaxKind::Comment
        }

        '"' => {
          self.advance();
          self.eat_while(|c| c != '"');
          if self.peek().is_some() {
            self.advance();
          }
          SyntaxKind::String
        }

        '0'..='9' => {
          self.eat_while(|c| c.is_ascii_digit());
          if self.peek() == Some('.') && self.peek2().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
            self.eat_while(|c| c.is_ascii_digit());
            SyntaxKind::Float
          } else {
            SyntaxKind::Int
          }
        }

        'a'..='z' | 'A'..='Z' | '_' => {
          self.eat_while(|c| c.is_alphanumeric() || c == '_');
          let text = &self.src[start..self.pos];
          match text {
            "let" => SyntaxKind::KwLet,
            "ref" => SyntaxKind::KwRef,
            "mut" => SyntaxKind::KwMut,
            "owned" => SyntaxKind::KwOwned,
            "in" => SyntaxKind::KwIn,
            "do" => SyntaxKind::KwDo,
            "then" => SyntaxKind::KwThen,
            "fn" => SyntaxKind::KwFn,
            "def" => SyntaxKind::KwDef,
            "match" => SyntaxKind::KwMatch,
            "of" => SyntaxKind::KwOf,
            "when" => SyntaxKind::KwWhen,
            "new" => SyntaxKind::KwNew,
            "struct" => SyntaxKind::KwStruct,
            "type" => SyntaxKind::KwType,
            "with" => SyntaxKind::KwWith,
            "uses" => SyntaxKind::KwUses,
            "cause" => SyntaxKind::KwCause,
            "effect" => SyntaxKind::KwEffect,
            "concept" => SyntaxKind::KwConcept,
            "requires" => SyntaxKind::KwRequires,
            "import" => SyntaxKind::KwImport,
            "export" => SyntaxKind::KwExport,
            "else" => SyntaxKind::KwElse,
            "null" => SyntaxKind::KwNull,
            "true" => SyntaxKind::KwTrue,
            "false" => SyntaxKind::KwFalse,
            "not" => SyntaxKind::KwNot,
            "and" => SyntaxKind::KwAnd,
            "or" => SyntaxKind::KwOr,
            _ => SyntaxKind::Symbol,
          }
        }

        '-' if self.peek2() == Some('>') => {
          self.advance();
          self.advance();
          SyntaxKind::Arrow
        }
        '<' if self.peek2() == Some('-') => {
          self.advance();
          self.advance();
          SyntaxKind::LeftArrow
        }
        '|' if self.peek2() == Some('>') => {
          self.advance();
          self.advance();
          SyntaxKind::Pipe
        }
        '!' if self.peek2() == Some('=') => {
          self.advance();
          self.advance();
          SyntaxKind::NotEq
        }
        '<' if self.peek2() == Some('=') => {
          self.advance();
          self.advance();
          SyntaxKind::LtEq
        }
        '>' if self.peek2() == Some('=') => {
          self.advance();
          self.advance();
          SyntaxKind::GtEq
        }

        '(' => {
          self.advance();
          SyntaxKind::LParen
        }
        ')' => {
          self.advance();
          SyntaxKind::RParen
        }
        '{' => {
          self.advance();
          SyntaxKind::LBrace
        }
        '}' => {
          self.advance();
          SyntaxKind::RBrace
        }
        '[' => {
          self.advance();
          SyntaxKind::LBracket
        }
        ']' => {
          self.advance();
          SyntaxKind::RBracket
        }
        ':' => {
          self.advance();
          SyntaxKind::Colon
        }
        ',' => {
          self.advance();
          SyntaxKind::Comma
        }
        '.' => {
          self.advance();
          SyntaxKind::Dot
        }
        '=' => {
          self.advance();
          SyntaxKind::Eq
        }
        '+' => {
          self.advance();
          SyntaxKind::Plus
        }
        '-' => {
          self.advance();
          SyntaxKind::Minus
        }
        '*' => {
          self.advance();
          SyntaxKind::Star
        }
        '/' => {
          self.advance();
          SyntaxKind::Slash
        }
        '<' => {
          self.advance();
          SyntaxKind::Lt
        }
        '>' => {
          self.advance();
          SyntaxKind::Gt
        }
        '\\' => {
          self.advance();
          SyntaxKind::Backslash
        }

        _ => {
          self.advance();
          SyntaxKind::Error
        }
      };

      tokens.push(self.make_token(kind, start));
    }

    tokens.push(Token {
      kind: SyntaxKind::Eof,
      text: String::new(),
      span: self.pos..self.pos,
    });

    tokens
  }
}
