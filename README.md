# BMCproxy
This proxy converts TLS 1.0 (removed in Chrome) back into unencrypted http. Run like:
```
cargo run --release 0.0.0.0:8080 YourBMC.lan:443
```

If it redirects you back to https, just edit it into http in the address bar

Also, feel free to hack on this if your BMC requires different settings

Probably should have been written in C but I wanted to test openssl_lite
