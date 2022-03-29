use api::configuration::{get_configuration, DatabaseSettings};
use api::startup::{get_connection_pool, Application};
use api::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::MockServer;

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
    pub email_server: MockServer,
    pub port: u16,
}

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub text: reqwest::Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);

            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");

            // spawn_appで生成したランダムなポート値をセットする
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(&body["html"].as_str().unwrap());
        let text = get_link(&body["text"].as_str().unwrap());

        ConfirmationLinks { html, text }
    }
}

// INFO: actix_webのテスト用のHelper関数を使えばより簡単に実装できる。
// しかし、FWからIntegrationテストを切り出しておくことで、他のFWに乗り換えたときも
// そのまま使うことができるので、わざわざtokioを用いてアプリケーションを背後で実行している。
// TODO: テスト終了時に、作成したDBインスタンスを削除する処理を追加
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    // メールテスト用のモックサーバを起動
    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        // テストでは、本処理のデータベース名とは異なるランダムな名前のデータベースで処理を行う
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.base_url = email_server.uri();
        c
    };

    // データベースを作成し、マイグレーションする
    configure_database(&configuration.database).await;

    // バックグラウンドタスクとしてアプリケーションを起動する
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application.");
    let application_port = application.port();
    let _ = tokio::spawn(application.run_until_stopped()); // INFO: テスト終了時にサーバは落ちる

    TestApp {
        address: format!("http://127.0.0.1:{}", application_port),
        db_pool: get_connection_pool(&configuration.database),
        email_server,
        port: application_port,
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
