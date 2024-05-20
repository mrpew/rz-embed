# rz-embed

**rz-embed** implements a simple proc-macro for embedding directory trees
in rust binaries. All files are gzip compressed at compile (or rather, macro expansion) time.
[lazy_static]() is used to decompress resources at runtime.

I originally wrote this to bundle frontend code (svelte, angular..) as gzip-compressed
data embedded in [rocket.rs](https://rocket.rs/) binaries.
adding `rocket = true` to the macro call will automatically generate the appropriate
handlers, as well as a `routes()` function to easily mount them.

## Usage

```
rz_embed::include_as_compressed!(
    "src/some-data",
    module_name = "some_data", // access files via some_data::FILENAME_SLUG
    rocket = true // generate routes for each file 
);
```

Rocket route URLs are relative to the source directory, i.e.
- `some-data/ipsum.md` -> `/ipsum.md`
- `some-data/foo/ipsum.md` -> `/foo/ipsum.md`

The macro will output compressed files (and the compression rate) to stdout:
```
❯ cargo run --release --bin example-rocket
   Compiling example-app v0.1.0 (rz-embed/example-app)
[~] rz-embed/example-app/src/frontend/index.html already compressed 388 -> 230 bytes (40.72%)
[~] rz-embed/example-app/src/frontend/index.css already compressed 33 -> 56 bytes (-69.70%)
[*] total: 421 -> 286 bytes (32.07%)
[+] rz-embed/example-app/src/some-data/ipsum.txt: 591 -> 207 bytes (64.97%)
[+] rz-embed/example-app/src/some-data/ipsum.md: 606 -> 215 bytes (64.52%)
[~] rz-embed/example-app/src/some-data/ipsum.xml already compressed 648 -> 255 bytes (60.65%)
[*] total: 1845 -> 677 bytes (63.31%)
    Finished release [optimized] target(s) in 5.65s
     Running `target/release/example-rocket`
🚀 Rocket has launched from http://127.0.0.1:8000
```

Generated routes can be collected with `<module-name>::routes()`:
```
>> (serve_index) GET /index
>> (serve_index_css) GET /index.css
>> (serve_index_html) GET /index.html
>> (serve_ipsum_md) GET /data/ipsum.md
>> (serve_ipsum_txt) GET /data/ipsum.txt
>> (serve_ipsum_xml) GET /data/ipsum.xml
```

See [example-app](./example-app/) for [generic](./example-app/src/main_generic.rs) and [rocket](./example-app/src/main_rocket.rs) examples.

### Compression

Compressed files are stored in `target/rz-embed/<input path slug>/`:
```
example-app/target/rz-embed
├── [...]_rz_embed_example_app_src_frontend
│  ├── index_css.gz
│  └── index_html.gz
└── [...]_rz_embed_example_app_src_some_data
   ├── ipsum_md.gz
   ├── ipsum_txt.gz
   └── ipsum_xml.gz
```

Files are only re-compressed if they changed after compression, *when the macro is expanded*.
This is done via file metadata/mtime - currently only tested on Linux/ext4.

# TODOs
- skip compression if it would increase file size (i.e. dont compress small files)
- skip compression for known compressed formats (i.e. jpeg,zip,...)
