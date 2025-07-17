package store

import (
	"sync"

	"github.com/afoley587/redis-lite/resp"
)

var (
	cache     = make(map[string]resp.RespValue)
	cacheLock = sync.RWMutex{}
)
