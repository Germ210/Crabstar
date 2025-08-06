mod ast;
mod parser;

use chumsky::Parser;
use parser::*;


fn main() {
  let input = "let x :: (a, @, c): a || c";

  let parser = parser();
  let (ast, errs) = parser.parse(input).into_output_errors();

  println!("Ast: {:#?}", ast);
  println!("Errors: {:#?}", errs);
}
