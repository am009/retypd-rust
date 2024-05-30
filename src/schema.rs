use core::fmt;
use std::{collections::HashMap, fmt::Debug};

use petgraph::graph::DiGraph;

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Variance {
    Covariant,
    Contravariant,
}

impl fmt::Display for Variance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Variance::Covariant => write!(f, "⊕"),
            Variance::Contravariant => write!(f, "⊖"),
        }
    }
}

impl Variance {
    pub fn invert(&self) -> Variance {
        match self {
            Variance::Covariant => Variance::Contravariant,
            Variance::Contravariant => Variance::Covariant,
        }
    }
    pub fn combine(&self, other: &Variance) -> Variance {
        if self == other {
            Variance::Covariant
        } else {
            Variance::Contravariant
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum FieldLabel {
    InPattern(String),
    OutPattern(String),
    DerefPattern {
        size: u32,
        offset: i32,
        bound: Option<Bound>,
    },
    Load,
    Store,
}

impl fmt::Display for FieldLabel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FieldLabel::InPattern(i) => write!(f, "in_{}", i),
            FieldLabel::OutPattern(i) => write!(f, "out_{}", i),
            FieldLabel::DerefPattern {
                size: base,
                offset,
                bound,
            } => {
                write!(f, "σ{}@{}", base, offset)?;
                if let Some(b) = bound {
                    write!(f, "{}", b)
                } else {
                    Ok(())
                }
            }
            FieldLabel::Load => write!(f, "load"),
            FieldLabel::Store => write!(f, "store"),
        }
    }
}

impl Debug for FieldLabel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl FieldLabel {
    pub fn variance(&self) -> Variance {
        match self {
            FieldLabel::InPattern(_) => Variance::Contravariant,
            FieldLabel::OutPattern(_) => Variance::Covariant,
            FieldLabel::DerefPattern {
                size: _,
                offset: _,
                bound: _,
            } => Variance::Covariant,
            FieldLabel::Load => Variance::Covariant,
            FieldLabel::Store => Variance::Contravariant,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Bound {
    Fixed(u32),
    NullTerm,
    NoBound,
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Bound::Fixed(i) => write!(f, "*[{}]", i),
            Bound::NullTerm => write!(f, "nullterm"),
            Bound::NoBound => write!(f, "nobound"),
        }
    }
}

impl Debug for Bound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct DerivedTypeVariable {
    pub name: String,
    // TODO refactor to a Field label pool
    pub fields: Vec<FieldLabel>,
}

impl fmt::Display for DerivedTypeVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)?;
        for field in &self.fields {
            write!(f, ".{}", field)?;
        }
        Ok(())
    }
}

impl Debug for DerivedTypeVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl DerivedTypeVariable {
    pub fn get_sub_dtv(&self, index: usize) -> DerivedTypeVariable {
        DerivedTypeVariable {
            name: self.name.clone(),
            fields: self.fields[..index].to_vec(),
        }
    }
    pub fn path_variance(&self) -> Variance {
        let mut variance = Variance::Covariant;
        for field in &self.fields {
            variance = variance.combine(&field.variance());
        }
        variance
    }
}

#[derive(PartialEq)]
pub struct Constraint {
    pub left: DerivedTypeVariable,
    pub right: DerivedTypeVariable,
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} <= {}", self.left, self.right)
    }
}

impl Debug for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct Program {
    pub language: String,
    // types: Lattice[DerivedTypeVariable],
    /// types for global variables
    // global_vars: Iterable[MaybeVar],
    // TODO: save function name string space
    // initial constraints for each function.
    pub proc_constraints: HashMap<String, Vec<Constraint>>,
    pub call_graph: DiGraph<String, ()>,
}
