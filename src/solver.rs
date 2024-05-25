use petgraph::{
    algo::{condensation, tarjan_scc, toposort},
    visit::DfsPostOrder,
};

use crate::schema::{Constraint, Program};

pub struct Solver<'a> {
    pub program: &'a Program,
}

impl Solver<'_> {
    pub fn new(program: &Program) -> Solver {
        Solver { program }
    }
    pub fn solve(self: Self) -> () {
        // find the scc in the callgraph, and iterate in post order
        let sccs = condensation(self.program.call_graph.clone(), true);
        let topo_sort = toposort(&sccs, None).unwrap();
        for ind in topo_sort.iter().rev() {
            let mut constrains: Vec<&Constraint> = Vec::new();
            // collect constraints for the scc
            for proc in sccs.node_weight(*ind).unwrap() {
                for c in self.program.proc_constraints.get(proc).unwrap() {
                    constrains.push(c);
                }
            }
            // 1. build the initial graph
            // 2. saturate the graph
            // 3. collect the set of interesting vars and run pathexpr on them
            // 4. create sketches for each function
        }
    }
}
