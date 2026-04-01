use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, Pat, Result, Token, braced, parse::Parse, parse::ParseStream, parse_macro_input};

struct EmatchInput {
  expr: Expr,
  _comma1: Token![,],
  _epath: Expr,
  _comma2: Token![,],
  arms: syn::punctuated::Punctuated<EmatchArm, Token![,]>,
}

struct EmatchArm {
  pat: syn::Pat,
  guard: Option<(Token![if], Box<Expr>)>,
  body: syn::Block,
}

impl Parse for EmatchArm {
  fn parse(input: ParseStream) -> Result<Self> {
    let pat = syn::Pat::parse_single(input)?;
    let guard = if input.peek(Token![if]) {
      let if_token: Token![if] = input.parse()?;
      let expr: Expr = input.parse()?;
      Some((if_token, Box::new(expr)))
    } else {
      None
    };
    let _arrow: Token![=>] = input.parse()?;
    let body: syn::Block = input.parse()?;
    Ok(EmatchArm { pat, guard, body })
  }
}

impl Parse for EmatchInput {
  fn parse(input: ParseStream) -> Result<Self> {
    let expr: Expr = input.parse()?;
    let _comma1: Token![,] = input.parse()?;
    let _epath: Expr = input.parse()?;
    let _comma2: Token![,] = input.parse()?;
    let content;
    braced!(content in input);
    let arms = content.parse_terminated(EmatchArm::parse, Token![,])?;
    Ok(EmatchInput {
      expr,
      _comma1,
      _epath,
      _comma2,
      arms,
    })
  }
}

fn extract_nested_patterns(pat: &Pat) -> (Pat, Vec<(String, Pat)>) {
  match pat {
    Pat::TupleStruct(tuple_pat) => {
      let mut replacements = Vec::new();
      let mut new_elems = syn::punctuated::Punctuated::new();

      for (i, elem) in tuple_pat.elems.iter().enumerate() {
        match elem {
          Pat::TupleStruct(_) => {
            let var_name = format!("__nested_{}", i);
            let var_ident = syn::Ident::new(&var_name, proc_macro2::Span::call_site());
            replacements.push((var_name, elem.clone()));
            new_elems.push(
              syn::PatIdent {
                attrs: vec![],
                by_ref: None,
                mutability: None,
                ident: var_ident,
                subpat: None,
              }
              .into(),
            );
          }
          _ => {
            new_elems.push(elem.clone());
          }
        }
      }

      let new_pat = Pat::TupleStruct(syn::PatTupleStruct {
        attrs: tuple_pat.attrs.clone(),
        path: tuple_pat.path.clone(),
        paren_token: tuple_pat.paren_token,
        elems: new_elems,
        qself: None,
      });

      (new_pat, replacements)
    }
    _ => (pat.clone(), vec![]),
  }
}

#[proc_macro]
pub fn ematch(input: TokenStream) -> TokenStream {
  let EmatchInput { expr, arms, .. } = parse_macro_input!(input as EmatchInput);

  let mut arm_checks = Vec::new();

  for arm in arms.iter() {
    let pat = &arm.pat;
    let body = &arm.body;
    let (flat_pat, nested) = extract_nested_patterns(pat);

    let body_with_nested = if !nested.is_empty() {
      let mut nested_ifs = quote! { #body };

      for (var_name, nested_pat) in nested.iter().rev() {
        let var_ident = syn::Ident::new(var_name, proc_macro2::Span::call_site());
        nested_ifs = quote! {
          if let #nested_pat = #var_ident.as_expr() {
            #nested_ifs
          }
        };
      }
      nested_ifs
    } else {
      quote! { #body }
    };

    if let Some((_, guard_expr)) = &arm.guard {
      arm_checks.push(quote! {
        if let #flat_pat = #expr {
          if #guard_expr {
            #body_with_nested
          }
        }
      });
    } else {
      arm_checks.push(quote! {
        if let #flat_pat = #expr {
          #body_with_nested
        }
      });
    }
  }

  let expanded = quote! {
    {
      use crate::epath::ir::Expr::*;
      use crate::epath::ir::{ExprId, EPath};
      use crate::ir::graph::IntSize;
      #(#arm_checks)*
    }
  };

  TokenStream::from(expanded)
}
