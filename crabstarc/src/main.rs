use crabstar_frontend::{parser::parser, Parser};

fn main() {
  let input = "let x: [1, 2]";

  let parser = parser();
  let (ast, errs) = parser.parse(input).into_output_errors();
  println!("Ast: {:#?}\n", ast);
  println!("Errors: {:#?}\n", errs); 
}
