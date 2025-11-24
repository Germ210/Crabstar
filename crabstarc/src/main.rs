use crabstar_frontend::parser::{parser, Parser};

fn main() {
  let src =
    "let add: fn(x, y): x + y in let multiply: fn(a, b): a * b in multiply(add(1, 2), add(3, 4)) let b: 12";
  println!("{:#?}", parser().parse(src).into_output_errors());
}
