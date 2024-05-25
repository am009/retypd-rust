use parser::constraints_from_json;

pub mod parser;
pub mod schema;
pub mod solver;

use solver::Solver;

fn main() {
    let program = constraints_from_json("retypd-constrains-simple.json").unwrap();
    let solver = Solver::new(&program);
    solver.solve();
}
