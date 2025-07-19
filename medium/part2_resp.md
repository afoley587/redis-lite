# Building Redis-Lite in Go – Part 2: RESP Protocol

In [Part 1](./part1.md), we built a concurrent TCP server in Go that accepts client connections and reads raw data from the wire. In this part, we’ll implement the Redis Serialization Protocol (RESP), which allows our server to understand and respond to client commands like `PING`, `GET`, and `SET`.

---

### What is RESP?

RESP stands for **REdis Serialization Protocol**. It’s a simple text-based format that Redis uses to encode requests and responses between clients and servers.

Here are the supported types in RESP:

| Type              | Prefix | Example                      |
|-------------------|--------|------------------------------|
| Simple Strings    | `+`    | `+OK\r\n`                    |
| Errors            | `-`    | `-ERR unknown command\r\n`   |
| Integers          | `:`    | `:1000\r\n`                  |
| Bulk Strings      | `$`    | `$5\r\nhello\r\n`            |
| Arrays            | `*`    | `*2\r\n$4\r\nPING\r\n$4\r\nPONG\r\n` |
| Null              | `_`    | `_\r\n`                      |

Clients encode their commands using RESP arrays. For example, the command `SET mykey hello` is encoded as:

```
*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nhello\r\n
```

---

### Parsing RESP Requests

Let’s look at the core of our RESP parser in `reader.go`.

```go
type Resp struct {
 Values []RespValue
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

The `Read()` function parses the next value from the buffer:

```go
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

Let’s break down one of these cases — bulk strings:

```go
func (r *Resp) readBulk() (RespValue, error) {
 lengthLine := r.readLine()
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

Arrays are parsed recursively using `readArray()`:

```go
func (r *Resp) readArray() (RespValue, error) {
 lengthLine := r.readLine()
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

---

### Writing RESP Responses

On the other end, we need to serialize `RespValue` structs back to the client using the RESP format. This is handled in `writer.go`:

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

func (rw *RespWriter) WriteOK() error {
 return rw.Write(NewSimpleString("OK"))
}

func (rw *RespWriter) WriteError(msg string) error {
 return rw.Write(NewError(msg))
}
```

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
```

---

### Handling Unknown or Invalid Input

Our RESP parser is designed to fail gracefully if a client sends something malformed:

- If the prefix byte isn’t recognized, it returns an error.
- If the array length doesn’t match, it returns an error.
- If CRLF is missing, parsing fails cleanly.

This keeps the server robust against bad input without panicking.

---

### Why Use RESP?

There are many reasons Redis uses RESP:

- Simple to implement
- Human-readable for debugging
- Compact and fast to parse
- Supports structured data like arrays

By supporting RESP in our Redis-lite implementation, we ensure compatibility with existing Redis clients.

---

### What’s Next

Now that we’ve built a RESP parser and serializer, we can finally start executing real commands like `PING`, `SET`, and `GET`.

In the next post, we’ll:

- Wire up a command dispatcher
- Implement an in-memory key-value store
- Persist data using an append-only log file (AOF)

This is where our Redis-lite server actually becomes useful. See you in Part 3.
