use crate::schema::DerivedTypeVariable;


struct SketchNode {
    dtv: DerivedTypeVariable,
    // these two bound is attached auxillary data.
    lower_bound: DerivedTypeVariable,
    upper_bound: DerivedTypeVariable,
}

struct Sketch {
    // directed graph
    // node lookup map from dtv to node index
    // root node
    // reference to type lattice
}