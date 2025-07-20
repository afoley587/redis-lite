.PHONY: $(shell sed -n -e '/^$$/ { n ; /^[^ .\#][^ ]*:/ { s/:.*$$// ; p ; } ; }' $(MAKEFILE_LIST))

help:
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.DEFAULT_GOAL := help

install-pre-commit:  ## Installs pre-commit hooks into the current project
	@pre-commit install

run-golang:  ## Runs the golang redis-lite server
	@cd golang && go run cmd/redis-lite/main.go

run-rust:  ## Runs the rust redis-lite server
	@cd rust/redis-lite && cargo run

run-pre-commit:  ## Runs pre-commit for all files
	@pre-commit run --all-files

clean-rust:  ## Cleans the rust project
	@cd rust/redis-lite && cargo clean

clean: clean-rust ## Cleans each project
