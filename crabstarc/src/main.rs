use crabstar_frontend::ast::Root;
use crabstar_frontend::parser::{parser, Parser};
use crabstar_frontend::typechecker::TypeChecker;

fn main() {
  let test_script = r#"
    let fib: fn(n): match n {
      of 0: 0
      of 1: 1
    } else: fib(n - 1) + fib(n - 2)
    in fib(10)

    let compose: fn(f, g, x): f(g(x)) in
    compose(
      fn(y): y + 1,
      fn(z): z * 2,
      10
    )
  "#;

  let (ast, err) = parser().parse(test_script).into_output_errors();

  if let Some(root_node) = ast {
    dbg!(&root_node);

    let root = Root::cast(root_node).unwrap();
    let mut checker = TypeChecker::new();

    for child in root.let_exprs() {
      dbg!(&child);
      let ty = checker.check(child.syntax());
      println!("Type: {:?}", ty);
    }
  } else {
    dbg!(err);
  }
}
