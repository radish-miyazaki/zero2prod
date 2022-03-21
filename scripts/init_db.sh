#!/usr/bin/env bash
set -x
set -eo pipefail

# 依存関係にあるpsql/sqlxがインストールされているかチェック
if ! [ -x "$(command -v psql)" ]; then
  echo >&2 "Error: `psql` is not installed."
  exit 1
fi
if ! [ -x "$(command -v sqlx)" ]; then
  echo >&2 "Error: `sqlx` is not installed."
  echo >&2 "Use:"
  echo >&2 "     cargo install --version=0.5.7 sqlx-cli --no-default-features --features postgres"
  echo >&2 "to install it."
  exit 1
fi

# カスタムユーザがセットされているかチェックし、そうでない場合はデフォルトでpostgresを返す
DB_USER="${POSTGRES_USER:=postgres}"
# カスタムパスワードがセットされているかチェックし、そうでない場合はデフォルトでpasswordを返す
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"
# カスタムデータベース名がセットされているかチェックし、そうでない場合はデフォルトでnewsletterを返す
DB_NAME="${POSTGRES_DB:=newsletter}"
# カスタムポート番号がセットされているかチェックし、そうでない場合はデフォルトで5432を返す
DB_PORT="${POSTGRES_PORT:=5432}"

# Postgresのコンテナが既に起動中の場合、SKIP_DOCKERフラグをオンにすることで
# Dockerの立ち上げをスキップできる
if [[ -z "${SKIP_DOCKER}" ]]
then
  docker run \
    --name zero2prod-db \
    -e POSTGRES_USER=${DB_USER} \
    -e POSTGRES_PASSWORD=${DB_PASSWORD} \
    -e POSTGRES_DB=${DB_NAME} \
    -p "${DB_PORT}":5432 \
    -d postgres \
    postgres -N 1000
fi

# Postgresがコマンドを受け付けるようになるまでPingを送り続ける
export PGPASSWORD="${DB_PASSWORD}"
until psql -h "localhost" -U "${DB_USER}" -p "${DB_PORT}" -d "postgres" -c '\q'; do
  >&2 echo "Postgres is still unavailable - sleeping"
  sleep 1
done

>&2 echo "Postgres is up and running on port ${DB_PORT} - running migrations now!"

export DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}
sqlx database create
sqlx migrate run

>&2 echo "Postgres has been migrated, ready to go!"