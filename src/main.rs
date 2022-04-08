use axum::{response::Html, routing::get, Router, extract::Path};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/:name", get(handler));
    let addr = SocketAddr::from(([192,168,0,42], 7777));
    println!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(Path(name): Path<String>)  -> Html<String> {
    Html(format!("<h1>Hello, {}</h1>", name))
}