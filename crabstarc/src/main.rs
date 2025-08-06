use crabstar_parser::{parser::parser, Parser};

fn main() {
  let input = "
    let x: 12 let b => false
  ";

  let parser = parser();
  let (ast, errs) = parser.parse(input).into_output_errors();

  println!("Ast: {:#?}", ast);
  println!("Errors: {:#?}", errs);
}
