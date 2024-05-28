use crate::schema::{Bound, Constraint, DerivedTypeVariable, FieldLabel, Program};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, digit1, multispace0},
    combinator::{map, map_res, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};
use petgraph::graph::DiGraph;
use std::{collections::HashMap, error::Error, fs::File, io::BufReader, str::FromStr};

use serde_json::Value;

pub fn constraints_from_json(json_path: &str) -> Result<Program, Box<dyn Error>> {
    let file = File::open(json_path)?;
    let reader = BufReader::new(file);
    let val: Value = serde_json::from_reader(reader)?;

    // parse callgraphs
    let mut graph = DiGraph::<String, ()>::new();
    let mut nodes = HashMap::new();
    let call_graph = val["callgraph"].as_object().unwrap();

    // Add nodes to the graph
    for node in call_graph.keys() {
        let node_index = graph.add_node(node.to_string());
        nodes.insert(node.as_str(), node_index);
    }
    // Add edges to the graph
    for (node, edges) in call_graph {
        let &node_index = nodes.get(node.as_str()).expect("Node not found");

        for edge in edges.as_array().unwrap() {
            let &edge_index = nodes.get(edge.as_str().unwrap()).expect("Edge not found");
            graph.add_edge(node_index, edge_index, ());
        }
    }

    // parse constrains
    let mut proc_constraints: HashMap<String, Vec<Constraint>> = HashMap::new();
    let constraints = val["constraints"].as_object().unwrap();
    for (func_name, constraints) in constraints {
        let constraints_str = constraints.as_array().unwrap();
        let mut cs: Vec<Constraint> = Vec::new();
        for constraint in constraints_str {
            let constraint = constraint.as_str().unwrap();
            let (str, constraint) = parse_constraint(constraint).unwrap();
            assert!(str.len() == 0); // no reaming data
            cs.push(constraint);
        }
        // insert to proc constrains
        proc_constraints.insert(func_name.to_string(), cs);
    }
    Ok(Program {
        language: val["language"].as_str().unwrap().to_string(),
        call_graph: graph,
        proc_constraints: proc_constraints,
    })
}

// this is a rust parser to parse the following language:
// constraint = DerivedTypeVariable ("<=" | '⊑') DerivedTypeVariable
// DerivedTypeVariable = Identifier ( '.' FieldLabel )* | Identifier
// FieldLabel = in_pattern | out_pattern | deref_pattern | 'load' | 'store'
// in_pattern = re.compile("in_([0-9]+)")
// out_pattern = re.compile("out_([0-9]+)")
// deref_pattern = re.compile(
//     "σ([0-9]+)@(-?[0-9]+)(\*\[(([0-9]+)|nullterm|nobound)\])?"
// )
// node_pattern = re.compile(r"(\S+)\.([⊕⊖])")

fn parse_i32(input: &str) -> IResult<&str, i32> {
    let (i, number) = map_res(recognize(preceded(opt(tag("-")), digit1)), |s| {
        i32::from_str(s)
    })(input)?;

    Ok((i, number))
}

fn is_not_whitespace_or_dot(c: char) -> bool {
    !c.is_whitespace() && c != '.'
}

fn parse_identifier(input: &str) -> IResult<&str, String> {
    map(take_while1(is_not_whitespace_or_dot), |s: &str| {
        s.to_string()
    })(input)
}

fn parse_in_pattern(input: &str) -> IResult<&str, FieldLabel> {
    map(
        preceded(tag("in_"), map_res(digit1, |s: &str| s.parse::<u32>())),
        FieldLabel::InPattern,
    )(input)
}

fn parse_out_pattern(input: &str) -> IResult<&str, FieldLabel> {
    alt((
        map(
            preceded(tag("out_"), map_res(digit1, |s: &str| s.parse::<u32>())),
            FieldLabel::OutPattern,
        ),
        map(tag("out"), |_| FieldLabel::OutPattern(0)),
    ))(input)
}

fn parse_deref_pattern(input: &str) -> IResult<&str, FieldLabel> {
    map(
        tuple((
            preceded(tag("σ"), map_res(digit1, |s: &str| s.parse::<u32>())),
            preceded(char('@'), parse_i32),
            opt(delimited(
                tag("*["),
                alt((
                    map(tag("nullterm"), |_| Bound::NullTerm),
                    map(tag("nobound"), |_| Bound::NoBound),
                    map(map_res(digit1, |s: &str| s.parse::<u32>()), Bound::Fixed),
                )),
                char(']'),
            )),
        )),
        |(base, offset, bound)| FieldLabel::DerefPattern {
            size: base,
            offset,
            bound,
        },
    )(input)
}

fn parse_load(input: &str) -> IResult<&str, FieldLabel> {
    map(tag("load"), |_| FieldLabel::Load)(input)
}

fn parse_store(input: &str) -> IResult<&str, FieldLabel> {
    map(tag("store"), |_| FieldLabel::Store)(input)
}

fn parse_field_label(input: &str) -> IResult<&str, FieldLabel> {
    alt((
        parse_in_pattern,
        parse_out_pattern,
        parse_deref_pattern,
        parse_load,
        parse_store,
    ))(input)
}

fn parse_derived_type_variable(input: &str) -> IResult<&str, DerivedTypeVariable> {
    map(
        pair(
            parse_identifier,
            many0(preceded(char('.'), parse_field_label)),
        ),
        |(identifier, fields)| DerivedTypeVariable {
            name: identifier,
            fields,
        },
    )(input)
}

fn parse_constraint(input: &str) -> IResult<&str, Constraint> {
    map(
        tuple((
            parse_derived_type_variable,
            preceded(
                delimited(multispace0, alt((tag("<="), tag("⊑"))), multispace0),
                parse_derived_type_variable,
            ),
        )),
        |(left, right)| Constraint { left, right },
    )(input)
}
