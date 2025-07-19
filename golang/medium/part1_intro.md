# Building Redis-Lite in Go – Part 1: A Concurrent TCP Server

In this series, we’re building a Redis-like server from scratch using Go.
Our goal is to create something that speaks the Redis protocol,
stores data in memory, and persists to disk using an append-only file (AOF).

In this first part, we’ll walk through setting up a minimal TCP server
that can handle multiple clients at once. This foundational layer gives
us the ability to process incoming commands
(like `PING`, `SET`, and `GET`) in future parts.

---

## Overview

Here’s what we’ll build in Part 1:

- A `server` package that handles incoming TCP connections concurrently.
- A `main.go` entrypoint that initializes the server and Append-Only File
  (AOF) persistence. AOF is discussed in detail in part 3.
- Basic command dispatching

We’ll also stub in placeholders for persistence and command execution that
will be fleshed out later.

---

## Bootstrapping the Server

Let’s start with the main entrypoint in `cmd/redis-lite/main.go`:

```go
package main

import (
 "flag"
 "log"

 "github.com/afoley587/redis-lite/persistence"
 "github.com/afoley587/redis-lite/server"
)

var addrFlag string
var aofPathFlag string

func main() {
 flag.StringVar(&addrFlag, "address", ":6379", "Address to bing to.")
 flag.StringVar(&aofPathFlag, "aofPath", "/tmp/aof.log", "Path on disk to create or read an AOF file.")
 flag.Parse()

 aof, err := persistence.NewAof(aofPathFlag)

 if err != nil {
  log.Fatalf("Failed to initialize AOF: %v", err)
 }

 s := server.NewServer(addrFlag, "tcp")
 err = s.ListenAndServe(aof)

 if err != nil {
  log.Fatalf("Failed to start server: %v", err)
 }

}
```

This acts as the main entrypoint to our system. It does a few things:

1. Parses CLI flags for a TCP bind address and AOF file path.
2. Initializes the append-only file handler.
3. Starts the Redis-lite TCP server.

You can run it with:

```bash
go run cmd/redis-lite/main.go --address=":6379" --aofPath="/tmp/aof.log"
```

---

## The `server` Package

In the above, we just create a server object and run the `ListenAndServe`
method on it.
Knowing what the server is doing is an important part of this project.
Let's go into more detail in the `server` package:

```go
package server

import (
 "fmt"
 "io"
 "log"
 "net"
 "strings"

 "github.com/afoley587/redis-lite/persistence"
 "github.com/afoley587/redis-lite/resp"
 "github.com/afoley587/redis-lite/store"
)

type Server struct {
 addr  string
 proto string
}

func NewServer(addr string, proto string) *Server {
 return &Server{addr, proto}
}
```

We first start off by defining a new `Server` struct.
This struct should know:

1. What address to listen on
1. Which protocol to use

The `NewServer` function lets us instantiate the `Server`.
Note that the `addr` and `proto` aren't exported and, therefore,
wouldn't be able to be set from outside of the `server` package.

### Starting the TCP Listener

With a `Server` object, we can now bind to the specified address
with the specified protocol.
That's where `ListenAndServe`, which `main.go` calls, comes in handy.

```go
func (s *Server) ListenAndServe(aof *persistence.Aof) error {
 l, err := net.Listen(s.proto, s.addr)
 if err != nil {
  return fmt.Errorf("couldn't start server: %w", err)
 }
 defer l.Close()

 log.Println("Server listening on :6379")

 for {
  conn, err := l.Accept()
  if err != nil {
   log.Printf("Couldn't accept connection: %v\n", err)
   continue
  }

  go handleConnection(conn, aof)
 }
}
```

This binds to the configured TCP address and accepts connections in a loop.
Each connection is handed off to a new goroutine via `handleConnection`,
which ensures that multiple clients can interact with the server concurrently.

---

### Handling Connections

The `handleConnection` function reads RESP-encoded data from the client
and dispatches commands:

```go
func handleConnection(conn net.Conn, aof *persistence.Aof) {
 defer conn.Close()
 rw := resp.NewRespWriter(conn) // See part 2

 for {
  buf := make([]byte, 1024)
  n, err := conn.Read(buf)
  if err != nil {
   if err == io.EOF {
    log.Printf("Client disconnected: %v", conn.RemoteAddr())
    return
   }
   log.Printf("Read error: %v", err)
   continue
  }

  r := resp.NewResp(buf[:n]) // See part 2
  cmd, err := r.Read()
  if err != nil || len(cmd.Array) == 0 {
   log.Printf("Invalid command: %v", err)
   rw.Write(resp.RespValue{Type: resp.RespError, String: "ERR invalid command"})
   continue
  }

  commandName := strings.ToUpper(cmd.Array[0].Bulk)
  handler, ok := store.Handlers[commandName] // See part 3
  if !ok {
   log.Printf("Unknown command: %s", commandName)
   rw.Write(resp.RespValue{Type: resp.RespError, String: "ERR unknown command"})
   continue
  }

  // Write to AOF before executing command
  if err := aof.Write(cmd); err != nil { // See part 3
   log.Printf("AOF write failed: %v", err)
  }

  response := handler(cmd.Array[1:])
  rw.Write(response) // See part 2
 }
}
```

This function begins tying the server functionality with the rest of the project.

- We allocate a 1KB buffer and read from the TCP stream.
- The bytes are parsed using the RESP protocol (discussed in Part 2).
- The command is looked up in a handler map
  (defined in `store.Handlers`, discussed in Part 3).
- The command is written to disk using AOF (discussed in Part 3).
- The result is written back to the client.

This is a basic read-execute-respond loop.
No threads.
No channels.
Just goroutines and blocking I/O.

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
