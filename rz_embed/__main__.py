from rz_embed import gather_dir, load_template, ResourceFile
import argparse
from pathlib import Path


def main():
    ap = argparse.ArgumentParser("rz-embed")
    ap.add_argument("--input", type=Path, required=True)
    ap.add_argument("--target-src", type=Path, required=True)
    ap.add_argument("--no-auto-html", action="store_true")
    ap.add_argument("--no-auto-index", action="store_true")
    args = ap.parse_args()

    all_files = gather_dir(args.input)
    resources = []
    for file in all_files:
        r = ResourceFile(args.input, file, not args.no_auto_html, not args.no_auto_index)
        if r.is_supported():
            resources.append(r)

    gen = load_template("header.rs")
    routes = []
    consts = []
    handlers = []

    gz_dir = (args.target_src / "rz-embed")
    gz_dir.mkdir(exist_ok=True)
    with open(gz_dir / '.gitignore','w') as outf:
        outf.write('*.gz')

    for r in resources:
        r.compress(args.target_src)
        #
        consts.append(r.decompression_routine())
        handlers.append(r.handler())
        routes += r.route_handler_list()

    indent = " " * 4
    route_list = f"\n{indent*2}" + f",\n{indent*2}".join(routes) + f"\n{indent}"

    gen = gen.replace("/* route_list */", route_list)
    gen = gen.replace("/* route_handlers */", "\n".join(handlers))
    gen = gen.replace("/* embed */", "\n".join(consts))

    with open(args.target_src / "rz_embed.rs", "w") as outf:
        outf.write(gen)


if __name__ == "__main__":
    main()
