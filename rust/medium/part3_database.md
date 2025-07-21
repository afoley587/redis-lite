# Building Redis-Lite in Rust – Part 3: Command Handling and AOF Persistence

In
[Part 2](./part2_resp.md),
we implemented RESP — the protocol used by Redis to communicate with clients.
In this final part of the series, we’ll hook up real commands like
`SET`, `GET`, and `DEL`, wire them into an in-memory store, and add
 persistence using an append-only file (AOF).

---

## In-Memory Storage

The in-memory store is just a Rust `HashMap<String, RespValue>` guarded by a `RWLock`.
This keeps it safe for concurrent access by multiple client threads.

```rust
static CACHE: Lazy<RwLock<HashMap<String, RespValue>>> = Lazy::new(|| RwLock::new(HashMap::new()));
```

We can break down this line a little bit:

* The `static CACHE` declares a global variable named `CACHE`
* The `Lazy<RwLock<HashMap<String, RespValue>>>` allows for
  deferred initialization of the static variable.
  The `RwLock` is a read-write lock covering a `HashMap<String, RespValue>`
* The `Lazy::new(|| RwLock::new(HashMap::new()))` creates a new
  `RwLock<HashMap<String, RespValue>>` when the initialization occurs.

Different go threads can then lock the mutex for reading or writing
which we will see in
[the next section](#command-dispatch).

---

## Command Dispatch

Our redis implementation will handle 4 commands:

1. [`DEL`](https://redis.io/docs/latest/commands/del/)
1. [`GET`](https://redis.io/docs/latest/commands/get/)
1. [`PING`](https://redis.io/docs/latest/commands/ping/)
1. [`SET`](https://redis.io/docs/latest/commands/set/)

Each RESP command (like `PING`, `SET`, etc.) will be routed
to a handler function which will perform the corresponding action.

We define this mapping using a simple `match` statement`:

```rust
pub fn handle_resp(command: &RespValue) -> RespValue {
    let arr = match command {
        RespValue::Array(a) => a,
        _ => return RespValue::Error("Only arrays accepted.".to_string()),
    };

    let cmd = match arr.get(0) {
        Some(RespValue::BulkString(s)) => s.to_lowercase(),
        _ => return RespValue::Error("Bulk string command expected".to_string()),
    };

    let args = arr[1..].to_vec();

    match cmd.as_str() {
        "ping" => ping(args),
        "get" => get(args),
        "set" => set(args),
        "del" => del(args),
        _ => RespValue::Error("Invalid command".to_string()),
    }
}
```

Let’s walk through a each handlers:

### `PING`

Ping will return a `PONG` if no argument is provided.
If an argument is provided, it will return a copy of the argument
as a bulk string.

```rust
fn ping(args: Vec<RespValue>) -> RespValue {
    if args.is_empty() {
        RespValue::SimpleString("PONG".to_string())
    } else {
        let mut parts = Vec::with_capacity(args.len());

        for arg in args {
            match arg {
                RespValue::BulkString(s) => parts.push(s),
                _ => return RespValue::Error("Invalid PING argument".to_string()),
            }
        }

        RespValue::BulkString(parts.join(" "))
    }
}
```

If we run the server now, we would see

```shell
% redis-cli
127.0.0.1:6379> ping
PONG
127.0.0.1:6379> ping hello world
"hello world"
127.0.0.1:6379>
```

### `SET`

Set will set a stored key to some value.
If key already holds a value, it will be overwritten.
It returns an OK response if the set was successfully
executed.
Otherwise, it'll return an error.

```rust
fn set(args: Vec<RespValue>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("SET requires key and value".to_string());
    }

    let key = match &args[0] {
        RespValue::BulkString(k) => k.clone(),
        _ => return RespValue::Error("Invalid key for SET".to_string()),
    };

    let val = args[1].clone();
    let mut map = CACHE.write().unwrap();
    map.insert(key, val);

    RespValue::SimpleString("OK".to_string())
}
```

If we run the server now, we should see

```shell
% redis-cli
127.0.0.1:6379> set 1
(error) ERR wrong number of arguments for 'SET'
127.0.0.1:6379> set 1 2
OK
127.0.0.1:6379>
```

### `GET`

Get will get the value of the key.
If the key does not exist nil will be returned.

```rust
fn get(args: Vec<RespValue>) -> RespValue {
    let key = match args.get(0) {
        Some(RespValue::BulkString(k)) => k,
        _ => return RespValue::Error("Missing key for GET".to_string()),
    };

    let map = CACHE.read().unwrap();
    match map.get(key) {
        Some(val) => val.clone(),
        None => RespValue::Null,
    }
}
```

If we run the server now, we should see

```shell
% redis-cli
127.0.0.1:6379> get
(error) ERR wrong number of arguments for 'GET'
127.0.0.1:6379> get 2
(nil)
127.0.0.1:6379> set 2 somevalue
OK
127.0.0.1:6379> get 2
"somevalue"
127.0.0.1:6379>
```

### `DEL`

Del will remove the specified keys.
We will then return the number of keys deleted
by this action.

```rust
fn del(args: Vec<RespValue>) -> RespValue {
    let mut deleted = 0;
    let mut map = CACHE.write().unwrap();

    for arg in args {
        if let RespValue::BulkString(k) = arg {
            if map.remove(&k).is_some() {
                deleted += 1;
            }
        }
    }

    RespValue::Integer(deleted)
}
```

If we run the server now, we should see

```shell
% redis-cli
127.0.0.1:6379> del
(error) ERR wrong number of arguments for 'DEL'
127.0.0.1:6379> del dontexist
(integer) 0
127.0.0.1:6379> set 1 2
OK
127.0.0.1:6379> set 2 3
OK
127.0.0.1:6379> del 1 2
(integer) 2
127.0.0.1:6379>
```

---

## AOF Persistence

To ensure that commands are not lost between restarts,
we will implement a persistence layer using an
[**append-only file** (AOF)](https://redis.io/docs/latest/operate/oss_and_stack/management/persistence/#append-only-file).
This file stores each command as a RESP-encoded array.
If the node crashes, all of the commands are saved to disk.
When it recovers, it can replay each command in order to rebuild
the state of the database.

For example, an AOF might look like:

```txt
*3
$3
set
$4
key1
$6
value1
*3
$3
set
$4
key2
$6
value2
*2
$3
del
$4
key1
```

Which would correspond to these commands:

```shell
% redis-cli
127.0.0.1:6379> set key1 value1
OK
127.0.0.1:6379> set key2 value2
OK
127.0.0.1:6379> del key1
(integer) 1
127.0.0.1:6379>
```

### Initialization

Our `Aof` definition is below:

```rust
pub struct Aof {
    reader: BufReader<File>,
    writer: BufWriter<File>,
    lock: Mutex<()>,
}
```

Our AOF needs a few pieces of data:

1. A file to write to.
1. A way to read the file on startup.
1. A mutex to ensure that no one concurrently writes to the file.

Additionally, the redis AOF allows you to set the
[flush interval](https://redis.io/docs/latest/operate/oss_and_stack/management/persistence/#how-durable-is-the-append-only-file).
In our case, we will use the `everysec` method to flush to our AOF
once a second.

---

### Writing to the AOF

When a command like `SET` or `DEL` is received,
it’s immediately appended to the file:

```rust
pub fn write(&mut self, val: &RespValue) -> std::io::Result<()> {
   let _lock = self.lock.lock().unwrap();
   let bytes = marshal(val);
   self.writer.write_all(&bytes)?;
   Ok(())
}
```

This is essentially the same process as writing a
response to the client.
In this case, however, we're writing each command to a file.

As discussed above, we also need to flush this from our buffer
to disk once a second to ensure that the changes are persisted.

```rust
pub fn sync(&mut self) -> std::io::Result<()> {
   let _lock = self.lock.lock().unwrap();
   self.writer.flush()?;
   Ok(())
}
```

---

### Replaying Commands on Startup

So now we have a file with all of the commands saved to disk.
But, if the server boots, we need a way to replay each command
in order to return the state of the database.

```rust
pub fn read(&mut self) -> std::io::Result<()> {
   let _lock = self.lock.lock().unwrap();
   loop {
      match read_resp(&mut self.reader) {
         Ok(command) => {
            println!("Replaying command: {:?}", command);
            let _ = handle_resp(&command);
         }
         Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
         Err(e) => return Err(e),
      }
   }
   Ok(())
}
```

In the `read` function, we read each command in order
and re-handle them.
If there are unknown commands in our AOF, we make sure
to report that to the logs because that could mean
there was AOF corruption.

---

## Final Thoughts

At this point, we’ve built a functional Redis clone that:

* Accepts concurrent TCP clients
* Parses and serializes RESP
* Supports basic commands like `PING`, `GET`, `SET`, and `DEL`
* Persists all write operations to an AOF file
* Replays the AOF to restore state at startup

There’s still room for improvement — things like expiration,
pub/sub, and eviction — but this foundation gave me a better
understanding of how Redis works under the hood.

---

Thanks for following along.
If you build something on top of this, or optimize it further,
I’d love to hear about it.
