# Building Redis-Lite in Rust – Part 2: RESP Protocol

In
[Part 1](./part1_intro.md),
we built a concurrent TCP server in Rust that accepts client connections
and reads raw data from the wire.
In this part, we’ll implement the
[Redis Serialization Protocol (RESP)](https://redis.io/docs/latest/develop/reference/protocol-spec/),
which allows our server to serialize the bytes from the client into a readable
format which correspond to commands like `PING`, `GET`, and `SET`.

---

## What is RESP?

RESP stands for **REdis Serialization Protocol**.
It’s a simple text-based format that Redis uses to encode
requests and responses between clients and servers.

As noted above, RESP is very simple.
A server knows the type of the RESP command by the first byte of data.
The server knows when a new line starts or the command is finished via
the `\r\n` separator (CLRF).

RESP supports a lot of
[different types](https://redis.io/docs/latest/develop/reference/protocol-spec/#resp-protocol-description).
However, our lite implementation will only handle the ones below for
simplicity:

| Type              | Prefix | Example                              |
|-------------------|--------|--------------------------------------|
| Simple Strings    | `+`    | `+OK\r\n`                            |
| Errors            | `-`    | `-ERR unknown command\r\n`           |
| Integers          | `:`    | `:1000\r\n`                          |
| Bulk Strings      | `$`    | `$5\r\nhello\r\n`                    |
| Arrays            | `*`    | `*2\r\n$4\r\nPING\r\n$4\r\nPONG\r\n` |
| Null              | `_`    | `_\r\n`                              |

Clients encode their commands using RESP arrays.
For example, the command `SET mykey hello` is encoded as:

```txt
*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nhello\r\n
```

Let's break down the above command:

* `*3` tells the server that the incoming request is an array with 3 items
* `\r\n` is the protocol's terminator, which always separates its parts.
* `$3` tells the server that the first item is a bulk string with 3 characters.
* `\r\n` is another separator.
* `SET` is the 3 characters mentioned above.
* `\r\n` is another separator.
* `$5` tells the server that the second item is a bulk string with 5
  characters.
* `\r\n` is another separator.
* `nmykey` is the 3 characters mentioned above.
* etc.

---

## Parsing RESP Requests

Now that we know what RESP is, we can start to produce an object that
can convert bytes from the TCP stream into RESP.

First, let's look at our RESP objects:

```rust
use crate::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i32),
    BulkString(String),
    Array(Vec<RespValue>),
    Null,
}
```

Our `RespValue` enum is pretty simple.
They're a enum which:

1. Knows what type they are (string, integer, etc.).
1. Based on what type they are, we can format requests and responses
   accordingly.

Knowing which type we are will be extremely helpful when we start
discussing
[writing responses to the client](#writing-resp-responses).

Let’s look at the core of our RESP parser in `resp.rs`.
The `read_resp()` function will handle reading the bytes from a TCP stream
and either returning an error or the RESP value:

```rust
pub fn read_resp<R: BufRead>(reader: &mut R) -> Result<RespValue, std::io::Error> {
    let mut line = String::new();

    // Skip empty or whitespace-only lines
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "EOF",
            ));
        }
        if !line.trim().is_empty() {
            break;
        }
    }

    if !line.starts_with('*') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a RESP array",
        ));
    }
    let array_len: usize = line[1..].trim().parse().unwrap();
    let mut elements = Vec::with_capacity(array_len);

    for _ in 0..array_len {
        line.clear();
        reader.read_line(&mut line)?;
        if !line.starts_with('$') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected bulk string",
            ));
        }

        let str_len: usize = line[1..].trim().parse().unwrap();
        let mut buf = vec![0; str_len + 2];
        reader.read_exact(&mut buf)?;

        let s = String::from_utf8_lossy(&buf[..str_len]).to_string();
        elements.push(RespValue::BulkString(s));
    }

    Ok(RespValue::Array(elements))
}
```

We first clear all empty and newlines until we get to some data.
This is especially useful when we begin replaying the AOF.
Note that `redis-cli` will send arrays of bulk strings to the server.
Because of that, we can ensure that the item we received is an array
of bulk strings.

For each item in the array that the `redis-cli` sent over, we have to:

1. Read the string size (`$3` indicates a bulk string of length three)
1. Read that amount of characters into memory
1. Convert those bytes to a bulk string

We can then arrange each bulk string into a serialized `RespValue::Array`
to dispatch commands (discussed in part 3).

We can walk through the earlier example:

```txt
*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nhello\r\n
```

In the above,

* `let array_len: usize = line[1..].trim().parse().unwrap();`
   would read `3\r\n` and then `trim` would remove `\r\n`.
* We would then enter the `for` loop.
  * Iteration 1:
    * `reader.read_line(&mut line)?;` would read `$3\r\n`
    * `let str_len: usize = line[1..].trim().parse().unwrap();`
      would read `3\r\n` and then `trim` would remove `\r\n`.
    * `reader.read_exact(&mut buf)?;` would read the next 3
      characters from the stream (`SET`)
    * `let s = String::from_utf8_lossy(&buf[..str_len]).to_string();`
      would convert the bytes to the word `SET`
  * Iteration 2:
    * `reader.read_line(&mut line)?;` would read `$5\r\n`
    * `let str_len: usize = line[1..].trim().parse().unwrap();`
      would read `5\r\n` and then `trim` would remove `\r\n`.
    * `reader.read_exact(&mut buf)?;` would read the next
      5 characters from the stream (`mykey`)
    * `let s = String::from_utf8_lossy(&buf[..str_len]).to_string();`
      would convert the bytes to the word `mykey`
  * Iteration 3:
    * `reader.read_line(&mut line)?;` would read `$5\r\n`
    * `let str_len: usize = line[1..].trim().parse().unwrap();`
      would read `5\r\n` and then `trim` would remove `\r\n`.
    * `reader.read_exact(&mut buf)?;` would read the next
      5 characters from the stream (`hello`)
    * `let s = String::from_utf8_lossy(&buf[..str_len]).to_string();`
      would convert the bytes to the word `hello`

In the end, we would return an array similar to `[SET, mykey, hello]`.

---

## Writing RESP Responses

On the other end, we need to serialize `RespValue` enums
back to the client using the RESP format.
This is handled in `resp.rs` via the `marshal` function:

```rust
pub fn marshal(value: &RespValue) -> Vec<u8> {
    match value {
        RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
        RespValue::Error(s) => format!("-{}\r\n", s).into_bytes(),
        RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
        RespValue::BulkString(s) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),
        RespValue::Array(arr) => {
            let mut buf = format!("*{}\r\n", arr.len()).into_bytes();
            for item in arr {
                buf.extend(marshal(item));
            }
            buf
        }
        RespValue::Null => b"$-1\r\n".to_vec(),
    }
}
```

This marshal functions should look a lot like how we read RESP values
but, just in reverse.
We are essentially tacking on the corresponding type from the RESP protocol
to the front of the integer, string, error, etc.
In the case of bulk strings and arrays, we also need to tack on the
length of the string or array, respectively.

---

## Handling Unknown or Invalid Input

Our RESP parser is designed to fail gracefully if a client sends something malformed:

* If the prefix byte isn’t recognized, it returns an error.
* If the array length doesn’t match, it returns an error.
* If CRLF is missing, parsing fails cleanly.

This keeps the server robust against bad input without panicking.

---

## What’s Next

Now that we’ve built a RESP parser and serializer,
we can finally start executing real commands like `PING`, `SET`, and `GET`.

In [part 3](./part3_database.md)

* Wire up a command dispatcher
* Implement an in-memory key-value store
* Persist data using an append-only log file (AOF)

This is where our Redis-lite server actually becomes useful. See you in Part 3.
