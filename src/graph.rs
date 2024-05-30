use std::{
    collections::{HashMap, HashSet}, env, error::Error, fmt::{self, Debug, Display}, fs::File, io::Write
};

use petgraph::{
    algo::{condensation, toposort},
    dot::Dot,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use crate::schema::{Constraint, DerivedTypeVariable, FieldLabel, Program, Variance};

/// This file contains the graph used for saturation and transducer in Appendix D.
///

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum EdgeLabel {
    One,
    Forget {
        capability: FieldLabel,
        // variance: Variance,
    },
    Recall {
        capability: FieldLabel,
        // variance: Variance,
    },
}

impl Display for EdgeLabel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EdgeLabel::One => write!(f, "_1_"),
            EdgeLabel::Forget { capability } => write!(f, "forget {}", capability),
            EdgeLabel::Recall { capability } => write!(f, "recall {}", capability),
        }
    }
}

impl Debug for EdgeLabel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

// #[derive(PartialEq, Eq, Hash, Clone)]
// pub struct EdgeLabel {
//     pub kind: EdgeLabelKind,
//     pub capability: FieldLabel,
//     pub variance: Variance,
// }

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum SideMark {
    None,
    Left,
    Right,
}

impl Display for SideMark {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SideMark::None => Ok(()),
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

impl Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl Node {
    pub fn forget_once(&self) -> Option<(FieldLabel, Node)> {
        if self.base.fields.is_empty() {
            return None;
        }
        let mut base = self.base.clone();
        let last = base.fields.pop().unwrap();
        let variance = self.suffix_variance.combine(&last.variance());
        let node = Node {
            base,
            suffix_variance: variance,
            sidemark: self.sidemark.clone(),
        };
        Some((last, node))
    }
}

pub struct ConstraintGraph {
    pub graph: DiGraph<Node, EdgeLabel>,
    pub graph_node_map: HashMap<Node, NodeIndex>,
}

impl ConstraintGraph {
    pub fn construct() -> Self {
        ConstraintGraph {
            graph: DiGraph::new(),
            graph_node_map: HashMap::new(),
        }
    }
    pub fn new(constraints: Vec<&Constraint>) -> Self {
        let mut g = ConstraintGraph {
            graph: DiGraph::new(),
            graph_node_map: HashMap::new(),
        };
        // 1. build the initial graph (Algorithm D.1 Transducer)
        g.build_initial_graph(constraints);
        // print the graph for debugging
        if let Some(path) = env::var("DEBUG_TRANS_INIT_GRAPH").ok() {
            let mut file = File::create(path).unwrap();
            write!(file, "{:?}", Dot::new(&g.graph)).unwrap();
        }
        // 2. saturate the graph
        g.saturate();
        // print the graph for debugging
        if let Some(path) = env::var("DEBUG_TRANS_SAT_GRAPH").ok() {
            let mut file = File::create(path).unwrap();
            write!(file, "{:?}", Dot::new(&g.graph)).unwrap();
        }
        // g.pathexpr();
        g
    }
    pub fn add_node(&mut self, node: Node) -> NodeIndex {
        if let Some(index) = self.graph_node_map.get(&node) {
            return *index;
        }
        let node_index = self.graph.add_node(node.clone());
        self.graph_node_map.insert(node.clone(), node_index);
        node_index
    }
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, label: EdgeLabel) -> bool {
        // self edge is not meaningful
        if from == to {
            false
        } else if let Some(_edge) = self
            .graph
            .edges_connecting(from, to)
            .find(|edge| edge.weight() == &label)
        {
            false
        } else {
            self.graph.add_edge(from, to, label);
            true
        }
    }

    fn add_recalls(&mut self, mut node_ind: NodeIndex) {
        let node = self.graph.node_weight(node_ind).unwrap().clone();
        let mut t = node.forget_once();
        while t.is_some() {
            let (cap, next) = t.unwrap();
            let next_ind = self.add_node(next.clone());
            self.add_edge(next_ind, node_ind, EdgeLabel::Recall { capability: cap });
            t = next.forget_once();
            node_ind = next_ind;
        }
    }

    fn add_forgets(&mut self, mut node_ind: NodeIndex) {
        let node = self.graph.node_weight(node_ind).unwrap().clone();
        let mut t = node.forget_once();
        while t.is_some() {
            let (cap, next) = t.unwrap();
            let next_ind = self.add_node(next.clone());
            self.add_edge(node_ind, next_ind, EdgeLabel::Forget { capability: cap });
            t = next.forget_once();
            node_ind = next_ind;
        }
    }

