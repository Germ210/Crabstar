use std::collections::HashMap;
use generational_arena::Arena;
use crabstar_frontend::{ast::{Ast, AstKind, TypedAst}, typechecker::Type};
use crate::ir::*;

pub(crate) struct IrGenerator {
  instrs: Arena<Instr>,
  functions: HashMap<String, Function>,
  globals: HashMap<String, Global>,
  current_function_params: HashMap<String, Type>,
  locals_stack: Vec<Temp>, 
  locals_names: Vec<String>,
  in_function: bool,
}

impl IrGenerator {
  pub fn new() -> Self {
    Self {
      instrs: Arena::new(),
      functions: HashMap::new(),
      globals: HashMap::new(),
      current_function_params: HashMap::new(),
      locals_stack: Vec::new(),
      locals_names: Vec::new(),
      in_function: false,
    }
  }

  pub fn generate(mut self, asts: Vec<Ast>) -> IrModule {
    for ast in asts {
      self.translate_ast(ast);
    }

    IrModule {
      functions: self.functions,
      globals: self.globals,
      instrs: self.instrs,
    }
  }

  fn translate_ast(&mut self, ast: Ast) -> Option<Temp> {
    let (_, TypedAst { ty, node }) = ast;

    match node {
      AstKind::Int(num) => {
        Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Int(num)))))
      }

      AstKind::Float(num) => {
        Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Float(num)))))
      }

      AstKind::Bool(b) => {
        Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Bool(b)))))
      }

      AstKind::Ident(name) => {
        if let Some(idx) = self.locals_names.iter().position(|n| *n == name) {
          let temp = self.locals_stack[idx];
          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Get(temp)))))
        }
        else if self.current_function_params.contains_key(&name) {
          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::GetParam(name)))))
        }
        else {
          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::GetGlobal(name)))))
        }
      }

      AstKind::Unary(op, operand) => {
        if let Some(operand_temp) = self.translate_ast(*operand) {
          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Call {
            callee: op,
            args: vec![operand_temp],
          }))))
        } else {
          None
        }
      }

      AstKind::Binary(op, left, right) => {
        if let (Some(left_temp), Some(right_temp)) =
          (self.translate_ast(*left), self.translate_ast(*right))
        {
          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Call {
            callee: op,
            args: vec![left_temp, right_temp],
          }))))
        } else {
          None
        }
      }

      AstKind::Block(exprs) => {
        let mut temps = Vec::new();
        for expr in exprs {
          if let Some(temp) = self.translate_ast(expr) {
            temps.push(temp);
          }
        }

        if temps.is_empty() {
          None
        } else {
          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Block(temps)))))
        }
      }

      AstKind::Let { name, args, value, next } => {
        if let Some(params) = args {
          let old_params = self.current_function_params.clone();
          let old_locals_stack = std::mem::take(&mut self.locals_stack);
          let old_locals_names = std::mem::take(&mut self.locals_names);
          let old_in_function = self.in_function;

          self.in_function = true;

          let mut fn_params = Vec::new();
          for param in &params {
            if let (_, TypedAst { ty: param_ty, node: AstKind::Ident(param_name) }) = param {
              fn_params.push((param_name.clone(), param_ty.clone()));
              self.current_function_params.insert(param_name.clone(), param_ty.clone());
            }
          }

          if let Some(body_temp) = self.translate_ast(*value) {
            let function = Function {
              params: fn_params,
              return_type: ty.clone(),
              body: vec![body_temp],
            };

            self.functions.insert(name.clone(), function);
          }

          self.current_function_params = old_params;
          self.locals_stack = old_locals_stack;
          self.locals_names = old_locals_names;
          self.in_function = old_in_function;

          if let Some(next_expr) = next {
            self.translate_ast(*next_expr)
          } else {
            None
          }
        } else {
          if let Some(value_temp) = self.translate_ast(*value) {
            if self.in_function {
              self.locals_names.push(name);
              self.locals_stack.push(value_temp);
            } else {
              self.globals.insert(name, Global {
                ty: ty.clone(),
                value: value_temp,
              });
            }

            if let Some(next_expr) = next {
              self.translate_ast(*next_expr)
            } else {
              None
            }
          } else {
            None
          }
        }
      }

      AstKind::Call { callee, args } => {
        if let (_, TypedAst { node: AstKind::Ident(fn_name), .. }) = &*callee {
          let mut arg_temps = Vec::new();
          for arg in args {
            if let Some(temp) = self.translate_ast(arg) {
              arg_temps.push(temp);
            } else {
              return None;
            }
          }

          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Call {
            callee: fn_name.clone(),
            args: arg_temps,
          }))))
        } else {
          None
        }
      }

      AstKind::If { cond, then_expr, else_expr } => {
        if let (Some(cond_temp), Some(then_temp)) =
          (self.translate_ast(*cond), self.translate_ast(*then_expr))
        {
          let mut choices = Vec::new();

          let then_closure_temp = self.instrs.insert(Instr::new(
              ty.clone(),
              Box::new(Expr::Closure {
                instrs: vec![Expr::Block(vec![then_temp])],
              }),
          ));
          choices.push(then_closure_temp);

          if let Some(else_ast) = else_expr {
            if let Some(else_temp) = self.translate_ast(*else_ast) {
              let else_closure_temp = self.instrs.insert(Instr::new(
                  ty.clone(),
                  Box::new(Expr::Closure {
                    instrs: vec![Expr::Block(vec![else_temp])],
                  }),
              ));
              choices.push(else_closure_temp);
            } else {
              return None;
            }
          }

          Some(self.instrs.insert(Instr::new(ty, Box::new(Expr::Select {
            index: cond_temp,
            choices,
          }))))
        } else {
          None
        }
      }

      AstKind::HeapAlloc { class: _, expr } => {
        if let Some(expr_temp) = self.translate_ast(*expr) {
          Some(self.instrs.insert(Instr::new(Type::Heap(Box::new(ty)), Box::new(Expr::HeapAlloc(expr_temp)))))
        } else {
          None
        }
      }

      _ => None,
    }
  }
}

pub fn generate_ir(asts: Vec<Ast>) -> IrModule {
  let generator = IrGenerator::new();
  generator.generate(asts)
}

