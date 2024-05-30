use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::{self, Debug, Display},
    fs::File,
    io::Write,
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
        write!(f, "{}{}{}", self.sidemark, self.base, self.suffix_variance)
    }
}

impl Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct ConstraintGraph {
    pub graph: DiGraph<Node, EdgeLabel>,
    pub graph_node_map: HashMap<Node, NodeIndex>,
}

impl ConstraintGraph {
    pub fn new(constrains: Vec<&Constraint>) -> Self {
        let mut g = ConstraintGraph {
            graph: DiGraph::new(),
            graph_node_map: HashMap::new(),
        };
        // 1. build the initial graph (Algorithm D.1 Transducer)
        g.build_initial_graph(constrains);
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
        if let Some(_edge) = self
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
    /// build the initial graph (Algorithm D.1 Transducer)
    pub fn build_initial_graph(&mut self, constrains: Vec<&Constraint>) {
        // add start and end node? TODO
        for c in constrains {
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
            let mut current = c.left.clone();
            let mut variance = Variance::Covariant;
            let mut prev_node = node_l;
            for _ in 0..c.left.fields.len() {
                let label = current.fields.pop().unwrap();
                variance = variance.combine(&label.variance());
                let node = Node {
                    base: current.clone(),
                    suffix_variance: variance.clone(),
                    sidemark: SideMark::None,
                };
                let node_index = self.add_node(node);
                let edge = EdgeLabel::Recall { capability: label };
                self.add_edge(node_index, prev_node, edge);
                prev_node = node_index;
            }
            // 2.2 right
            let mut current = c.right.clone();
            let mut variance = Variance::Covariant;
            let mut prev_node = node_r;
            for _ in 0..c.right.fields.len() {
                let label = current.fields.pop().unwrap();
                variance = variance.combine(&label.variance());
                let node = Node {
                    base: current.clone(),
                    suffix_variance: variance.clone(),
                    sidemark: SideMark::None,
                };
                let node_index = self.add_node(node);
                let edge = EdgeLabel::Forget { capability: label };
                self.add_edge(prev_node, node_index, edge);
                prev_node = node_index;
            }
            // 3. TODO add the start and end edge.
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
                let node = self.graph.node_weight(node_ind).unwrap();
                if node.suffix_variance == Variance::Contravariant {
                    if let Some(set) = reaching_set.get(&node_ind) {
                        for (cap, node) in set {
                            if cap == &FieldLabel::Store {
                                to_add_invert.push((node_ind, FieldLabel::Load, node.to_owned()));
                            }
                            if cap == &FieldLabel::Load {
                                to_add_invert.push((node_ind, FieldLabel::Store, node.to_owned()));
                            }
                        }
                    }
                }
            }
            for (source, cap, target) in to_add_invert {
                changed |= add_reaching(&mut reaching_set, target, (cap, source));
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
        let mut constrains: Vec<&Constraint> = Vec::new();
        // collect constraints for the scc:
        // 1. instantiate type schemes for each call
        // 1. instantiate constrains for global variable.
        for proc in sccs.node_weight(*ind).unwrap() {
            assert!(!type_schemes.contains_key(proc));
            // TODO for each call outside of SCC, instantiate the type scheme.
            for c in program.proc_constraints.get(proc).unwrap() {
                constrains.push(c);
            }
        }

        let mut cg = ConstraintGraph::new(constrains);
        // 3. collect the set of interesting vars and run pathexpr on them
        // 4. create sketches for each function
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_constraint;
    #[test]
    fn test_saturation() {
        let constraints = [
            "F.in_0 <= var1",
            "var2 <= var3",
            "var1 <= var3",
            "var3.load.σ4@0 <= var2",
            "var3.load.σ4@4 <= var4",
            "var4 <= close.in_0",
            "close.out_eax <= F.out_eax",
            "close.in_0 <= FileDescriptor",
            "SuccessZ <= close.out_eax",
        ];
        let mut constrains = Vec::new();
        for c in constraints.iter() {
            let c = parse_constraint(c).unwrap();
            assert!(c.0.len() == 0);
            constrains.push(c.1);
        }
    }
}
