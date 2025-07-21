# Building Redis-Lite in Rust – Part 1: A Concurrent TCP Server

In this series, we’re building a Redis-like server from scratch using Rust.
Our goal is to create something that speaks the Redis protocol,
stores data in memory, and persists to disk using an append-only file (AOF).

In this first part, we’ll walk through setting up a minimal TCP server
that can handle multiple clients at once. This foundational layer gives
us the ability to process incoming commands
(like `PING`, `SET`, and `GET`) in future parts.

---

## Overview

Here’s what we’ll build in Part 1:

- A `TcpListener` that handles incoming TCP connections concurrently.
- A `main.rs` entrypoint that initializes the server and Append-Only File
  (AOF) persistence. AOF is discussed in detail in part 3.
- Basic command dispatching

We’ll also stub in placeholders for persistence and command execution that
will be fleshed out later.

---

## Bootstrapping the Server

Let’s start with the main entrypoint in `src/main.rs`:

```go
mod persistence;
mod resp;
mod store;

mod prelude {
    pub use crate::persistence::*;
    pub use crate::resp::*;
    pub use crate::store::*;
    pub use clap::Parser;
    pub use once_cell::sync::Lazy;
    pub use std::{
        collections::HashMap,
        fs::File,
        io::{BufReader, BufWriter, prelude::*},
        net::{TcpListener, TcpStream},
        sync::{Arc, Mutex, RwLock},
        thread,
        time::Duration,
    };
}

use prelude::*;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(default_value = "0.0.0.0:6379")]
    addr: String,

    #[arg(default_value = "/tmp/aof.log")]
    aof_path: String,
}

fn main() {
    let args = Args::parse();

    let listener = TcpListener::bind(args.addr).unwrap();

    let aof = Arc::new(Mutex::new( // Discussed in part 3
        Aof::new(args.aof_path.as_str()).expect("Failed to open AOF"),
    ));

    let aof_clone = Arc::clone(&aof);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            if let Ok(mut aof) = aof_clone.lock() {
                if let Err(e) = aof.sync() {
                    eprintln!("AOF sync failed: {}", e);
                }
            }
        }
    });

    aof.lock().unwrap().read().expect("Failed to replay AOF");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let _aof = Arc::clone(&aof);

        thread::spawn(|| {
            let _ = handle_connection(stream, _aof); // Discussed below
        });
    }
}
```

This acts as the main entrypoint to our system. It does a few things:

1. Parses CLI flags for a TCP bind address and AOF file path (using `clap`).
1. Initializes the append-only file handler. AOF is discussed further in part 3.
1. Replays the AOF.
1. Begins the syncing process for the AOF.
1. Starts the Redis-lite TCP server.

You'll note that we're wrapping the AOF in an `Arc`
(Atomic Reference Counted pointer) because we're accessing this
instance from multiple threads.
Arc enables multiple threads to own the same Aof instance.
Without Arc, Rust would not allow you to move aof into both the main thread
and the background sync thread (or each client connection thread).

Each client connection will take place in a separate thread so as to not
block other clients from connecting.

You can run it with:

```bash
cargo run 0.0.0.0:6379 /tmp/aof.log
```

---

## Handling Connections

In the above, we created a TCP server and started listening on it.
However, we didn't discuss the actual handling of each client request.

The `handle_connection` function reads RESP-encoded data from the client
and dispatches commands:

```go
fn handle_connection(stream: TcpStream, aof: Arc<Mutex<Aof>>) -> Result<(), std::io::Error> {
    let mut buf_reader = BufReader::new(stream);

    loop {
        let command = read_resp(&mut buf_reader)?; // Discussed in part 2

        let response = handle_resp(&command); // Discussed in part 2

        if !matches!(response, RespValue::Error(_)) {
            aof.lock().unwrap().write(&command)?;
        }

        buf_reader
            .get_mut()
            .write_all(marshal(&response).as_ref())
            .unwrap();
    }
}
```

This function begins tying the server functionality with the rest of the project.

- We allocate a buffered reader to the TCP stream.
- The bytes are read from the buffered stream using the RESP protocol
  (discussed in Part 2).
- The command is handled appropriately (discussed in Part 3).
- The command is written to disk using AOF (discussed in Part 3).
- The result is written back to the client.

This is a basic read-execute-respond loop running in it's own thread.

---

## What's Next

We now have a working server that:

- Accepts multiple clients over TCP
- Reads incoming RESP messages
- Dispatches commands to a handler function
- Persists input to disk using AOF

But right now, we haven’t covered how RESP works and we haven’t
implemented any actual commands yet.
That’s what parts 2 and 3 will focus on.

[Part 2](./part2_resp.md)
will handle the implementation of the
[Redis Serialization Protocol (RESP)](https://redis.io/docs/latest/develop/reference/protocol-spec/)
so our server can understand real Redis commands like:

```txt
SET mykey hello
GET mykey
```

[Part 3](./part3_database.md)
will then handle the actual interactions with the database.

If you’ve ever wanted to peek under the hood of how Redis talks to
clients, stay tuned for the next post.
