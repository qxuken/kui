# A failing test you can debug

The ordinary test passes. The opt-in failure injects a checkout regression:
the HTTP reply says 20% off, but checkout still charges $20 instead of $16.
The failure stays in `fixtures/`, outside normal test discovery.

From the repository root:

```sh
# Normal, passing test.
./target/debug/kui test examples/lua/test-runner-demo

# Run the deliberate failure and inspect its completed recording.
./target/debug/kui test examples/lua/test-runner-demo/fixtures/discount_failure.lua --ui

# Or start at the initial app state and step through it.
./target/debug/kui test examples/lua/test-runner-demo/fixtures/discount_failure.lua --debug
```

Debug pauses with the app visible. Use **Step** to advance an action or
**Resume test** to reach the failure. At the failure, open **Errors**, inspect
**App state** before/after the discount, read the request/reply under
**Messages & I/O**, and read the subtotal and total under **Output**. The
screenshot filmstrip preserves the checkout states. Large state payloads
remain scrollable and can be copied in full.

Change the fixture's `checkout_case(true)` argument to `false` to remove
fault injection and rerun the same test. The normal application path and
normal test suite remain passing.

For an agent:

```sh
./target/debug/kui test examples/lua/test-runner-demo/fixtures/discount_failure.lua --json
./target/debug/kui test report examples/lua/test-runner-demo/test-results --json
./target/debug/kui test output examples/lua/test-runner-demo/test-results
```
