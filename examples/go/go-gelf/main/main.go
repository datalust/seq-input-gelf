package main

import (
    "gopkg.in/Graylog2/go-gelf.v2/gelf"
    "log"
)

func main() {
    var logger, err = gelf.NewUDPWriter("127.0.0.1:12201")

    if err != nil {
        log.Fatalf("Failed to create UDP writer: %s", err)
    }

    // Don't prefix messages with a redundant timestamp etc.
    log.SetFlags(0)
    
    log.SetOutput(logger)

    log.Print("Hello, from go!")
}
