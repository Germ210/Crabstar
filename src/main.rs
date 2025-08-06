mod ast;
mod parser;

use chumsky::Parser;
use parser::*;


fn main() {
  let input = "let x :: (a, b, c): a + b / c,
    let b => x + 12";

  let parser = parser();
  let (ast, errs) = parser.parse(input).into_output_errors();

  println!("Ast: {:#?}", ast);
  println!("Errors: {:#?}", errs);
}
