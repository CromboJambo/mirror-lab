set positional-arguments

default:
    @just --list

check:
    cargo check --workspace

build:
    cargo build -p crabjar

run +args='state list':
    cargo run -p crabjar -- {{args}}

test:
    cargo test --workspace

clean:
    cargo clean
