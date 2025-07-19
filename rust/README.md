# Redis-Lite In Rust

This is a redis-lite implementation in
[rust](https://www.rust-lang.org/).

## Project Layout

```shell
.
├── README.md
└── redis-lite
    ├── Cargo.lock
    ├── Cargo.toml
    ├── src
    │   ├── main.rs
    │   ├── persistence
    │   │   ├── aof.rs
    │   │   └── mod.rs
    │   ├── resp
    │   │   ├── mod.rs
    │   │   └── resp.rs
    │   └── store
    │       ├── db.rs
    │       └── mod.rs
```

## Running

You can run this redis-lite server via `cargo`:

```shell
cd redis-lite && cargo run
```

In another terminal, you can connect via `redis-cli`:

```shell
$ redis-cli
redis-cli
127.0.0.1:6379> ping hi
"hi"
127.0.0.1:6379>
```

## Demo

<video src=./img/demo-rust.mov />
