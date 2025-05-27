## Installation

1. Install Rust:

https://rustup.rs/

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Create .env
```bash
cp .env.example .env
```

3. Run docker-compose services
```bash
docker-compose up -d
```

4. Run observability services
```bash
(cd observability; docker-compose up -d)
```

5. Install sqlx-cli:
```bash
cargo install sqlx-cli
```

6. Run the migrations

```bash
sqlx migrate run
```

## Running the Service

1. Run the service:
```bash
cargo run
```
