// INFO: Skip: 5.4 Deploy To DigitalOcean Apps Platform

// NEXT: 6.1 Requirements
use api::configuration::get_configuration;
use api::startup::run;
use api::telemetry::{get_subscriber, init_subscriber};
use sqlx::PgPool;
use std::net::TcpListener;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool = PgPool::connect_lazy(&configuration.database.connection_string())
        .expect("Failed to connect to Postgres.");

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let listener = TcpListener::bind(address)?;

    run(listener, connection_pool)?.await
}
