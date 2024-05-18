use anyhow::{anyhow, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use regex::Regex;
use std::fs::{create_dir_all, File};
use std::io::{self, BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const BINARY_RESPONSE_DECL: &str = "
pub struct BinaryResponse(&'static [u8], rocket::http::ContentType);

impl<'r> rocket::response::Responder<'r, 'static> for BinaryResponse {
    fn respond_to(self, _: &'r Request<'_>) -> rocket::response::Result<'static> {
        rocket::response::Response::build()
            .header(self.1)
            .sized_body(self.0.len(), Cursor::new(self.0))
            .ok()
    }
}
";

pub fn decompression_routine(const_name: &str, compressed_source: &PathBuf) -> String {
    format!(
        "
lazy_static! {{
    static ref {const_name}: Vec<u8> = {{
        let compressed_data: &[u8] = include_bytes!({compressed_source:?});
        let mut decoder = GzDecoder::new(compressed_data);
        let mut decompressed_data = Vec::new();
        decoder.read_to_end(&mut decompressed_data).unwrap();
        decompressed_data
    }};
}}"
    )
}

pub struct HandlerDecl {
    pub route_fn: String,
    pub handler_code: String,
}
pub fn route_function_decl(
    url: &PathBuf,
    return_type: &str,
    return_statement: &str,
) -> HandlerDecl {
    let route_slug = slugify(&url.to_string_lossy());
    let handler_code = format!(
        "
#[get({url:?})]
pub fn serve_{route_slug}() -> {return_type} {{
    {return_statement}
}}
"
    );
    HandlerDecl {
        route_fn: route_slug,
        handler_code,
    }
}

fn routes_collector(routes: Vec<String>) -> String {
    let indent = " ".repeat(4);
    let joined_routes = routes.join(&format!(",\n{}", indent.repeat(2)));
    let route_list = format!("\n{}{}\n{}", indent.repeat(2), joined_routes, indent);
    format!(
        "
pub fn routes() -> Vec<Route> {{
    routes![{route_list}]
}}
"
    )
}

fn slugify(value: &str) -> String {
    // Create regex patterns
    let re_non_alphanumeric = Regex::new(r"[^\w\s-]").unwrap();
    let re_whitespace_hyphen = Regex::new(r"[-\s]+").unwrap();

    // Replace non-alphanumeric characters with underscores
    let mut value = re_non_alphanumeric.replace_all(value, "_").to_string();
    // Trim and convert to lowercase
    value = value.trim().to_lowercase();
    // Replace whitespace and hyphens with underscores
    value = re_whitespace_hyphen.replace_all(&value, "_").to_string();

    // Remove leading underscore if present
    if value.starts_with('_') {
        value = value[1..].to_string();
    }

    value
}

#[derive(Debug)]
pub enum ContentType {
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

    pub fn rocket_type(&self) -> &str {
        match self {
            ContentType::Unknown => todo!(),
            ContentType::Png => "ContentType::PNG",
            ContentType::Ttf => "ContentType::TTF",
            ContentType::Ico => "ContentType::Icon",
        }
    }
}

#[derive(Debug)]
pub enum FileType {
    Html,
    JavaScript,
    Css,
    Json,
    Binary(ContentType),
}

impl FileType {
    pub fn from_extension(ext: &Option<String>) -> Self {
        let ext = match ext {
            Some(e) => Some(e.as_str()),
            None => None,
        };
        match ext {
            Some("html") => FileType::Html,
            Some("js") => FileType::JavaScript,
            Some("css") => FileType::Css,
            Some("json") => FileType::Json,
            //
            Some(other) => FileType::Binary(ContentType::from_extension(other)),
            None => FileType::Binary(ContentType::Unknown),
        }
    }

    pub fn rocket_return_type(&self) -> &str {
        match self {
            FileType::Html => "rocket::response::content::RawHtml<&'static [u8]>",
            FileType::JavaScript => "rocket::response::content::RawJavaScript<&'static [u8]>",
            FileType::Css => "rocket::response::content::RawCss<&'static [u8]>",
            FileType::Json => "rocket::response::content::RawJson<&'static [u8]>",
            FileType::Binary(_) => "BinaryResponse",
        }
    }

    pub fn return_statement(&self, const_name: &str) -> String {
        match self {
            FileType::Html => format!("RawHtml({const_name})"),
            FileType::JavaScript => format!("RawJavaScript({const_name})"),
            FileType::Css => format!("RawCss({const_name})"),
            FileType::Json => format!("RawJson({const_name})"),
            FileType::Binary(content_type) => format!(
                "BinaryResponse({const_name},{})",
                content_type.rocket_type()
            ),
        }
    }
}

pub struct ResourceFile {
    path: PathBuf,
    pub slug: String,
    pub name: String,
    pub extension: Option<String>,
    pub file_type: FileType,
}

impl std::fmt::Display for ResourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} -- {:?}", self.path, self.file_type)
    }
}

