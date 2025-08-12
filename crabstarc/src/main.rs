use crabstar_frontend::{parser::parser, Parser};
use crabstar_opt::{memory_managment::LifetimeAnalyzer, translate_ast::generate_ir};

fn main() {
  let input = "let x => new I32(14)";

  let parser = parser();
  let (ast, errs) = parser.parse(input).into_output_errors();
  println!("Ast: {:#?}\n", ast);
  println!("Errors: {:#?}\n", errs);
  if errs.is_empty() {
    let mut ir_mod = generate_ir(ast.unwrap());
    println!("Ir: {:#?}\n", ir_mod);
    ir_mod.run_pass(LifetimeAnalyzer::new());
  } 

}
