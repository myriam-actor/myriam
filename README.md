# myriam-rs

Local and remote actor model implementation with an API heavily inspired in (but not necessarily equivalent to) [Goblins](https://gitlab.com/spritely/guile-goblins).

```rust
// LocalHandle allows for local, type-safe communication
// UntypedHandle tries to do the same, but requests and responses are
// bag of bytes and have to be {de}serialized
let (local_handle, untyped_handle)
    = remote::spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 3 }).await?;

// create router with a TOR netlayer
let layer =
    TorNetLayer::new_for_service("127.0.0.1.9050", "127.0.0.1:8080", "/tmp/myriam/foo")
        .await?;

let router_handle = Router::with_netlayer(layer, Some(RouterOpts::default())).await?;

// routers handle external access to several attached actors
// we can think of this exposed actor as a capability
// "tor:0139aa9b4d523e1da515ce21a818e579acd005fbd0aea62ef094ac1b845f99e7@someaddress.onion"
let address = router_handle.attach(untyped_handle).await?;

let new_layer =
    TorNetLayer::new_for_service("127.0.0.1.9050", "127.0.0.1:8081", "/tmp/myriam/bar")
        .await?;

let remote_handle
    = RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TorNetLayer>::new(&address, new_layer);
//                     type handle once ^

// use RemoteHandle just like a LocalHandle
let res = remote_handle.send(Message::Task(42)).await?;

// capabilities can be revoked anytime
router_handle.revoke(&address).await?;

tokio::time::sleep(Duration::from_millis(100));

// ...and thus we can't invoke this one anymore
remote_handle.send(Message::Ping).await.unwrap_err();
```

Check out the examples for a more comprehensive demo.

## features

* `remote (default)`: support for remote messaging
* `tcp (default)`: TCP test-only net layer
* `tor (default)`: Tor net layer - requires a running and properly configured Tor router
