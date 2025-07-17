package persistence

import (
	"bufio"
	"fmt"
	"io"
	"log"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/afoley587/redis-lite/resp"
	"github.com/afoley587/redis-lite/store"
)

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

	go a.sync()

	return a, nil
}

func (a *Aof) sync() {
	for {
		a.mu.Lock()
		err := a.file.Sync()

		if err != nil {
			log.Printf("Unable to flush AOF to disk: %v\n", err)
		}
		a.mu.Unlock()
		time.Sleep(a.syncPd)
	}
}

func (a *Aof) Close() error {
	a.mu.Lock()
	defer a.mu.Unlock()
	err := a.file.Close()
	if err != nil {
		return fmt.Errorf("could not close AOF File: %w", err)
	}
	return nil
}

func (a *Aof) Read() error {
	a.mu.Lock()
	defer a.mu.Unlock()

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
			continue // ignore malformed input
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

func (a *Aof) Write(val resp.RespValue) error {
	a.mu.Lock()
	defer a.mu.Unlock()

	_, err := a.file.Write(val.Marshal())

	if err != nil {
		return fmt.Errorf("could not save value to AOF: %w", err)
	}
	return nil
}
