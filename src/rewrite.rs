use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use log::*;

use crate::{
    AddResult, EGraph, Id, Language, Metadata, ParseError, Pattern, SearchMatches, WildMap,
};

pub struct RewriteBuilder<L, M> {
    name: String,
    patterns: Vec<Pattern<L>>,
    appliers: Vec<Rc<dyn Applier<L, M>>>,
    conditions: Vec<Condition<L>>,
    application_limit: usize,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Rewrite<L, M> {
    pub name: String,
    pub patterns: Vec<Pattern<L>>,
    pub appliers: Vec<Rc<dyn Applier<L, M>>>,
    pub conditions: Vec<Condition<L>>,
    pub application_limit: usize,
}

/// Shorthand for `RewriteBuilder::new`.
pub fn rw<L, M>(name: impl Into<String>) -> RewriteBuilder<L, M> {
    RewriteBuilder::new(name)
}

impl<L, M> RewriteBuilder<L, M> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            patterns: vec![],
            appliers: vec![],
            conditions: vec![],
            application_limit: 10_000,
        }
    }
}

impl<L, M> RewriteBuilder<L, M>
where
    L: Language + FromStr,
    M: Metadata<L>,
{
    pub fn with_pattern_str(self, pattern_str: &str) -> Result<Self, ParseError> {
        pattern_str
            .parse()
            .map(|p: Pattern<L>| self.with_pattern(p))
    }
    pub fn with_applier_str(self, applier_str: &str) -> Result<Self, ParseError> {
        applier_str
            .parse()
            .map(|a: Pattern<L>| self.with_applier(a))
    }
    pub fn p(self, pattern_str: &str) -> Self {
        self.with_pattern_str(pattern_str).unwrap()
    }
    pub fn a(self, applier_str: &str) -> Self {
        self.with_applier_str(applier_str).unwrap()
    }
}

impl<L, M> RewriteBuilder<L, M>
where
    L: Language,
    M: Metadata<L>,
{
    pub fn with_pattern(mut self, pattern: Pattern<L>) -> Self {
        self.patterns.push(pattern);
        self
    }
    pub fn with_applier(mut self, applier: impl Applier<L, M> + 'static) -> Self {
        self.appliers.push(Rc::new(applier));
        self
    }
    pub fn with_condition(mut self, condition: Condition<L>) -> Self {
        self.conditions.push(condition);
        self
    }
    /// Default is 10_000.
    pub fn with_application_limit(mut self, application_limit: usize) -> Self {
        self.application_limit = application_limit;
        self
    }

    pub fn build(self) -> Result<Rewrite<L, M>, ()> {
        assert_ne!(self.patterns.len(), 0);
        assert_ne!(self.appliers.len(), 0);
        // TODO check binding here
        Ok(Rewrite {
            name: self.name,
            patterns: self.patterns,
            appliers: self.appliers,
            conditions: self.conditions,
            application_limit: self.application_limit,
        })
    }

    // TODO is this needed? We could check binding on calls to the
    // builder methods
    /// Shorthand for `.build().unwrap()`.
    pub fn mk(self) -> Rewrite<L, M> {
        self.build().unwrap()
    }
}

impl<L: Language, M: Metadata<L>> Rewrite<L, M> {
    /// This `run` is for testing use only. You should use things
    /// from the `egg::run` module
    #[cfg(test)]
    pub(crate) fn run(&self, egraph: &mut EGraph<L, M>) -> Vec<Id> {
        let start = instant::Instant::now();

        let matches = self.search(egraph);
        log::debug!("Found rewrite {} {} times", self.name, matches.len());

        let ids = self.apply(egraph, &matches);
        let elapsed = start.elapsed();
        log::debug!(
            "Applied rewrite {} {} times in {}.{:03}",
            self.name,
            ids.len(),
            elapsed.as_secs(),
            elapsed.subsec_millis()
        );

        ids
    }

    pub fn search(&self, egraph: &EGraph<L, M>) -> Vec<SearchMatches> {
        self.patterns
            .iter()
            .flat_map(|p| p.search(egraph))
            .collect()
    }

