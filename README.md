# infinigen-rs
A Rust library that supports the creation and serialization of infinite chunked terrain, like that of Minecraft.

Allows for packing of groups of chunk data into regions and automatic loading/unloading. Region file handles are cached, allowing for better I/O performance. Chunks are also automatically compressed using zlib, further reducing I/O and file size.

# Example
Go to `example` and do `cargo run` to run the example.
![Screenshot](/example/scrot.png)

It's experimental and will probably corrupt everything. Use with caution.
