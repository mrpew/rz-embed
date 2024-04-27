"""
rz-embed

Bundler for embedding website/SPAs for rocket.rs
"""

from dataclasses import dataclass
from pathlib import Path
import os
import shutil
import gzip
import re


def slugify(value):
    value = re.sub(r"[^\w\s-]", "_", value).strip().lower()
    value = re.sub(r"[-\s]+", "_", value)
    if value[0] == "_":
        return value[1:]
    return value


def template_path(name):
    return Path(__file__).parent / "templates" / name


def load_template(name):
    with open(template_path(name)) as inf:
        return inf.read()


SUPPORTED_FILE_TYPES = (
    "css",
    "html",
    "ico",
    "js",
    "json",
    "png",
    "ttf",
)

RETURN_TYPES = {
    "html": "RawHtml<&'static [u8]>",
    "js": "RawJavaScript<&'static [u8]>",
    "css": "RawCss<&'static [u8]>",
    "json": "RawJson<&'static [u8]>",
}

CONTENT_TYPE = {
    "png": "ContentType::PNG",
    "ttf": "ContentType::TTF",
    "ico": "ContentType::Icon",
}


def gather_dir(input_dir: Path):
    for e in input_dir.iterdir():
        if e.is_dir():
            yield from gather_dir(e)
        elif e.is_file():
            yield e


def rocket_get(url):
    return f'#[get("/{url}")]'


@dataclass
class ResourceFile:
    base: Path  # path to the input folder
    source_path: Path  # full path to the file
    auto_html: bool
    auto_index: bool

    def extension(self):
        return self.source_path.name.split(".")[-1]

    def is_supported(self):
        return self.extension() in SUPPORTED_FILE_TYPES

    def handler(self):
        return_type = RETURN_TYPES.get(self.extension(), "BinaryResponse")
        if return_type == "BinaryResponse":
            content_type = CONTENT_TYPE[self.extension()]
            return_statement = f"BinaryResponse(&{self.const_name()}, {content_type})"
        else:
            return_statement = f'{return_type.split("<")[0]}(&{self.const_name()})'
        h = load_template("handler.rs")
        h = h.replace("return_type", return_type)
        h = h.replace("return_statement", return_statement)

        gen = ""
        for i, target in enumerate(self.route_targets()):
            hd = h.replace("serve_route", f"{self.route_name()}_{i}")
            gen += hd.replace("/* route_targets */", target)
            gen += "\n"
        gen += "\n"

        return gen

    def compressed_path(self):
        return Path("rz-embed") / Path(f"{self.slug()}.gz")

    def decompression_routine(self):
        result = load_template("decompress.rs")
        result = result.replace("const_name", self.const_name())
        result = result.replace("compressed_source", str(self.compressed_path()))
        return result

    def route_name(self):
        return f"serve_{self.slug()}"

    def route_handler_list(self):
        name = self.route_name()
        for i, _ in enumerate(self.route_targets()):
            yield f"{name}_{i}"

    def rel_path(self):
        return self.source_path.relative_to(self.base)

    def route_url(self):
        return str(self.rel_path())

    def slug(self):
        return slugify(self.route_url())

    def const_name(self):
        return f"GZ_{self.slug().upper()}"

    def route_targets(self):
        default_url = self.route_url()
        targets = [rocket_get(default_url)]
        if default_url.endswith(".html") and self.auto_html:
            targets.append(rocket_get(default_url[:-5]))
        if (
            default_url.endswith("index.html")
            and self.source_path.parent.absolute() == self.base.absolute()
            and self.auto_index
        ):
            targets.append(rocket_get(""))
        return targets

    def compress(self, dst_dir: Path):
        with open(self.source_path, "rb") as f_in:
            with gzip.open(dst_dir / self.compressed_path(), "wb") as f_out:
                shutil.copyfileobj(f_in, f_out)
