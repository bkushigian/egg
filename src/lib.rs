#![warn(missing_docs)]
/*!
[`EGraph`]s (and almost everything else in this crate) are
parameterized over the language given by the user (by implementing
the [`Language`] trait).

If your Language implements [`FromStr`] (and Languages derived using
[`define_language!`] do), you can easily create [`RecExpr`]s to add to
an [`EGraph`].

[`EGraph`]: struct.EGraph.html
[`Language`]: trait.Language.html
[`RecExpr`]: struct.RecExpr.html
[`define_language!`]: macro.define_language.html
[`FromStr`]: https://doc.rust-lang.org/std/str/trait.FromStr.html

Add `egg` to your `Cargo.toml` like this:
```toml
[dependencies]
egg = "0.5.0"
```

# Example

```
use egg::{*, rewrite as rw};

define_language! {
    enum SimpleLanguage {
        Num(i32),
        "+" = Add([Id; 2]),
        "*" = Mul([Id; 2]),
        // language items are parsed in order, and we want symbol to
        // be a fallback, so we put it last.
        // `Symbol` is an egg-provided interned string type
        Symbol(egg::Symbol),
    }
}

let rules: &[Rewrite<SimpleLanguage, ()>] = &[
    rw!("commute-add"; "(+ ?a ?b)" => "(+ ?b ?a)"),
    rw!("commute-mul"; "(* ?a ?b)" => "(* ?b ?a)"),

    rw!("add-0"; "(+ ?a 0)" => "?a"),
    rw!("mul-0"; "(* ?a 0)" => "0"),
    rw!("mul-1"; "(* ?a 1)" => "?a"),
];

let start = "(+ 0 (* 1 foo))".parse().unwrap();
let runner = Runner::default().with_expr(&start).run(rules);
println!(
    "Stopped after {} iterations, reason: {:?}",
    runner.iterations.len(),
    runner.stop_reason
);
```
!*/

mod macros;

mod dot;
mod eclass;
mod egraph;
mod extract;
mod language;
mod machine;
mod pattern;
mod rewrite;
mod run;
mod subst;
mod unionfind;
mod util;

/// A key to identify [`EClass`](struct.EClass.html)es within an
/// [`EGraph`](struct.EGraph.html).
pub type Id = u32;

pub(crate) use unionfind::UnionFind;

pub use {
    dot::Dot,
    eclass::EClass,
    egraph::EGraph,
    extract::*,
    language::*,
    pattern::{ENodeOrVar, Pattern, PatternAst, SearchMatches},
    rewrite::{Applier, Condition, ConditionEqual, ConditionalApplier, Rewrite, Searcher},
    run::*,
    subst::{Subst, Var},
    util::*,
};

#[cfg(test)]
fn init_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[doc(hidden)]
pub mod test;
