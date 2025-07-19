# Redis-Lite In GoLang

This is a redis-lite implementation in 
[golang]().

## Project Layout

```shell
.
├── cmd
│   └── redis-lite
│       └── main.go
├── go.mod
├── persistence
│   └── aof.go
├── README.md
├── resp
│   ├── reader.go
│   ├── resp.go
│   └── writer.go
├── server
│   └── server.go
└── store
    ├── db.go
    └── handlers.go
```

## Running

```shell
go run cmd/redis-lite/main.go
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

<video src=./img/demo-go.mov />