FROM rust:1.99-trixie AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime

# The stylesheet and favicon are compiled in, so the binary is the whole app.
COPY --from=builder /app/target/release/challenge /app/challenge

# 8080, not 80: the distroless nonroot user cannot bind a privileged port.
ENV RUST_LOG="info"
ENV HOST="0.0.0.0"
ENV PORT="8080"
EXPOSE 8080

CMD ["/app/challenge"]
