FROM rust:latest

WORKDIR /telegpt/

COPY src/ src/
COPY Cargo.* .
COPY boot.sh .
COPY LICENSE .

RUN cargo install --path . 

CMD ["/telegpt/boot.sh"]
