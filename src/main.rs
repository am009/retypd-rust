use clap::{arg, command};
use parser::constraints_from_json;

pub mod parser;
pub mod schema;
pub mod solver;
pub mod sketches;
pub mod graph;

use solver::Solver;

fn main() {
    env_logger::init();
    let matches = command!()
        .arg(arg!([json_in] "Path to the constraints json file").default_value("tests/retypd-constrains-simple.json"))
        .get_matches();
    let program = constraints_from_json(matches.get_one::<String>("json_in").unwrap()).unwrap();
    let solver = Solver::new(&program);
    solver.infer_shapes();
}
