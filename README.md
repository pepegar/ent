# ENT

Storage agnostic ORM & database for graph data (Exclusively DAGs).

Clients can tune the consistency level on reads. (fullconsistency, after(timestamp), fastest)

## TODO

- [x] find a way to create tokio servers from grpc
- [ ] ci: run tests in gha
- [ ] cd: create -latest images on push, push
- [ ] think about zookies?  This may constrain the datastores we can use.  Does psql support point in time?
- [x] define grpc for schemaservice, graphservice & userservice
- [x] find a way to create migrations
- [ ] find a library for jsonschema validation
- [ ] describe schema evolution principles
- [x] create initial migrations for all subsystems
- [ ] protobuf: integrate buf.build
- [ ] license: find a license that allows users to self-deploy, but not comercialize.
- [ ] client: think about the ORM side of things
- [x] deployment: create dockerfile
- [ ] deployment: create helm

## Users vs Clients

Clients perform admin operations in ENT.
Users are end users.

## Idea for transforming user ORM actions to SQL

I want to allow users to skip defining tables, and make it flexible for them to
access.

## Developer flow

### 1. Declare schemata

QUESTION: allow users to declare schemata in protobuf?  Would require compilation of their models.

Your schema is defined in json-schema terms.

```json
{
    "type"...
    ...
    ...
}
```

You can use `ent` cli to register this schema in ent server.

### 2. Declare objects

```rust
#[derive(Entity)]
struct Chat {
    id: uint64,
    name: String, // TODO: Think about how to allow LWW or other kind of conflict resolution for object fields. Automerge?
}
```

Entity derivation will generate (if it doesn't exist) fields for type & id, and
will create a method `get_metadata` that gets the rest of the fields into an
json object.

### 3. Query

```rust
/// your typical monadic runtime.
/// evaluating requires a natural transformation `EntOp ~> Result`
enum EntOp<T> {
    Err,
    Ok(T)
}

let entClient = EntClient::connect(...).await

let documents: EntOp<Vector<Document>> = entClient.queryObjects<Document>(type: "documents", viewer: currentUser);

let documents = documents.?;


// it'd be great if we could've a query planner that identifies this n+1 query and perform fusion...
// We need an algebra for graph operations.
documents
    .iter()
    .map(|doc| {
        let comments = EntOp<Vector<Comment>> = doc.getEdges<Comment>(type: "comment")
    })
    .exec()

```

### Algebra of graph operations

- get object
- batch get object
- get edge
- batch get edge
- get edge list
- batch get edge list

## db

### Triple store

Ent server is a triplestore

```sql
create table if not exists schemata(
    id int,
    schema jsonb,
);

create table if not exists objects (
    id int,
    metadata jsonb,
    created_at timestamp,
    updated_at timestamp,
);

create table if not exists triples (
    from_type text,
    from_id int,
    relation text,
    to_type text,
    to_id text,
    metadata jsonb
);
```

```sql
insert into
    triples(from_type, from_id, relation, to_type, to_id, metadata) values
    ('document', 1, 'comment', 'comment', 1, '{}'::jsonb);
```

### Schema store

Allow users to store schemata of their entities.

```sql
create table schemata (
    schema jsonb,
)
```

## Bring your own identities

Ent does not manage identities.  It's up to the user to manage identities.  The
only think Ent expects is that user requests come with a jwt that identifies
the user.

By default, Ent will get the user id from the jwt (`sub` claim) and associate
all edges & objects create by that user with that id.

Ent validates the jwt with the public key of the issuer, but does not store or
refresh tokens for the user, it's up to the client to perform that.
