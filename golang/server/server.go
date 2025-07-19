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
