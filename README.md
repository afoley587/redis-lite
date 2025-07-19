# redis-lite

## Overview

This is a repo dedicated to my attempts at building a lite redis server in
multiple languages.
My hopes here is to continue learning new things (protocols, technologies, etc.)
in different languages (Golang, Rust, etc.).

## Current Commands

Each `redis-lite` implementation supports the following commands:

1. [`DEL`](https://redis.io/docs/latest/commands/del/)
1. [`GET`](https://redis.io/docs/latest/commands/get/)
1. [`PING`](https://redis.io/docs/latest/commands/ping/)
1. [`SET`](https://redis.io/docs/latest/commands/set/)

Each implementation also uses the
[append-only file](https://redis.io/docs/latest/operate/oss_and_stack/management/persistence/#append-only-file)
persistence method.

You can use `redis-cli` to connect to each server and try the commands.

```shell
$ redis-cli ping
OK
```

## Current Implementations

1. [golang](./golang/)
1. [rust](./rust/)
