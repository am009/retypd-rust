use std::{collections::HashMap, env, fmt::Debug, fs::File, io::Write};

use petgraph::{
    algo::{condensation, toposort},
    dot::Dot,
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};

use crate::{graph::infer_proc_types, schema::{Constraint, DerivedTypeVariable, FieldLabel, Program}};

pub struct Solver<'a> {
    pub program: &'a Program,
}

impl Solver<'_> {
    pub fn new(program: &Program) -> Solver {
        Solver { program }
    }
    pub fn solve(self: Self) -> () {
        infer_proc_types(self.program);
    }

    // TODO Probably should not do this to the whole program? but for a func at a time
    /// Infer the sketches for a set of constraints.
    /// Algorithm E.1 in paper.
    pub fn infer_shapes(self: Self) -> () {
        struct Node {
            dtv: DerivedTypeVariable,
            represent: Option<NodeIndex>,
        }

        impl Debug for Node {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{:?}", self.dtv)
            }
        }

        let mut g = DiGraph::<Node, FieldLabel>::new();
        let mut gm: HashMap<DerivedTypeVariable, NodeIndex> = HashMap::new();

        fn find_equiv_group(g: &mut DiGraph<Node, FieldLabel>, mut ind: NodeIndex) -> NodeIndex {
            let mut vec = Vec::new();
            let mut node = g.node_weight(ind).unwrap();
            while node.represent.is_some() && node.represent.unwrap() != ind {
                vec.push(ind);
                ind = node.represent.unwrap();
                node = g.node_weight(ind).unwrap();
            }
            let ret = ind;
            for ind in vec {
                if ind != ret {
                    let node = g.node_weight_mut(ind).unwrap();
                    node.represent = Some(ret);
                }
            }
            ret
        }

        fn unify(g: &mut DiGraph<Node, FieldLabel>, x: NodeIndex, y: NodeIndex) -> () {
            if x != y {
                // make x the representative of y
                let node = g.node_weight_mut(y).unwrap();
                node.represent = Some(x);
                let mut to_unify = Vec::new();
                for edge_x in g.edges_directed(x, petgraph::Direction::Outgoing) {
                    let label_x = edge_x.weight();
                    for edge_y in g.edges_directed(y, petgraph::Direction::Outgoing) {
                        let label_y = edge_y.weight();
                        // unify if the labels are the same, or one is load and the other is store.
                        if label_x == label_y
                            || (label_x == &FieldLabel::Load && label_y == &FieldLabel::Store)
                            || (label_x == &FieldLabel::Store && label_y == &FieldLabel::Load)
                        {
                            log::debug!(
                                "Unify: there is a edge from {:?} to {:?} with label {:?}",
                                g.node_weight(edge_x.source()),
                                g.node_weight(edge_x.target()),
                                label_x
                            );
                            log::debug!(
                                "And a edge from {:?} to {:?} with label {:?}",
                                g.node_weight(edge_y.source()),
                                g.node_weight(edge_y.target()),
                                label_y
                            );
                            let node_x = edge_x.target();
                            let node_y = edge_y.target();
                            to_unify.push((node_x, node_y));
                        }
                    }
                }
                for (node_x, node_y) in to_unify {
                    unify(g, node_x, node_y);
                }
            }
        }

        for (_, cons) in &self.program.proc_constraints {
            // TODO deduplicate dtv beforehand
            for c in cons {
                for c in [&c.left, &c.right] {
                    let mut prev_id: Option<NodeIndex> = None;
                    // handle base type variable
                    if c.fields.len() == 0 {
                        if !gm.contains_key(&c) {
                            let node = Node {
                                dtv: c.clone(),
                                represent: None,
                            };
                            let node_index = g.add_node(node);
                            gm.insert(c.clone(), node_index);
                        }
                    }
                    // handle derived type variable
                    for i in 1..=c.fields.len() {
                        let dtv_l = c.get_sub_dtv(i - 1);
                        // log::debug!("For two dtv {:?}.", dtv_l);
                        // find the equiv group of sub_dtv(i-1)
                        let node_id = if i == 1 {
                            if !gm.contains_key(&dtv_l) {
                                let node = Node {
                                    dtv: dtv_l.clone(),
                                    represent: None,
                                };
                                let node_index = g.add_node(node);
                                gm.insert(dtv_l.clone(), node_index);
                                node_index
                            } else {
                                find_equiv_group(&mut g, gm.get(&dtv_l).unwrap().clone())
                            }
                        } else {
                            prev_id.unwrap()
                        };
                        // find the equiv group of sub_dtv(i).
                        let dtv_r = c.get_sub_dtv(i);
                        // log::debug!("And dtv {:?}.", dtv_r);
                        let new_node_id = if !gm.contains_key(&dtv_r) {
                            let node = Node {
                                dtv: dtv_r.clone(),
                                represent: None,
                            };
                            let node_index = g.add_node(node);
                            gm.insert(dtv_r.clone(), node_index);
                            node_index
                        } else {
                            find_equiv_group(&mut g, gm.get(&dtv_r).unwrap().clone())
                        };
                        // create edge with field label i, if not exist
                        if !g
                            .edges_connecting(node_id, new_node_id)
                            .any(|edge| edge.weight() == &c.fields[i - 1])
                        {
                            g.add_edge(node_id, new_node_id, c.fields[i - 1].clone());
                        }
                        prev_id = Some(new_node_id);
                    }
                }
            }
        }

        // print the graph for debugging
        if let Some(g_path) = env::var("DEBUG_G_GRAPH").ok() {
            let mut file = File::create(g_path).unwrap();
            write!(file, "{:?}", Dot::new(&g)).unwrap();
        }

        for (_, cons) in &self.program.proc_constraints {
            for c in cons {
                let ind = gm.get(&c.left).unwrap();
                let x = find_equiv_group(&mut g, *ind);
                let ind2 = gm.get(&c.right).unwrap();
                let y = find_equiv_group(&mut g, *ind2);
                unify(&mut g, x, y)
            }
        }
        // build the g quotient graph
        let mut g_quotient = DiGraph::<Vec<DerivedTypeVariable>, FieldLabel>::new();
        // map from node in g to node in g_quotient
        let mut gm_quotient: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        for ind in g.node_indices() {
            let rep = find_equiv_group(&mut g, ind);
            let node = g.node_weight(ind).unwrap();
            if !gm_quotient.contains_key(&rep) {
                let mut vec = Vec::new();
                vec.push(node.dtv.clone());
                let node = g_quotient.add_node(vec);
                gm_quotient.insert(rep, node);
            } else {
                let node2 = g_quotient
                    .node_weight_mut(gm_quotient.get(&rep).unwrap().clone())
                    .unwrap();
                node2.push(node.dtv.clone());
            }
        }

        for ind in g.edge_indices() {
            let source = g.edge_endpoints(ind).unwrap().0;
            let target = g.edge_endpoints(ind).unwrap().1;
            let source_rep = find_equiv_group(&mut g, source);
            let target_rep = find_equiv_group(&mut g, target);
            let source_quotient = gm_quotient.get(&source_rep).unwrap();
            let target_quotient = gm_quotient.get(&target_rep).unwrap();
            let edge = g.edge_weight(ind).unwrap();
            g_quotient.add_edge(
                source_quotient.clone(),
                target_quotient.clone(),
                edge.clone(),
            );
        }

        // print the graph for debugging
        if let Some(g_quotient_path) = env::var("DEBUG_G_QUOTIENT_GRAPH").ok() {
            let mut file = File::create(g_quotient_path).unwrap();
            write!(file, "{:?}", Dot::new(&g_quotient)).unwrap();
        }
    }
}
