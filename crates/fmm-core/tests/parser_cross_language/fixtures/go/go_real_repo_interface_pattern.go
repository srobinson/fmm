
package storage

import (
    "context"
    "time"

    "github.com/jackc/pgx/v5/pgxpool"
)

type Store interface {
    Get(ctx context.Context, key string) (string, error)
    Set(ctx context.Context, key string, value string, ttl time.Duration) error
    Delete(ctx context.Context, key string) error
}

type PostgresStore struct {
    pool *pgxpool.Pool
}

func NewPostgresStore(pool *pgxpool.Pool) *PostgresStore {
    return &PostgresStore{pool: pool}
}

type cacheEntry struct {
    value     string
    expiresAt time.Time
}