    /// build the initial graph (Algorithm D.1 Transducer)
    pub fn build_initial_graph(&mut self, constraints: Vec<&Constraint>) {
        // add start and end node? TODO
        for c in constraints {
            // 1. add two node and 1-labeled edge
            // TODO should we add left or right side mark label or not?
            //    related to the set of interesting variables.
            let node_l = self.add_node(Node {
                base: c.left.clone(),
                suffix_variance: Variance::Covariant,
                sidemark: SideMark::None, // TODO
            });
            let node_r = self.add_node(Node {
                base: c.right.clone(),
                suffix_variance: Variance::Covariant,
                sidemark: SideMark::None, // TODO
            });
            // add 1-labeled edge between them
            self.graph.add_edge(node_l, node_r, EdgeLabel::One);
            // 2. add each sub var node and edges.
            // 2.1 left
            self.add_recalls(node_l);
            // 2.2 right
            self.add_forgets(node_r);
            // TODO add the start and end edge?

            // 3-4 the inverse of the above
            // 3. inverse node and 1-labeled edge
            let r_node_l = self.add_node(Node {
                base: c.left.clone(),
                suffix_variance: Variance::Contravariant,
                sidemark: SideMark::None, // TODO
            });
            let r_node_r = self.add_node(Node {
                base: c.right.clone(),
                suffix_variance: Variance::Contravariant,
                sidemark: SideMark::None, // TODO
            });
            // add 1-labeled edge between them
            self.graph.add_edge(r_node_r, r_node_l, EdgeLabel::One);
            // 4.1 inverse left
            self.add_recalls(r_node_l);
            // 4.2 inverse right
            self.add_forgets(r_node_r);
            // TODO add the start and end edge?
        }
    }
    pub fn saturate(&mut self) {
        // reaching_set changed or graph changed
        let mut changed = false;
        let mut reaching_set = HashMap::<NodeIndex, HashSet<(FieldLabel, NodeIndex)>>::new();

        let add_reaching =
            |reaching_set: &mut HashMap<NodeIndex, HashSet<(FieldLabel, NodeIndex)>>,
             dest,
             elem| {
                let set = reaching_set.entry(dest).or_default();
                set.insert(elem)
            };
        // 1. add forget edge to reaching set
        for edge in self.graph.raw_edges() {
            if let EdgeLabel::Forget { capability } = &edge.weight {
                changed |= add_reaching(
                    &mut reaching_set,
                    edge.target(),
                    (capability.clone(), edge.source()),
                );
            }
        }
        while changed {
            changed = false;
            for edge in self.graph.raw_edges() {
                if let EdgeLabel::One = &edge.weight {
                    let source = edge.source();
                    let target = edge.target();
                    if let Some(set) = reaching_set.get(&source) {
                        for (cap, node) in set.clone() {
                            changed |= add_reaching(&mut reaching_set, target, (cap, node));
                        }
                    }
                }
            }
            let mut to_add = Vec::new();
            for edge in self.graph.raw_edges() {
                if let EdgeLabel::Recall { capability } = &edge.weight {
                    let source = edge.source();
                    let target = edge.target();
                    if let Some(set) = reaching_set.get(&source) {
                        for (cap, node) in set {
                            if cap == capability {
                                log::debug!("Adding edge from {} to {} with {}", self.graph.node_weight(*node).unwrap(), self.graph.node_weight(target).unwrap(), EdgeLabel::One);
                                to_add.push((node.to_owned(), target, EdgeLabel::One));
                            }
                        }
                    }
                }
            }
            for (source, target, label) in to_add {
                changed |= self.add_edge(source, target, label);
            }
            let mut to_add_invert = Vec::new();
            for node_ind in self.graph.node_indices() {
                let node_x = self.graph.node_weight(node_ind).unwrap();
                if node_x.suffix_variance == Variance::Contravariant {
                    if let Some(set) = reaching_set.get(&node_ind) {
                        for (cap, node) in set {
                            if cap == &FieldLabel::Store {
                                log::debug!("node {} can reach node {} with {}.", self.graph.node_weight(*node).unwrap(), node_x, cap);
                                to_add_invert.push((node.to_owned(), FieldLabel::Load, node_ind));
                            }
                            if cap == &FieldLabel::Load {
                                log::debug!("node {} can reach node {} with {}.", self.graph.node_weight(*node).unwrap(), node_x, cap);
                                to_add_invert.push((node.to_owned(), FieldLabel::Store, node_ind));
                            }
                        }
                    }
                }
            }
            for (source, cap, target) in to_add_invert {
                // find the variance inverted node.
                let mut node = self.graph.node_weight(target).unwrap().clone();
                log::debug!("Process: node {} can reach node {} with {}.", self.graph.node_weight(source).unwrap(), node, if cap == FieldLabel::Load {"store"} else {"load"} );
                node.suffix_variance = node.suffix_variance.invert();
                // find the target node.
                log::debug!("Try to add reaching set elem ({}, {}) to R({})", self.graph.node_weight(source).unwrap(), cap, node);
                let inv_target = self.graph_node_map.get(&node).unwrap();
                changed |= add_reaching(&mut reaching_set, inv_target.clone(), (cap, source));
            }
        }
    }
}

