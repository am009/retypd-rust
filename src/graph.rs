use std::{collections::HashMap, fmt::{self, Display}};

use petgraph::{
    algo::{condensation, toposort},
    dot::Dot,
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};

use crate::schema::{Constraint, DerivedTypeVariable, FieldLabel, Program, Variance};

/// This file contains the graph used for saturation and transducer in Appendix D.
///

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum EdgeLabelKind {
    Forget,
    Recall,
}

impl Display for EdgeLabelKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EdgeLabelKind::Forget => write!(f, "forget"),
            EdgeLabelKind::Recall => write!(f, "recall"),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct EdgeLabel {
    pub kind: EdgeLabelKind,
    pub capability: FieldLabel,
    pub variance: Variance,
}

impl Display for EdgeLabel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.capability)
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum SideMark {
    No,
    Left,
    Right,
}

impl Display for SideMark {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SideMark::No => Ok(()),
            SideMark::Left => write!(f, "L:"),
            SideMark::Right => write!(f, "R:"),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Node {
    pub base: DerivedTypeVariable,
    pub suffix_variance: Variance,
    pub sidemark: SideMark,
}

impl Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}.{}", self.sidemark, self.base, self.suffix_variance)
    }
}

pub struct ConstraintGraph {
    pub g: DiGraph<Node, EdgeLabel>,
    pub gm: HashMap<Node, NodeIndex>,
}

pub fn infer_proc_types(program: &Program) {
    // type schemes for each function
    let mut type_schemes: HashMap<String, Vec<Constraint>> = std::collections::HashMap::new();

    // find the scc in the callgraph, and iterate in post order
    let sccs = condensation(program.call_graph.clone(), true);
    let topo_sort = toposort(&sccs, None).unwrap();
    for ind in topo_sort.iter().rev() {
        let mut constrains: Vec<&Constraint> = Vec::new();
        // collect constraints for the scc
        for proc in sccs.node_weight(*ind).unwrap() {
            assert!(!type_schemes.contains_key(proc));
            for c in program.proc_constraints.get(proc).unwrap() {
                constrains.push(c);
            }
        }
        // 1. build the initial graph
        // 2. saturate the graph
        // 3. collect the set of interesting vars and run pathexpr on them
        // 4. create sketches for each function
    }
}
