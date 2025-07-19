package store

import (
	"strings"

	"github.com/afoley587/redis-lite/resp"
)

var Handlers = map[string]func([]resp.RespValue) resp.RespValue{
	"PING": ping,
	"GET":  get,
	"SET":  set,
	"DEL":  del,
}

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
