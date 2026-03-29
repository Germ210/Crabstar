#[macro_export]
macro_rules! ematch {
  ($expr:expr, $epath:expr, $($pat:tt)*) => {
    $crate::ematch_inner!($expr, $epath, $($pat)*)
  };
}

#[macro_export]
macro_rules! ematch_inner {
  ($expr:expr, $epath:expr, $variant:ident($($args:tt)*) if $guard:expr => $body:block $($rest:tt)*) => {
    $crate::ematch_arm!($expr, $epath, $variant($($args)*), if $guard, $body);
    $crate::ematch_inner!($expr, $epath, $($rest)*)
  };
  ($expr:expr, $epath:expr, $variant:ident($($args:tt)*) => $body:block $($rest:tt)*) => {
    $crate::ematch_arm!($expr, $epath, $variant($($args)*), , $body);
    $crate::ematch_inner!($expr, $epath, $($rest)*)
  };
  ($expr:expr, $epath:expr, ) => {};
  ($expr:expr, $epath:expr) => {};
}

#[macro_export]
macro_rules! ematch_arm {
  ($expr:expr, $epath:expr, $variant:ident($first:tt, $Nested:ident($($inner:tt)*), $($rest_args:tt)*), if $guard:expr, $body:block) => {
    #[allow(unused_imports)]
    use $crate::epath::ir::Expr::*;
    match &*$expr {
      $crate::epath::ir::Expr::$variant($first, __inner, $($rest_args)*) => {
        match &*__inner.as_expr() {
          $crate::epath::ir::Expr::$Nested($($inner)*) if $guard => $body
          _ => {}
        }
      }
      _ => {}
    }
  };
  ($expr:expr, $epath:expr, $variant:ident($first:tt, $Nested:ident($($inner:tt)*), $($rest_args:tt)*), , $body:block) => {
    #[allow(unused_imports)]
    use $crate::epath::ir::Expr::*;
    match &*$expr {
      $crate::epath::ir::Expr::$variant($first, __inner, $($rest_args)*) => {
        match &*__inner.as_expr() {
          $crate::epath::ir::Expr::$Nested($($inner)*) => $body
          _ => {}
        }
      }
      _ => {}
    }
  };
  ($expr:expr, $epath:expr, $variant:ident($($args:tt)*), if $guard:expr, $body:block) => {
    #[allow(unused_imports)]
    use $crate::epath::ir::Expr::*;
    match &*$expr {
      $crate::epath::ir::Expr::$variant($($args)*) if $guard => $body
      _ => {}
    }
  };
  ($expr:expr, $epath:expr, $variant:ident($($args:tt)*), , $body:block) => {
    #[allow(unused_imports)]
    use $crate::epath::ir::Expr::*;
    match &*$expr {
      $crate::epath::ir::Expr::$variant($($args)*) => $body
      _ => {}
    }
  };
}
