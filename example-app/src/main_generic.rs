rz_embed::include_as_compressed!("src/some-data", module_name = "some_data");

fn main() {
    println!(
        "ipsum.md:\n{}",
        String::from_utf8_lossy(&some_data::IPSUM_MD)
    );
}
