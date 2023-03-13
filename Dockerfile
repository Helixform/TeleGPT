FROM rust:latest AS BUILD
WORKDIR /usr/src/telegpt

COPY src/ src/
COPY Cargo.* .
RUN cargo install --path . 

FROM debian:latest
WORKDIR /telegpt/

COPY --from=BUILD /usr/src/telegpt/target/release/telegpt /telegpt/telegpt
COPY LICENSE .
COPY boot.sh .

# install dependencies
#   libsqlite3.so.0: cannot open shared object file
#   failed:../ssl/statem/statem_clnt.c:1914
RUN apt update -y && \ 
    apt install -y --no-install-recommends sqlite3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

CMD ["/telegpt/boot.sh"]