    pub fn apply(&self, egraph: &mut EGraph<L, M>, ematches: &[SearchMatches]) -> Vec<Id> {
        let mut applications = Vec::new();
        'outer: for ematch in ematches {
            for mapping in &ematch.mappings {
                if self.conditions.iter().all(|c| c.check(egraph, mapping)) {
                    for applier in &self.appliers {
                        for applied_root in applier.apply(egraph, mapping) {
                            // only union and return the id if we
                            // learned something from this application
                            if applied_root.id != ematch.eclass {
                                let leader = egraph.union(ematch.eclass, applied_root.id);
                                applications.push(leader);
                            }

                            if applications.len() > self.application_limit {
                                warn!(
                                    "Rule {} exceeded the limit: {}",
                                    self.name,
                                    applications.len()
                                );
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }

        applications
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Condition<L> {
    pub lhs: Pattern<L>,
    pub rhs: Pattern<L>,
}

impl<L: Language> Condition<L> {
    fn check<M>(&self, egraph: &mut EGraph<L, M>, mapping: &WildMap) -> bool
    where
        M: Metadata<L>,
    {
        let lhs_id = self.lhs.subst_and_find(egraph, mapping);
        let rhs_id = self.rhs.subst_and_find(egraph, mapping);
        lhs_id == rhs_id
    }
}

pub trait Applier<L: Language, M: Metadata<L>>: fmt::Debug {
    fn apply(&self, egraph: &mut EGraph<L, M>, mapping: &WildMap) -> Vec<AddResult>;
}

#[cfg(test)]
mod tests {

    use crate::{enode as e, *};

    fn wc<L: Language>(name: &QuestionMarkName) -> Pattern<L> {
        Pattern::Wildcard(name.clone(), crate::pattern::WildcardKind::Single)
    }

    #[test]
    fn conditional_rewrite() {
        crate::init_logger();
        let mut egraph = EGraph::<String, ()>::default();

        let pat = |e| Pattern::ENode(Box::new(e));
        let x = egraph.add(e!("x")).id;
        let y = egraph.add(e!("2")).id;
        let mul = egraph.add(e!("*", x, y)).id;

        let true_pat = pat(e!("TRUE"));
        let true_id = egraph.add(e!("TRUE")).id;

        let a: QuestionMarkName = "?a".parse().unwrap();
        let b: QuestionMarkName = "?b".parse().unwrap();

        let mul_to_shift = rw("mul_to_shift")
            .with_pattern(pat(e!("*", wc(&a), wc(&b))))
            .with_applier(pat(e!(">>", wc(&a), pat(e!("log2", wc(&b))),)))
            .with_condition(Condition {
                lhs: pat(e!("is-power2", wc(&b))),
                rhs: true_pat,
            })
            .mk();

        println!("rewrite shouldn't do anything yet");
        egraph.rebuild();
        let apps = mul_to_shift.run(&mut egraph);
        assert_eq!(apps, vec![]);

        println!("Add the needed equality");
        let two_ispow2 = egraph.add(e!("is-power2", y)).id;
        egraph.union(two_ispow2, true_id);

        println!("Should fire now");
        egraph.rebuild();
        let apps = mul_to_shift.run(&mut egraph);
        assert_eq!(apps, vec![mul]);
    }

    #[test]
    fn fn_rewrite() {
        crate::init_logger();
        let mut egraph = EGraph::<String, ()>::default();

        let start = "(+ x y)".parse().unwrap();
        let goal = "xy".parse().unwrap();

        let root = egraph.add_expr(&start);

        let a: QuestionMarkName = "?a".parse().unwrap();
        let b: QuestionMarkName = "?b".parse().unwrap();

        fn get(egraph: &EGraph<String, ()>, id: Id) -> &str {
            &egraph[id].nodes[0].op
        }

        #[derive(Debug)]
        struct Appender;
        impl Applier<String, ()> for Appender {
            fn apply(&self, egraph: &mut EGraph<String, ()>, map: &WildMap) -> Vec<AddResult> {
                let a: QuestionMarkName = "?a".parse().unwrap();
                let b: QuestionMarkName = "?b".parse().unwrap();
                let a = get(&egraph, map[&a][0]);
                let b = get(&egraph, map[&b][0]);
                let s = format!("{}{}", a, b);
                vec![egraph.add(e!(&s))]
            }
        }

        let pat = |e| Pattern::ENode(Box::new(e));
        let fold_add = rw("fold_add")
            .with_pattern(pat(e!("+", wc(&a), wc(&b))))
            .with_applier(Appender)
            .mk();

        fold_add.run(&mut egraph);
        assert_eq!(egraph.equivs(&start, &goal), vec![root]);
    }
}