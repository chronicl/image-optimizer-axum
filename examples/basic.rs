use axum::Router;
use image_optimizer::ImageOptimizer;
use std::net::SocketAddr;

// Very important to run with --release flag: cargo run --release --example basic.
// Otherwise the image conversion is very slow.
// Example requests:
// Resizing and changing quality
// http://127.0.0.1:3003/images/sample.jpg?width=100&height=100&quality=80
// Cropping
// http://127.0.0.1:3003/images/sample.jpg?cx=50&cy=50&cwidth=100&cheight=100
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let optimizer = ImageOptimizer::new("./examples/images")?;
    let router = Router::new().nest("/images", optimizer.router());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3003));
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();

    Ok(())
}
