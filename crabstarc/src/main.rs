mod ir_lowering;
use crabstar_backend::{codegen::generate_code, regalloc::x86_64::Win64};
use crabstar_frontend::{
  ast::{AstNode, BehaviorDef, Ident, LetExpr, Root},
  parser::{parser, Parser},
  typechecker::TypeChecker,
  types::Type,
};
use ir_lowering::Compiler;
use std::env;
use std::fs;
use std::process;

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() < 2 {
    eprintln!("Usage: {} <input.crab> [output.o]", args[0]);
    eprintln!("  If output is not specified, defaults to 'output.o'");
    process::exit(1);
  }
  let input_path = &args[1];
  let output_path = if args.len() >= 3 {
    &args[2]
  } else {
    "output.o"
  };
  let source = match fs::read_to_string(input_path) {
    Ok(content) => content,
    Err(e) => {
      eprintln!("Error reading file '{}': {}", input_path, e);
      process::exit(1);
    }
  };

  let (ast, err) = parser().parse(&source).into_output_errors();

  if let Some(root_node) = ast {
    println!("=== Full AST ===");
    println!("{:#?}", root_node);

    let root = Root::cast(root_node).unwrap();
    let mut checker = TypeChecker::new();

    for child in root.type_decls() {
      checker.check_type_decl(&child);
    }
    checker.resolve_all_types();

    for child in root.children() {
      if let Some(behavior) = BehaviorDef::cast(child) {
        checker.check_behavior_def(&behavior);
      }
    }

    println!("\n=== Declared Types ===");
    for (name, ty) in &checker.declared_types {
      println!("{}: {:?}", name, ty);
    }

    println!("\n=== Behaviors ===");
    for (name, behavior) in &checker.behaviors {
      println!("{}: {:?}", name, behavior);
    }

    println!("\n=== Type Checking All Expressions ===");
    for child in root.children() {
      if let Some(ast_node) = AstNode::cast(child.clone()) {
        println!("\nNode: {:?}", std::mem::discriminant(&ast_node));
      }
    }

    let mut typed_functions = Vec::new();

    for child in root.children() {
      if let Some(let_expr) = LetExpr::cast(child) {
        let expr_node = let_expr.expr().into_node().unwrap();
        let expr_ty = checker.check(&expr_node);

        println!("Type: {:#?}", expr_ty);

        if let Type::Fn { .. } = expr_ty {
          let func_name = Ident::cast(let_expr.name().into_node().unwrap()).unwrap();
          let func_name = func_name.name();
          let func_name = func_name.as_token().unwrap().text();

          typed_functions.push((func_name.to_string(), let_expr, expr_ty));
        }
      }
    }

    let mut compiler = Compiler::new();
    let mut lowered_functions = Vec::new();

    for (func_name, let_expr, fn_ty) in typed_functions {
      if let Some((cfg, cif)) = compiler.build_with_ffi(&let_expr, &fn_ty) {
        lowered_functions.push((func_name, cfg, cif));
      }
    }

    if !lowered_functions.is_empty() {
      let mut all_obj_bytes = Vec::new();

      for (func_name, cfg, cif) in lowered_functions {
        let obj_bytes = generate_code::<Win64>(&cfg, &cif, &func_name);
        all_obj_bytes.extend(obj_bytes);
      }

      match fs::write(output_path, &all_obj_bytes) {
        Ok(_) => println!("\nSuccessfully wrote object file to '{}'", output_path),
        Err(e) => {
          eprintln!("Error writing output file '{}': {}", output_path, e);
          process::exit(1);
        }
      }
    }
  } else {
    eprintln!("Parse errors:");
    for e in err {
      eprintln!("  {}", e);
    }
    process::exit(1);
  }
}
