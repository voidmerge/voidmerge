# Installing the `vm` Utility

## Option 1: Install from crates.io

1. First, [Get Rust (https://rustup.rs/)](https://rustup.rs/).
2. Second, Install the `vm` utility with cargo:

```sh
cargo install voidmerge --bin vm
```

Use this command like other local commands:

```sh
vm --help
```

## Option 2: Use a docker image

```sh
docker pull ghcr.io/voidmerge/voidmerge-vm:latest
```

Use this command through docker:

```sh
docker run -it ghcr.io/voidmerge/voidmerge-vm:latest vm --help
```

Note: if you're running 'vm serve' in docker, you'll likely need to add additional options to the command to bind a volume and expose the port of the server.
