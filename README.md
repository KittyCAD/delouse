# Using `delouse`

First, edit your `Cargo.toml` to add the `delouse` crate as an optional
dependency, and add a new feature ("`debug`").

```toml
[dependencies]
...
delouse = { version = "0", optional = true }
...

[features]
...
debug = ["dep:delouse"]
...
```

Next, during the startup in your `main` or similar, put in:

```rust
...

async fn main() -> Result<()> {
    #[cfg(feature = "debug")]
    {
        delouse::init().unwrap();
    }

    ...
}

...
```

# Running your program

When building with `cargo build` or running with `cargo run`, add an additional
`--features debug` flag to enable `delouse`.

# Using `delouse`

By default, and due to no toggles existing yet, `delouse` will bind to
`127.0.0.1:7132`. The interface is OpenAPI/JSON based, so you can shave
that yak how you'd like, but I tend to just use cURL. Here's some commands
for bad days:


| What | Command | Platform Restrictions | Notes |
| - | - | - | - |
| Rust Stacktrace    | <code>curl http://localhost:7132/stacktrace/rust &#124; jq -r .stacktrace</code>  | | |
| ELF Information    | <code>curl http://localhost:7132/elf/info &#124; jq .</code>                      | Linux 🐧 | |
| Request a coredump | <code>curl http://localhost:7132/coredump</code>                                  | Linux 🐧 | Process will exit |
| Tokio Stacktrace   | <code>curl http://localhost:7132/stacktrace/tokio &#124; jq -r .stacktrace</code> | Linux 🐧, `tokio_unstable` | This endpoint is *very* flaky. If this locks up `tokio`'s runtime, this will panic the process with the stacktrace. |

# tokio specific notes

A lot of the surface we need is unstable. The following table is a list
of endpoints and required `cfg` directives.

| Endpoint | cfgs |
| - | - |
| `stacktrace/tokio` | `tokio_unstable`, `tokio_taskdump` |

In the author's very humble opinion, the following `.cargo/config.toml`
settings are encouraged, in the absense of overriding convictions or
specific engineering restrictions when running with `tokio`:

```toml
[build]
rustflags = ["--cfg", "tokio_unstable"]

[target.x86_64-unknown-linux-gnu]
rustflags = ["--cfg", "tokio_unstable", "--cfg", "tokio_taskdump"]
```

If this is not possible, `delouse` will gracefully degrade and not
serve any endpoints which can not be run.
