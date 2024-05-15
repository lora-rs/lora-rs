# Examples

## Using these examples outside of this project

The `Cargo.toml` in each of these examples depends on a local path to `lora-phy`, `lorawan`, and `lorawan-device`.

When starting a project locally, you will want to use a cargo version (these example versions here may not be up to date):

```
lora-phy = { version = "3" , features = ["lorawan-radio"] }
lorawan-device = { version = "0.12", default-features = false, features = ["embassy-time", "default-crypto", "defmt"] }
```

Or a git reference:
```
lora-phy = { git = "https://github.com/lora-rs/lora-rs.git", features = ["lorawan-radio"] }
lorawan-device = { git = "https://github.com/lora-rs/lora-rs.git", default-features = false, features = ["embassy-time", "default-crypto", "defmt"] }
```