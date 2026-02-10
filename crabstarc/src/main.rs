use crabstar_frontend::{
  ast::Root,
  parser::{parser, Parser},
};

fn main() {
  let test_script =
    r#"concept Adder requires {x = int} with {def add(self: int, n: int) -> int: self.x + n}"#;
  let (ast, err) = parser().parse(test_script).into_output_errors();

  if !err.is_empty() {
    println!("Errors:");
    for e in err {
      println!("{:?}", e);
    }
  }

  if let Some(ast) = ast {
    let root = Root::cast(ast).unwrap();
    println!("AST:");
    for (i, node) in root.children().enumerate() {
      println!("\nNode {}:", i);
      println!("{:#?}", node);
    }
  }
}
