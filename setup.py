from setuptools import setup, find_packages

setup(
    name="rz-embed",
    version="1.0",
    description="Helper for embedding website/SPAs in rocket.rs apps",
    url="https://github.com/mrpew/rz-embed",
    author="mr",
    packages=find_packages(),
    include_package_data=True,
    package_data={
        "rz_embed": ["templates/*.rs"],
    },
    entry_points={
        'console_scripts': [
            'rz-embed = rz_embed.__main__:main'
        ]
    },
)
