use parser::constraints_from_json;

pub mod parser;
pub mod schema;
pub mod solver;
mod sketches;

use solver::Solver;

fn main() {
    env_logger::init();
    let program = constraints_from_json("tests/retypd-constrains-simple.json").unwrap();
    let solver = Solver::new(&program);
    solver.infer_shapes();
}
