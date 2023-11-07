use anyhow::Context;
use askama::Template;
use axum::{
    body::Bytes,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};

use sha1::{Digest, Sha1};

mod structs;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use serde_bytes::ByteBuf;
use serde_derive::{Deserialize, Serialize};

use std::sync::{Arc, Mutex};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    file: Mutex<Vec<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "with_axum_htmx_askama=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("initializing router...");

    let app_state = Arc::new(AppState {
        file: Mutex::new(vec![]),
    });
    let router = Router::new()
        .route("/", get(hello))
        .route("/file", post(handle_fl))
        .with_state(app_state);
    let port = 8000_u16;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    info!("router initialized, now listening on port {}", port);
    //add tailwind
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .context("error while starting server")?;

    Ok(())
}

//handle file upload

#[derive(TryFromMultipart)]
struct Fl {
    file: FieldData<Bytes>,
}
#[derive(Template)]
#[template(path = "name.html")]
struct NameTemplate {
    name: String,
    peers: String,
}

async fn handle_fl(data: TypedMultipart<Fl>) -> impl IntoResponse {
    let decode: structs::Torrent = serde_bencode::from_bytes(&data.file.contents).unwrap();
    let enco_info = serde_bencode::to_bytes(&decode.info).unwrap();
    let mut hasher = Sha1::new();
    hasher.update(enco_info);
    let res = hasher.finalize();
    let hx = NameTemplate {
        name: decode.info.name,
        peers: hex::encode(res),
    };
    hx.render().unwrap()
}
//server the file upload
async fn hello() -> impl IntoResponse {
    let template = HelloTemplate {};
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate;

/// A wrapper type that we'll use to encapsulate HTML parsed by askama into valid HTML for axum to serve.
struct HtmlTemplate<T>(T);

/// Allows us to convert Askama HTML templates into valid HTML for axum to serve in the response.
impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        // Attempt to render the template with askama
        match self.0.render() {
            // If we're able to successfully parse and aggregate the template, serve it
            Ok(html) => Html(html).into_response(),
            // If we're not, return an error or some bit of fallback HTML
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}
