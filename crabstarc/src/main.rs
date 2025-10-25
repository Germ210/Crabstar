use crabstar_frontend::parser::{number, parse};

fn main() {
  println!("{:#?}", parse(number, "6.7"));
}
