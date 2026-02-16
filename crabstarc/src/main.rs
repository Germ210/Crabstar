use crabstar_frontend::ast::Root;
use crabstar_frontend::parser::{parser, Parser};
use crabstar_frontend::types::{Type, TypeTable};

fn main() {
  let test_script = r#"
# This is a comment
let x -> int32: f(a, b) in x + 10  # inline comment
# another comment
"#;

  let (ast, err) = parser().parse(test_script).into_output_errors();

  if let Some(root_node) = ast {
    dbg!(&root_node);

    let root = Root::cast(root_node).unwrap();
    let mut type_table = TypeTable::new();

    for let_expr in root.let_exprs() {
      let type_expr_node = let_expr.type_expr();
      if let Some(te_node) = type_expr_node.as_node() {
        if let Some(type_expr) = crabstar_frontend::ast::TypeExpr::cast(te_node.clone()) {
          let ty = Type::from_type_expr(&type_expr);
          type_table.insert(let_expr.syntax().clone(), ty.clone());
          println!("Let expression type: {:?}", ty);
        }
      }
    }

    dbg!(&type_table);
  } else {
    dbg!(err);
  }
}
