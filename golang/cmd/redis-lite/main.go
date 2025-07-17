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
	flag.StringVar(&aofPathFlag, "aofPath", "/tmp/data", "Path on disk to create or read an AOF file.")
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
