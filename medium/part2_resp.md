# Building Redis-Lite in Go – Part 2: RESP Protocol

In
[Part 1](./part1_intro.md),
we built a concurrent TCP server in Go that accepts client connections
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

```go
package resp

import "strconv"

const (
 RespString  = '+'
 RespError   = '-'
 RespInteger = ':'
 RespBulk    = '$'
 RespArray   = '*'
 RespNull    = '_'
)

type RespValue struct {
 Type    byte
 String  string
 Integer int
 Bulk    string
 Array   []RespValue
}

// Factory methods for constructing RESP values
func NewSimpleString(s string) RespValue {
 return RespValue{Type: RespString, String: s}
}

func NewBulkString(b string) RespValue {
 return RespValue{Type: RespBulk, Bulk: b}
}

func NewInteger(i int) RespValue {
 return RespValue{Type: RespInteger, Integer: i}
}

func NewArray(arr []RespValue) RespValue {
 return RespValue{Type: RespArray, Array: arr}
}

func NewNull() RespValue {
 return RespValue{Type: RespNull}
}

func NewError(msg string) RespValue {
 return RespValue{Type: RespError, String: msg}
}
```

Our `RespValue` structs are pretty simple.
They're a struct which:

1. Knows what type they are
1. Can have a string, integer, bulk string, or array value depending
   on what type they are.

Knowing which type we are will be extremely helpful when we start
discussing
[writing responses to the client](#writing-resp-responses).

The factory methods are pretty self-explanatory, so we can omit
discussing those.
Let’s look at the core of our RESP parser in `reader.go`.

```go
type Resp struct {
 Values []RespValue // Parsed top-level values
 curr   int
 buf    []byte
}

func NewResp(buf []byte) *Resp {
 return &Resp{
  buf:    buf,
  Values: make([]RespValue, 0),
 }
}
```

Our `Resp` struct needs to know a few things:

1. The serialized values it has found thus far.
1. The current pointer in its byte stream.
1. The buffer from the byte stream.

Just like in
[part 1](./part1_intro.md)
we provide an interface to create a new object as some of
the attributes are unexported.

This object needs to be able to read from its buffer and
turn the results into `RespValue` objects.

The `Read()` function will handle this for us:

```go
func (r *Resp) readLine() []byte {
 start := r.curr
 for {
  if r.curr+1 >= len(r.buf) || (r.buf[r.curr] == '\r' && r.buf[r.curr+1] == '\n') {
   break
  }
  r.curr++
 }
 end := r.curr
 r.curr += 2 // skip CRLF
 return r.buf[start:end]
}

func (r *Resp) readByte() byte {
 b := r.buf[r.curr]
 r.curr++
 return b
}

func (r *Resp) Read() (RespValue, error) {
 if r.curr >= len(r.buf) {
  return RespValue{}, fmt.Errorf("empty buffer")
 }

 switch r.readByte() {
 case RespString:
  return r.readSimpleString(), nil
 case RespError:
  return r.readError(), nil
 case RespInteger:
  return r.readInteger()
 case RespBulk:
  return r.readBulk()
 case RespArray:
  return r.readArray()
 case RespNull:
  return NewNull(), nil
 default:
  return RespValue{}, fmt.Errorf("unknown RESP type at byte: %d", r.curr-1)
 }
}
```

As noted above, the datatype is dictated by the first byte of the data stream.
We use the `readByte` function to read the first byte from the stream and
advance our internal pointer.
Similarly, `readLine` is a helper function which reads up to the CLRF
and advances our pointer accordingly.

We can then read each different type accordingly.

Let’s break down one of these cases — bulk strings:

```go
func (r *Resp) readBulk() (RespValue, error) {
 lengthLine := r.readLine() // Read the string length and advance pointer to next line
 length, err := strconv.Atoi(string(lengthLine))
 if err != nil {
  return RespValue{}, fmt.Errorf("invalid bulk string length: %v", err)
 }

 if length == -1 {
  return NewNull(), nil
 }

 start := r.curr
 end := start + length

 if end > len(r.buf) {
  return RespValue{}, fmt.Errorf("bulk string out of bounds")
 }

 bulk := string(r.buf[start:end])
 r.curr = end + 2 // skip CRLF

 return NewBulkString(bulk), nil
}
```

In the above, we will read the entire bulk string line.
If our input was `$4\r\Alex\r\n`, we would have read `$` in the
`Read` function via `readByte`.
Then we would have read `4\r\n` via `lengthLine := r.readLine()` in `readBulk`.
Finally, we read the `Alex\r\n` via the rest of the `readBulk` function.

Arrays are a bit more complicated.
However, arrays are how the `redis-cli` passes commands to the server
so they're very important!
Arrays are parsed recursively using `readArray()`:

```go
func (r *Resp) readArray() (RespValue, error) {
 lengthLine := r.readLine() // Read the array length and advance pointer to next line
 length, err := strconv.Atoi(string(lengthLine))
 if err != nil {
  return RespValue{}, fmt.Errorf("invalid array length: %v", err)
 }

 if length == -1 {
  return NewNull(), nil
 }

 values := make([]RespValue, 0, length)
 for i := 0; i < length; i++ {
  val, err := r.Read()
  if err != nil {
   return RespValue{}, fmt.Errorf("array item %d: %v", i, err)
  }
  values = append(values, val)
 }

 return NewArray(values), nil
}
```

As before, we read the array length first via `r.readLine()`.
We then run `Read` again for each item in the array.
This will ensure that each value gets read properly using our same
built-in mechanisms.
Finally, we return a new `RespValue` with it's array of values filled out.

---

## Writing RESP Responses

On the other end, we need to serialize `RespValue` structs
back to the client using the RESP format.
This is handled in `writer.go`:

```go
type RespWriter struct {
 writer io.Writer
}

func NewRespWriter(w io.Writer) *RespWriter {
 return &RespWriter{writer: w}
}

func (rw *RespWriter) Write(value RespValue) error {
 _, err := rw.writer.Write(value.Marshal())
 return err
}
```

The writer will internally keep track of an `io.Writer`,
such as a TCP stream, file, etc., and will marshal
values to the stream.

Each `RespValue` knows how to marshal itself into bytes:

```go
func (rv RespValue) Marshal() []byte {
 switch rv.Type {
 case RespString:
  return marshalSimpleString(rv.String)
 case RespError:
  return marshalError(rv.String)
 case RespInteger:
  return marshalInteger(rv.Integer)
 case RespBulk:
  return marshalBulk(rv.Bulk)
 case RespArray:
  return marshalArray(rv.Array)
 case RespNull:
  return marshalNull()
 default:
  return []byte{}
 }
}
```

For example:

```go
func marshalBulk(b string) []byte {
 return append(
  append(
   append([]byte{RespBulk}, []byte(strconv.Itoa(len(b)))...),
   '\r', '\n'),
  append([]byte(b), '\r', '\n')...,
 )
}

func marshalArray(arr []RespValue) []byte {
 out := []byte{RespArray}
 out = append(out, []byte(strconv.Itoa(len(arr)))...)
 out = append(out, '\r', '\n')

 for _, v := range arr {
  out = append(out, v.Marshal()...)
 }
 return out
}
```

These marshal functions should look a lot like how we read RESP values
but, just in reverse.

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
