# rz-embed

**rz-embed** bundles your frontend code (svelte, angular..) as gzip-compressed
data embedded in [rocket.rs]() binaries.

> TL;DR  
> `cargo add flate2 lazy_static`  
> `rz-embed --input frontend/build --target-src src`  

## Usage
Assuming your frontend lives in `frontend/build`, and the rust code lives in 
`./src`:

```
rz-embed --input frontend/build --target-src src
```

rz-embed will gzip all supported files in `frontend/build` to `src/rz-embed`,
and will create a `src/rz-embed.rs` file containing the decompression routines
and routes for each file:

```rs

lazy_static! {
    static ref GZ_INDEX_HTML: Vec<u8> = {
        let compressed_data: &[u8] = include_bytes!("rz-embed/index_html.gz");
        let mut decoder = GzDecoder::new(compressed_data);
        let mut decompressed_data = Vec::new();
        decoder.read_to_end(&mut decompressed_data).unwrap();
        decompressed_data
    };
}

#[get("/index.html")]
pub fn serve_index_html_0() -> RawHtml<&'static [u8]> {
    RawHtml(&GZ_INDEX_HTML)
}
#[get("/index")]
pub fn serve_index_html_1() -> RawHtml<&'static [u8]> {
    RawHtml(&GZ_INDEX_HTML)
}
#[get("/")]
pub fn serve_index_html_2() -> RawHtml<&'static [u8]> {
    RawHtml(&GZ_INDEX_HTML)
}
```

Additional routes will be created for `html` files for compatibility 
with frameworks like svelte. If an `index.html` file is detected in the input
directory root, an additional route for the url `/` will be created as well.
This behaviour can be disabled with the `--no-auto-html` and `--no-auto-index` 
flags.

## Requirements

**rz-embed** generates code that utilizes [lazy_static](https://crates.io/crates/lazy_static)
to decompress the embedded resources with [flate2](https://crates.io/crates/flate2).

```
cargo add flate2 lazy_static
```