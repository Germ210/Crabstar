use crabstar_frontend::{parser::parser, Parser};

fn main() {
  let input = "
    const fib :: (a, b): a + -b

    let main => fib(0, 1)
  ";
  
  let parser = parser();
  let (ast, errs) = parser.parse(input).into_output_errors();
  
  println!("AST: {:#?}\n", ast);
  println!("Parse Errors: {:#?}\n", errs);
  
}
