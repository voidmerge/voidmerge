# voidmerge

VoidMerge Open Source Monorepo, containing server and commandline utilites and client libraries.

## Getting Started

### 1) build and install the `vm` commandline utility

```
cargo install --path ./rs/voidmerge
```

The typescript client libraries expect the utility to be in the path.

### 2) install the nodejs dependencies

```
npm install
```

### 3) build and test voidmerge-client

```
npm --workspace ts/voidmerge-client test
```

### 4) run the example

```
npm --workspace ts/example1 start
```

### 5) finally, open up a browser

[http://127.0.0.1:8080](http://127.0.0.1:8080)
