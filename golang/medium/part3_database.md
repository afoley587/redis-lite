# Building Redis-Lite in Go – Part 3: Command Handling and AOF Persistence

In
[Part 2](./part2_resp.md),
we implemented RESP — the protocol used by Redis to communicate with clients.
In this final part of the series, we’ll hook up real commands like
`SET`, `GET`, and `DEL`, wire them into an in-memory store, and add
 persistence using an append-only file (AOF).

---

## In-Memory Storage

The in-memory store is just a Go `map[string]RespValue` guarded by a `sync.RWMutex`.
This keeps it safe for concurrent access by multiple client goroutines.

```go
var (
 cache     = make(map[string]resp.RespValue)
 cacheLock = sync.RWMutex{}
)
```

Different go routines can then lock the mutex for reading (`RLock`)
or writing (`Lock`) which we will see in
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

We define this mapping using a simple dictionary:

```go
var Handlers = map[string]func([]resp.RespValue) resp.RespValue{
 "PING": ping,
 "GET":  get,
 "SET":  set,
 "DEL":  del,
}
```

Let’s walk through a each handlers:

### `PING`

Ping will return a `PONG` if no argument is provided.
If an argument is provided, it will return a copy of the argument
as a bulk string.

```go
func ping(args []resp.RespValue) resp.RespValue {
 if len(args) == 0 {
  return resp.NewSimpleString("PONG")
 }

 values := make([]string, 0, len(args))
 for _, arg := range args {
  values = append(values, arg.Bulk)
 }

 return resp.NewBulkString(strings.Join(values, " "))
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

```go
func set(args []resp.RespValue) resp.RespValue {
 if len(args) != 2 {
  return resp.NewError("ERR wrong number of arguments for 'SET'")
 }
 key := strings.TrimSpace(args[0].Bulk)
 value := args[1]

 cacheLock.Lock()
 cache[key] = value
 cacheLock.Unlock()

 return resp.NewSimpleString("OK")
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

```go
func get(args []resp.RespValue) resp.RespValue {
 if len(args) != 1 {
  return resp.NewError("ERR wrong number of arguments for 'GET'")
 }
 key := strings.TrimSpace(args[0].Bulk)

 cacheLock.RLock()
 defer cacheLock.RUnlock()

 if val, ok := cache[key]; ok {
  return val
 }
 return resp.NewNull()
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

```go
func del(args []resp.RespValue) resp.RespValue {
 if len(args) == 0 {
  return resp.NewError("ERR wrong number of arguments for 'DEL'")
 }
 cacheLock.Lock()
 defer cacheLock.Unlock()

 deleted := 0
 for _, arg := range args {
  key := strings.TrimSpace(arg.Bulk)
  if _, ok := cache[key]; ok {
   delete(cache, key)
   deleted++
  }
 }
 return resp.NewInteger(deleted)
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

```go
type Aof struct {
 file   *os.File
 rd     *bufio.Reader
 mu     sync.Mutex
 syncPd time.Duration
}

func NewAof(path string) (*Aof, error) {
 file, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0666)
 if err != nil {
  return nil, fmt.Errorf("could not open AOF File: %w", err)
 }
 a := &Aof{file: file, rd: bufio.NewReader(file), syncPd: time.Second}

 if err := a.Read(); err != nil {
  return nil, fmt.Errorf("failed to restore AOF: %w", err)
 }

 go a.sync()  // discussed below
 return a, nil
}
```

Our AOF needs a few pieces of data:

1. A file to write to.
1. A way to read the file on startup.
1. A mutex to ensure that no one concurrently writes to the file.
1. A sync period. The redis AOF allows you to set the
   [flush interval](https://redis.io/docs/latest/operate/oss_and_stack/management/persistence/#how-durable-is-the-append-only-file).
   In our case, we will use the `everysec` method to flush to our AOF
   once a second.

---

### Writing to the AOF

When a command like `SET` or `DEL` is received,
it’s immediately appended to the file:

```go
func (a *Aof) Write(val resp.RespValue) error {
 a.mu.Lock()
 defer a.mu.Unlock()

 _, err := a.file.Write(val.Marshal())
 if err != nil {
  return fmt.Errorf("could not save value to AOF: %w", err)
 }
 return nil
}
```

This is essentially the same process as writing a
response to the client.
In this case, however, we're writing each command to a file.

As discussed above, we also need to flush this from our buffer
to disk once a second to ensure that the changes are persisted.

```go
func (a *Aof) sync() {
 for {
  a.mu.Lock()
  a.file.Sync()
  a.mu.Unlock()
  time.Sleep(a.syncPd)
 }
}
```

In `NewAof`, we spawn the `sync` as a go routine so
sleeping for a second doesn't block any operations.

---

### Replaying Commands on Startup

So now we have a file with all of the commands saved to disk.
But, if the server boots, we need a way to replay each command
in order to return the state of the database.

```go
func (a *Aof) Read() error {
 data, err := io.ReadAll(a.rd)
 if err != nil {
  return fmt.Errorf("failed to read AOF: %w", err)
 }

 parser := resp.NewResp(data)
 for parser.HasNext() {
  cmd, err := parser.Read()
  if err != nil {
   return fmt.Errorf("error parsing AOF command: %w", err)
  }
  if cmd.Type != resp.RespArray || len(cmd.Array) == 0 {
   continue
  }
  commandName := strings.ToUpper(cmd.Array[0].Bulk)
  args := cmd.Array[1:]

  handler, ok := store.Handlers[commandName]
  if !ok {
   return fmt.Errorf("unknown command in AOF: %w", err)
  }
  handler(args)
 }
 return nil
}
```

In the `Read` function, we read each command in order
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
