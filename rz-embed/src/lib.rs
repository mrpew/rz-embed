extern crate proc_macro;
use std::path::PathBuf;

use lazy_static::lazy_static;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Expr, Ident, LitStr, Token,
};

use flate2::write::GzEncoder;
use flate2::Compression;
use regex::Regex;
use std::fs::{self, create_dir_all, File};
use std::io::{self, BufReader, BufWriter, Read};
use std::path::Path;
use walkdir::WalkDir;

lazy_static! {
    static ref RE_NON_ALPHANUMERIC: Regex = Regex::new(r"[^\w\s-]").unwrap();
    static ref RE_WHITESPACE_HYPHEN: Regex = Regex::new(r"[-\s]+").unwrap();
    static ref RE_REDUCE_UNDESCORES: Regex = Regex::new(r"_+").unwrap();
}

fn slugify(value: &str) -> String {
    // Replace non-alphanumeric characters with underscores
    let mut value = RE_NON_ALPHANUMERIC.replace_all(value, "_").to_string();
    // Trim and convert to lowercase
    value = value.trim().to_lowercase();
    // Replace whitespace and hyphens with underscores
    value = RE_WHITESPACE_HYPHEN.replace_all(&value, "_").to_string();
    // Reduce multiple underscores to one
    value = RE_REDUCE_UNDESCORES.replace_all(&value, "_").to_string();
    // Remove leading underscore if present
    if value.starts_with('_') {
        value = value[1..].to_string();
    }

    value
}

#[derive(Debug)]
enum ContentType {
    Unknown,
    Png,
    Ttf,
    Ico,
}

impl ContentType {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "png" => Self::Png,
            "ttf" => Self::Ttf,
            "ico" => Self::Ico,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug)]
enum FileType {
    Html,
    JavaScript,
    Css,
    Json,
    Xml,
    Plain,
    Binary(ContentType),
}

impl FileType {
    pub fn from_extension(ext: &Option<String>) -> Self {
        let ext = ext.as_ref().map(|e| e.as_str());
        match ext {
            Some("html") => FileType::Html,
            Some("js") => FileType::JavaScript,
            Some("css") => FileType::Css,
            Some("json") => FileType::Json,
            Some("xml") => FileType::Xml,
            Some("txt") | Some("md") => FileType::Plain,
            //
            Some(other) => FileType::Binary(ContentType::from_extension(other)),
            None => FileType::Binary(ContentType::Unknown),
        }
    }
}

struct ResourceFile {
    pub path: PathBuf,
    pub slug: String,
    pub const_name: String,
    pub file_type: FileType,
}

impl std::fmt::Display for ResourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} -- {:?}", self.path, self.file_type)
    }
}

impl ResourceFile {
    /// Create a ResourceFile from a path relative to the source directory
    pub fn from_path(rel_path: &Path) -> Self {
        let name = rel_path
            .file_name()
            .expect("Failed to get file name")
            .to_str()
            .expect("Failed to convert file name to str");
        let path_str = rel_path.to_str().expect("Failed to convert path to str");
        let extension = name.split('.').last().map(|ext| ext.to_string());
        let file_type = FileType::from_extension(&extension);
        let slug = slugify(path_str);
        let const_name = slug.to_ascii_uppercase();
        Self {
            path: rel_path.to_owned(),
            slug,
            const_name,
            file_type,
        }
    }

    pub fn collect(root_dir: &PathBuf) -> Vec<ResourceFile> {
        let mut result = Vec::new();
        for entry in WalkDir::new(root_dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(root_dir).expect("path error");
                let resource = ResourceFile::from_path(relative_path);
                result.push(resource);
            }
        }
        result
    }
    //
}

