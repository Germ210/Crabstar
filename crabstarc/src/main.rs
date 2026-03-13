mod ir_lowering;

use crabstar_backend::abi::types::AbiType;
use crabstar_frontend::{
  ast::{AstNode, Root},
  parser::{parser, Parser},
  typechecker::TypeChecker,
  types::Type,
};
use ir_lowering::Compiler;

fn main() {
  let source = "let myfunc: fn(a: int32, b: int32) -> int32: a + b";
  let (ast, err) = parser().parse(source).into_output_errors();

  if let Some(root_node) = ast {
    let root = Root::cast(root_node).unwrap();
    let mut checker = TypeChecker::new();

    for child in root.let_exprs() {
      let ty = checker.check(child.syntax());
      println!("Type: {:?}", ty);

      let expr = child.expr();
      if let Some(expr_node) = expr.as_node() {
        println!("Expr node kind: {:?}", expr_node.kind());
        if let Some(ast_node) = AstNode::cast(expr_node.clone()) {
          println!("AstNode: {:?}", std::mem::discriminant(&ast_node));
          if let AstNode::FnExpr(_) = ast_node {
            println!("Found FnExpr!");
            if let Type::Fn { params, .. } = ty {
              println!("Compiling with {} params", params.len());
              let param_types: Vec<AbiType> = params.iter().map(Compiler::type_to_abi).collect();
              let mut compiler = Compiler::new();
              let cfg = compiler.compile_function(expr_node, &param_types);
              println!("\nCompiled IR:");
              println!("{:#?}", cfg);
            }
          }
        }
      }
    }
  } else {
    dbg!(err);
  }
}
