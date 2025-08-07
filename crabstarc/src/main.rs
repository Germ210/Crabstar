use crabstar_parser::{parser::parser, Parser};

fn main() {
  let input = "
    let x: if true: 12 elif false: 66 else: 16
  ";

  let parser = parser();
  let (ast, errs) = parser.parse(input).into_output_errors();

  println!("Ast: {:#?}", ast);
  println!("Errors: {:#?}", errs);
}
