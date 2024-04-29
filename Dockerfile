FROM python:3.11

COPY . /tmp/build
WORKDIR /tmp/build
RUN pip install . && rm -r /tmp/build

ENTRYPOINT [ "rz-embed" ]