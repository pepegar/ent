#!/bin/bash
set -e

# Wait for postgres
echo "Waiting for postgres..."
until PGPASSWORD=$POSTGRES_PASSWORD psql -h "$DB_HOST" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c '\q'; do
  echo "Postgres is unavailable - sleeping"
  sleep 1
done

echo "Postgres is up - executing migrations"
sqlx migrate run --database-url "postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@$DB_HOST/$POSTGRES_DB"

echo "Starting application"
exec "$@"