impl ResourceFile {
    /// Create a ResourceFile from a path relative to the source directory
    pub fn from_path(rel_path: &Path) -> Result<Self> {
        let name = rel_path
            .file_name()
            .ok_or(anyhow!("Failed to get file name"))?
            .to_str()
            .ok_or(anyhow!("Failed to convert file name to str"))?;
        let path_str = rel_path
            .to_str()
            .ok_or(anyhow!("Failed to convert path to str"))?;
        let extension = name
            .split('.')
            .last()
            .map_or(None, |ext| Some(ext.to_string()));
        let file_type = FileType::from_extension(&extension);
        Ok(Self {
            path: rel_path.to_owned(),
            slug: slugify(path_str),
            name: name.to_string(),
            extension,
            file_type,
        })
    }

    pub fn collect(root_dir: &PathBuf) -> Vec<ResourceFile> {
        let mut result = Vec::new();
        for entry in WalkDir::new(&root_dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                if let Ok(relative_path) = path.strip_prefix(&root_dir) {
                    match ResourceFile::from_path(relative_path) {
                        Ok(resource) => {
                            result.push(resource);
                        }
                        Err(err) => {
                            log::warn!("Skipping {relative_path:?}: {err}");
                        }
                    }
                }
            }
        }
        result
    }
    //

    fn return_type(&self) -> &str {
        self.file_type.rocket_return_type()
    }

    fn return_statement(&self) -> String {
        self.file_type.return_statement(&self.slug)
    }
}

fn gz_dir(root: &PathBuf) -> PathBuf {
    let mut d = root.clone();
    d.push("rz-embed");
    d
}

fn compress_resource(src: &PathBuf, dst: &PathBuf, r: &ResourceFile) -> Result<(u64, u64)> {
    let mut src = src.clone();
    src.push(&r.path);

    // Open the source file for reading
    let f_in = File::open(&src)?;
    let reader = BufReader::new(f_in);

    // Create the destination path
    let mut compressed_path = dst.clone();
    compressed_path.push(format!("{}.gz", r.slug));

    // Open the destination file for writing
    let f_out = File::create(&compressed_path)?;
    let writer = BufWriter::new(f_out);

    // Create a GzEncoder to compress the data
    let mut encoder = GzEncoder::new(writer, Compression::default());

    // Copy the data from the source file to the compressed file
    io::copy(&mut reader.take(u64::MAX), &mut encoder)?;
    // Finish the compression process
    encoder.finish()?;

    let meta_compressed = std::fs::metadata(compressed_path)?;
    let meta_original = std::fs::metadata(src)?;
    let original_size = meta_original.len();
    let compressed_size = meta_compressed.len();

    Ok((original_size, compressed_size))
}

pub fn calculate_compression_rate(original_size: u64, compressed_size: u64) -> f64 {
    if original_size == 0 {
        return 0.0;
    }
    let rate = 1.0 - (compressed_size as f64 / original_size as f64);
    rate * 100.0
}

pub fn compress_resources(
    src: &PathBuf,
    dst: &PathBuf,
    resources: Vec<ResourceFile>,
) -> Result<(u64, u64)> {
    let mut total = 0_u64;
    let mut compressed = 0_u64;
    let gz = gz_dir(dst);
    create_dir_all(&gz)?;
    for r in resources {
        let (orig, reduced) = compress_resource(&src, &gz, &r)?;
        let rate = calculate_compression_rate(orig, reduced);
        log::debug!("{:?} {orig} -> {reduced} ({rate:.2})", r.path);
        total += orig;
        compressed += reduced;
    }
    Ok((total, compressed))
}

pub fn generate_code(target_dir: &PathBuf, resources: &Vec<ResourceFile>) -> Result<String> {
    let mut code = String::from(
        "use lazy_static::lazy_static;
use flate2::read::GzDecoder;
",
    );
    let gz_dir = gz_dir(&target_dir);
    if resources
        .iter()
        .any(|r| matches!(r.file_type, FileType::Binary(_)))
    {
        code.push_str(BINARY_RESPONSE_DECL);
    }

    let mut const_decls = String::new();
    let mut handlers = String::new();
    let mut routes = Vec::<String>::new();
    for r in resources {
        let mut compressed_rel = gz_dir.clone().strip_prefix(&target_dir)?.to_owned();
        compressed_rel.push(format!("{}.gz", r.slug));
        let decl = decompression_routine(&r.slug, &compressed_rel);
        const_decls.push_str(&decl);

        let decl = route_function_decl(&r.path, r.return_type(), &r.return_statement());
        handlers.push_str(&decl.handler_code);
        routes.push(decl.route_fn);

        if matches!(r.file_type, FileType::Html) {
            let mut path_without_ext = r.path.clone();
            path_without_ext.set_extension("");
            let decl =
                route_function_decl(&path_without_ext, r.return_type(), &r.return_statement());
            handlers.push_str(&decl.handler_code);
            routes.push(decl.route_fn);
        }
    }

    code.push_str(&const_decls);
    code.push_str(&handlers);
    code.push_str(&routes_collector(routes));

    Ok(code)
}
