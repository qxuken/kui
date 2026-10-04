# kui-node

The Node.js addon behind the `@qxuken/kui` npm package, built with napi-rs.

This crate is not published to crates.io. It exposes kui's headless core (`Ctx`) and windowed surface (`KuiWindow`) to JavaScript, and ships as a prebuilt library inside the npm package, whose JSX runtime and app loop are the API users see. A frame comes in as one flat instruction stream the package's encoder produces, lowered into the core in a single crossing; events go back as plain JSON.

## Build from the repository

```
cargo build -p kui-node --release
```

Then point the package at the result (`target/release/libkui_node.so`, `.dylib` or `kui_node.dll`):

```
KUI_NODE_LIB=/path/to/kui/target/release/libkui_node.so node app.mjs
```

The npm package picks a bundled prebuild for linux-x64, linux-arm64, darwin-arm64, darwin-x64 and win32-x64 by itself; `KUI_NODE_LIB` is for every other platform and for running against a local build.

## Documentation

- The npm package and its install instructions: `packages/kui/README.md` in the repository
- Book: <https://kui-book.qxuken.dev>
- Repository: <https://github.com/qxuken/kui>

## License

MIT
