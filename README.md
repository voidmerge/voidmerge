# voidmerge

![Crates.io License](https://img.shields.io/crates/l/voidmerge)
[![Crates.io Version](https://img.shields.io/crates/v/voidmerge)](https://crates.io/crates/voidmerge)
[![NPM Version](https://img.shields.io/npm/v/%40voidmerge%2Fvoidmerge-client)](https://www.npmjs.com/package/@voidmerge/voidmerge-client)

[https://voidmerge.com](https://voidmerge.com)

[VoidMerge Handbook](https://voidmerge.com/book)

VoidMerge Open Source Monorepo, containing server and commandline utilites and client libraries.

## Getting Started

### 1) build and install the `vm` commandline utility

```
cargo install --path ./rs/voidmerge --bin vm
```

The typescript test expect the utility to be in the path.

### 2) install the nodejs dependencies

```
npm install
```

### 3) run the test suite

```
npm test
```

### 4) run the example

```
npm --workspace ts/example1 start
```

### 5) finally, open up a browser

[http://127.0.0.1:8080](http://127.0.0.1:8080)