fn compress_resource(src: &Path, dst: &Path, r: &ResourceFile) -> (u64, u64) {
    let src = src.join(&r.path);
    let meta_original = fs::metadata(&src).expect("Failed to get file metadata");
    let compressed_path = dst.join(format!("{}.gz", r.slug));

    // Check if the compressed file exists, return early if the original was not
    // modified since compression occured.
    if let Ok(compressed_metadata) = fs::metadata(&compressed_path) {
        if meta_original
            .modified()
            .expect("Failed to get modified time")
            <= compressed_metadata
                .modified()
                .expect("Failed to get modified time")
        {
            let original_sz = meta_original.len();
            let compressed_sz = compressed_metadata.len();
            println!(
                "[~] {} already compressed {} -> {} bytes ({:.2}%)",
                src.display(),
                original_sz,
                compressed_sz,
                calculate_compression_rate(original_sz, compressed_sz)
            );
            return (meta_original.len(), compressed_metadata.len());
        }
    }

    // Open the source file for reading
    let f_in = File::open(&src).expect("Failed to open source file");
    let reader = BufReader::new(f_in);

    // Open the destination file for writing
    let f_out = File::create(&compressed_path).expect("Failed to create compressed file");
    let writer = BufWriter::new(f_out);

    // Create a GzEncoder to compress the data
    let mut encoder = GzEncoder::new(writer, Compression::default());

    // Compress the file
    io::copy(&mut reader.take(u64::MAX), &mut encoder).expect("Read failed");
    encoder.finish().expect("Compression failed");

    let meta_compressed = fs::metadata(compressed_path).expect("Failed to get metadata");
    let original_sz = meta_original.len();
    let compressed_sz = meta_compressed.len();
    println!(
        "[+] {}: {} -> {} bytes ({:.2}%)",
        src.display(),
        original_sz,
        compressed_sz,
        calculate_compression_rate(original_sz, compressed_sz)
    );

    (original_sz, compressed_sz)
}

fn calculate_compression_rate(original_size: u64, compressed_size: u64) -> f64 {
    if original_size == 0 {
        return 0.0;
    }
    let rate = 1.0 - (compressed_size as f64 / original_size as f64);
    rate * 100.0
}

fn compress_resources(src: &Path, gz: &Path, resources: &Vec<ResourceFile>) -> (u64, u64) {
    let mut total_original_sz = 0_u64;
    let mut total_compressed_sz = 0_u64;
    create_dir_all(gz).expect("Failed to create gz directory");
    for r in resources {
        let (orig, reduced) = compress_resource(src, gz, r);
        total_original_sz += orig;
        total_compressed_sz += reduced;
    }
    println!(
        "[*] total: {} -> {} bytes ({:.2}%)",
        total_original_sz,
        total_compressed_sz,
        calculate_compression_rate(total_original_sz, total_compressed_sz)
    );
    (total_original_sz, total_compressed_sz)
}

//

struct InclAsCompressedArgs {
    folder_path: LitStr,
    module_name: syn::Ident,
    rocket: bool,
}

impl Parse for InclAsCompressedArgs {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let folder_path: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut module_name = None;
        let mut rocket = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match ident.to_string().as_str() {
                "module_name" => {
                    let name: LitStr = input.parse()?;
                    module_name = Some(Ident::new(&name.value(), name.span()));
                }
                "rocket" => {
                    let pub_expr: Expr = input.parse()?;
                    if let Expr::Lit(expr_lit) = pub_expr {
                        if let syn::Lit::Bool(lit_bool) = expr_lit.lit {
                            rocket = lit_bool.value;
                        }
                    }
                }
                _ => return Err(syn::Error::new(ident.span(), "Unexpected parameter")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(InclAsCompressedArgs {
            folder_path: folder_path.clone(),
            module_name: module_name.unwrap_or_else(|| Ident::new("embedded", folder_path.span())),
            rocket,
        })
    }
}

fn generate_rocket_code(resources: &Vec<ResourceFile>) -> proc_macro2::TokenStream {
    let mut generated_code = quote! {};
    let mut route_idents = Vec::<syn::Ident>::new();

    if resources
        .iter()
        .any(|r| matches!(r.file_type, FileType::Binary(_)))
    {
        generated_code = quote! {
        #generated_code

        pub struct BinaryResponse(&'static [u8], rocket::http::ContentType);
        impl<'r> rocket::response::Responder<'r, 'static> for BinaryResponse {
            fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'static> {
                rocket::response::Response::build()
                    .header(self.1)
                    .sized_body(self.0.len(), std::io::Cursor::new(self.0))
                    .ok()
            }
        }
                    };
    }

