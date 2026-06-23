# OfferBot — starter MVP

Легкий starter для MVP на Rust з Axum + Teloxide (webhook). Містить базову структуру модулів і заглушки для DB/Redis.

Швидкий старт:

1. Створити `.env` на основі `.env.example` і заповнити `TELOXIDE_TOKEN`.
2. Запустити:
```bash
cargo run
```

Далі: додати реальні реалізації `db::repo` (rusqlite/sqlx) і `cache::redis` (async redis).
