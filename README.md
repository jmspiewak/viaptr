# viaptr

An experimental library for packing complex types into pointer-sized fields.

## Examples

```rust,ignore
Compact<Result<Box<A>, Box<B>>> // A pointer to A or B, taking up only one machine word
Compact<(Box<A>, Bits<2>)> // A tagged pointer with two additional bits of information
```

## TODO

- document safety constraints on unsafe traits and functions
- general documentation

## License

Mozilla Public License 2.0
