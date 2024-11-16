#[derive(Debug, PartialEq)]
struct Object {
    tpe: String,
    id: i64,
}

#[derive(Debug, PartialEq)]
struct Triple {
    from: Object,
    to: Object,
    relation: String,
}