    for res in resources {
        let const_name = syn::Ident::new(&res.const_name, Span::call_site());
        let mut handler_url = String::from("/");
        handler_url.push_str(res.path.to_str().expect("path to_str failed"));
        let handler_name = syn::Ident::new(
            &format!("serve_{}", slugify(&handler_url)),
            Span::call_site(),
        );
        route_idents.push(handler_name.clone());

        let handler_code = match &res.file_type {
            FileType::Html => quote! {
                #[get(#handler_url)]
                pub fn #handler_name() -> rocket::response::content::RawHtml<&'static [u8]> {
                    rocket::response::content::RawHtml(&#const_name)
                }
            },
            FileType::JavaScript => quote! {
                #[get(#handler_url)]
                pub fn #handler_name() -> rocket::response::content::RawJavaScript<&'static [u8]> {
                    rocket::response::content::RawJavaScript(&#const_name)
                }
            },
            FileType::Css => quote! {
                #[get(#handler_url)]
                pub fn #handler_name() -> rocket::response::content::RawCss<&'static [u8]> {
                    rocket::response::content::RawCss(&#const_name)
                }
            },
            FileType::Json => quote! {
                #[get(#handler_url)]
                pub fn #handler_name() -> rocket::response::content::RawJson<&'static [u8]> {
                    rocket::response::content::RawJson(&#const_name)
                }
            },
            FileType::Xml => quote! {
                #[get(#handler_url)]
                pub fn #handler_name() -> rocket::response::content::RawXml<&'static [u8]> {
                    rocket::response::content::RawXml(&#const_name)
                }
            },
            FileType::Plain => quote! {
                #[get(#handler_url)]
                pub fn #handler_name() -> rocket::response::content::RawText<&'static [u8]> {
                    rocket::response::content::RawText(&#const_name)
                }
            },

            FileType::Binary(content_type) => match content_type {
                ContentType::Unknown => quote! {
                    #[get(#handler_url)]
                    pub fn #handler_name() -> BinaryResponse {
                        BinaryResponse(&#const_name, rocket::http::ContentType::Binary)
                    }
                },
                ContentType::Png => quote! {
                    #[get(#handler_url)]
                    pub fn #handler_name() -> BinaryResponse {
                        BinaryResponse(&#const_name, rocket::http::ContentType::PNG)
                    }
                },
                ContentType::Ttf => quote! {
                    #[get(#handler_url)]
                    pub fn #handler_name() -> BinaryResponse {
                        BinaryResponse(&#const_name,  rocket::http::ContentType::TTF)
                    }
                },
                ContentType::Ico => quote! {
                    #[get(#handler_url)]
                    pub fn #handler_name() -> BinaryResponse {
                        BinaryResponse(&#const_name,   rocket::http::ContentType::Icon)
                    }
                },
            },
        };

        generated_code = quote! {
            #generated_code
            #handler_code
        };
        // Add an additional route for Html files without the .html extension
        if matches!(res.file_type, FileType::Html) {
            let mut handler_url = PathBuf::from("/");
            handler_url.push(res.path.to_str().expect("path to_str failed"));
            handler_url.set_extension("");
            let handler_url = handler_url.to_str().unwrap();
            let handler_name = syn::Ident::new(
                &format!("serve_{}", slugify(handler_url)),
                Span::call_site(),
            );
            route_idents.push(handler_name.clone());
            let handler = quote! {
                #[get(#handler_url)]
                pub fn #handler_name() -> rocket::response::content::RawHtml<&'static [u8]> {
                    rocket::response::content::RawHtml(&#const_name)
                }
            };
            generated_code = quote! {
                #generated_code
                #handler
            };
        }
    }
    let routes_collector = quote! {
        pub fn routes() -> Vec<rocket::Route> {
            routes![#(#route_idents),*]
        }
    };

    generated_code = quote! {
        #generated_code
        #routes_collector
    };
    generated_code
}

#[proc_macro]
pub fn include_as_compressed(input: TokenStream) -> TokenStream {
    // Parse input
    let args = parse_macro_input!(input as InclAsCompressedArgs);
    let module_name = args.module_name;
    // Input paths should behave like include_bytes
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let folder_path = Path::new(&manifest_dir).join(args.folder_path.value());
    let folder_str = folder_path
        .to_str()
        .expect("Failed to get str from folder_path");
    let resources = ResourceFile::collect(&folder_path);
    let input_slug = slugify(folder_str);
    // GZ dir should conform to crate conventions
    let gz_dir = PathBuf::from(&manifest_dir)
        .join("target")
        .join("rz-embed")
        .join(input_slug);

    compress_resources(&folder_path, &gz_dir, &resources);

    let mut generated_code = quote! {
        use lazy_static::lazy_static;
        use flate2::read::GzDecoder;
        use std::io::Read;
    };

    // Generate code for each resource
    for res in &resources {
        let name = syn::Ident::new(&res.const_name, Span::call_site());
        let compressed_source_path = gz_dir.join(format!("{}.gz", res.slug));
        let compressed_source_path_str = compressed_source_path.to_str().unwrap();
        let res_code = quote! {
            lazy_static! {
                pub static ref #name: Vec<u8> = {
                    let compressed_data: &[u8] = include_bytes!(#compressed_source_path_str);
                    let mut decoder = GzDecoder::new(compressed_data);
                    let mut decompressed_data = Vec::new();
                    decoder.read_to_end(&mut decompressed_data).unwrap();
                    decompressed_data
                };
            }
        };
        generated_code = quote! {
            #generated_code
            #res_code
        };
    }

    // Function to restore to disk
    let mut store_to_disk_fn_body = quote! {};
    for res in &resources {
        let name = syn::Ident::new(&res.const_name, Span::call_site());
        let path = syn::LitStr::new(&res.path.to_string_lossy(), Span::call_site());
        let parts = quote! {
            {
                let path = dst.join(#path);
                let parent = path.parent().expect("Failed to get parent: {path:?}");
                if !parent.is_dir() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut file_handle = std::fs::File::create(path)?;
                std::io::Write::write_all(&mut file_handle, &#name)?;
            }
        };
        store_to_disk_fn_body = quote! {
            #store_to_disk_fn_body
            #parts
        };
    }
    let store_to_disk_fn = quote! {
        pub fn extract_to_folder(dst: &std::path::Path) -> std::result::Result<(), std::io::Error> {
            #store_to_disk_fn_body
            Ok(())
        }
    };
    generated_code = quote! {
        #generated_code
        #store_to_disk_fn
    };

    if args.rocket {
        let rocket_code = generate_rocket_code(&resources);
        generated_code = quote! {
            #generated_code
            #rocket_code
        };
    }

    let result = quote! {
        mod #module_name {
            #generated_code
        }
    };

    result.into()
}


mod tests {
    #[test]
    fn test_slugify() {
        // just a sanity check - maybe we should further limit this to prevent "uncommon code points"
        assert_eq!(
            super::slugify(
                "f0o/b$r/b🇺🇳z/!\"§$%&()=?`''¹²³¼½¬{[]}\\¸ÜÄäü*':;.,@ł€¶ŧ←↓→øþ¨~»«¢„“”µ·…txt"
            ),
            "f0o_b_r_b_z_üääü_ł_ŧ_øþ_µ_txt"
        );
        assert_eq!(super::slugify("a______b"), "a_b");
    }
}
