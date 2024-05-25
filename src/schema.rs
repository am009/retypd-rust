use std::collections::HashMap;

use petgraph::graph::DiGraph;


#[derive(Debug, PartialEq)]
pub enum FieldLabel {
    InPattern(u32),
    OutPattern(u32),
    DerefPattern {
        base: u32,
        offset: i32,
        bound: Option<Bound>,
    },
    Load,
    Store,
}

#[derive(Debug, PartialEq)]
pub enum Bound {
    Fixed(u32),
    NullTerm,
    NoBound,
}

#[derive(Debug, PartialEq)]
pub struct DerivedTypeVariable {
    pub name: String,
    // TODO refactor to a Field label pool
    pub fields: Vec<FieldLabel>,
}

#[derive(Debug, PartialEq)]
pub struct Constraint {
    pub left: DerivedTypeVariable,
    pub right: DerivedTypeVariable,
}


pub struct Program {
    pub language: String,
    // types: Lattice[DerivedTypeVariable],
    // global_vars: Iterable[MaybeVar],
    // proc_constraints: MaybeDict[MaybeVar, ConstraintSet],
    // callgraph: Union[
    //     MaybeDict[MaybeVar, Iterable[MaybeVar]], networkx.DiGraph
    // ],
    // TODO: save function name string space
    pub proc_constraints: HashMap<String, Vec<Constraint>>,
    pub call_graph: DiGraph<String, ()>,
}
