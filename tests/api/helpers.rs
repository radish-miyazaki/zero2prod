use api::configuration::{get_configuration, DatabaseSettings};
use api::email_client::EmailClient;
use api::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;

// テスト開始時に一度だけ呼ばれる処理
// テスト用の構造化ログを生成しておく
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    // テスト実行時にTEST_LOG=trueがセットされていれば、ログを出力する
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

// INFO: actix_webのテスト用のHelper関数を使えばより簡単に実装できる。
// しかし、FWからIntegrationテストを切り出しておくことで、他のFWに乗り換えたときも
// そのまま使うことができるので、わざわざtokioを用いてアプリケーションを背後で実行している。
// TODO: テスト終了時に、作成したDBインスタンスを削除する処理を追加
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    // ポート番号が固定だと、番号によってはテストが落ちる可能性があるので、空いているポートを見つけて繋ぐようにする
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    // テストでは、本処理のデータベース名とは異なるランダムな名前のデータベースで処理を行う
    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.database.database_name = Uuid::new_v4().to_string();

    let connection_pool = configure_database(&configuration.database).await;

    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address.");

    let timeout = configuration.email_client.timeout();

    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.api_key,
        timeout,
    );

    let server = api::startup::run(listener, connection_pool.clone(), email_client)
        .expect("Failed to bind address");

    // テスト終了時にサーバが落ちるようにする
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool,
    }
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // テスト用の新しいデータベースを作成
    let mut connection = PgConnection::connect(&config.connection_string_without_db())
        .await
        .expect("Failed to connect to Postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    // データベースをテスト用に切り替える
    let connection_pool = PgPool::connect(&config.connection_string())
        .await
        .expect("Failed to connect to Postgres");
    // テーブルをマイグレーションする
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}