pub fn infer_proc_types(program: &Program) {
    // type schemes for each function
    let mut type_schemes: HashMap<String, Vec<Constraint>> = std::collections::HashMap::new();

    // find the scc in the callgraph, and iterate in post order
    let sccs = condensation(program.call_graph.clone(), true);
    let topo_sort = toposort(&sccs, None).unwrap();
    for ind in topo_sort.iter().rev() {
        let mut constraints: Vec<&Constraint> = Vec::new();
        // collect constraints for the scc:
        // 1. instantiate type schemes for each call
        // 1. instantiate constraints for global variable.
        for proc in sccs.node_weight(*ind).unwrap() {
            assert!(!type_schemes.contains_key(proc));
            // TODO for each call outside of SCC, instantiate the type scheme.
            for c in program.proc_constraints.get(proc).unwrap() {
                constraints.push(c);
            }
        }

        let mut cg = ConstraintGraph::new(constraints);
        // 3. collect the set of interesting vars and run pathexpr on them
        // 4. create sketches for each function
    }
}

#[cfg(test)]
mod tests {
    use super::ConstraintGraph;
    use crate::graph::{Node, SideMark};
    use crate::parser::{parse_constraint, parse_derived_type_variable};
    use crate::schema::{Constraint, DerivedTypeVariable, Variance};
    use petgraph::dot::Dot;
    use std::fs::{self, File};
    use std::io::{Read, Write};

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn parse_constraint_str(cons: &[&str]) -> Vec<Constraint> {
        let mut constraints = Vec::new();
        for c in cons {
            let c = parse_constraint(c).unwrap();
            assert!(c.0.len() == 0);
            constraints.push(c.1);
        }
        constraints
    }

    #[test]
    fn test_slides_example() {
        let constraints = [
            "F.in_stack0 <= 𝛿",
            "𝛼 <= 𝜑",
            "𝛿 <= 𝜑",
            "𝜑.load.σ4@0 <= 𝛼",
            "𝜑.load.σ4@4 <= 𝛼'",
            "𝛼' <= close.in_stack0",
            "close.out_eax <= F.out_eax",
            "close.in_stack0 <= _FileDescriptor",
            "_SuccessZ <= close.out_eax",
        ];
        let constraints = parse_constraint_str(&constraints);

        let mut cg = ConstraintGraph::construct();
        cg.build_initial_graph(constraints.iter().collect());
        let dot = Dot::new(&cg.graph).to_string();
        // let mut file = File::create("slides_example.dot").unwrap();
        // write!(file, "{:?}", Dot::new(&cg.graph)).unwrap();
        let answer = fs::read_to_string("tests/slides_example.dot")
            .expect("Unable to read tests/test_saturation.dot");
        assert!(dot == answer);
    }

    #[test]
    fn test_saturation() {
        init();
        let constraints = parse_constraint_str(&[
            "y <= p",
            "p <= x",
            "_A <= x.store",
            "y.load <= _B"
        ]);
        let cg = ConstraintGraph::new(constraints.iter().collect());
        
        let mut file = File::create("sat-paper.dot").unwrap();
        write!(file, "{:?}", Dot::new(&cg.graph)).unwrap();

        let x_store_plus = cg.graph_node_map.get(&Node {
            base: parse_derived_type_variable("x.store").unwrap().1,
            suffix_variance: Variance::Covariant,
            sidemark: SideMark::None,
        }).unwrap();
        let y_load_plus = cg.graph_node_map.get(&Node {
            base: parse_derived_type_variable("y.load").unwrap().1,
            suffix_variance: Variance::Covariant,
            sidemark: SideMark::None,
        }).unwrap();
        let mut has_one = false;
        for edge in cg.graph.edges_connecting(x_store_plus.to_owned(), y_load_plus.to_owned()) {
            if edge.weight() == &super::EdgeLabel::One {
                has_one = true;
            }
        }
        assert!(has_one, "Cannot infer subtype relation x.store <= y.load !");
    }
}
