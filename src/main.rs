// INFO: Skip: 5.4 Deploy To DigitalOcean Apps Platform

// NEXT: 7.5 Database Migrations
use api::configuration::get_configuration;
use api::startup::Application;
use api::telemetry::{get_subscriber, init_subscriber};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");

    let application = Application::build(configuration).await?;
    application.run_until_stopped().await?;
    Ok(())
}
