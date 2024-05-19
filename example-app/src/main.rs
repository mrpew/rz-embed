#[macro_use]
extern crate rocket;

rz_embed::include_as_compressed!(
    "src/frontend",
    module_name = "embedded_frontend",
    rocket = true
);

#[get("/")]
fn index() -> rocket::response::content::RawHtml<&'static [u8]> {
    embedded_frontend::serve_index_html()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", embedded_frontend::routes())
        .mount("/", routes![index])
}
