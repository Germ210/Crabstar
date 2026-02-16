use crabstar_frontend::ast::Root;
use crabstar_frontend::parser::{parser, Parser};
use crabstar_frontend::typechecker::TypeChecker;

fn main() {
  let test_script = r#"
    let x: fn(): fn(a, b): a + b in x()(1, 2)
    let y: fn(): fn(a, b): a + b in x()("Hello, ", "World!")
  "#;
  let (ast, err) = parser().parse(test_script).into_output_errors();
  if let Some(root_node) = ast {
    dbg!(&root_node);

    let root = Root::cast(root_node).unwrap();
    let mut checker = TypeChecker::new();

    for child in root.let_exprs() {
      let ty = checker.check(child.syntax());
      println!("Type: {:?}", ty);
    }
  } else {
    dbg!(err);
  }
}
