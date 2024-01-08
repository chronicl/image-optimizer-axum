use image_optimizer::ImageOptimizer;
use std::net::SocketAddr;

// Very important to run with --release flag: cargo run --release --example basic --features axum.
// Or use --features poem instead if you want to use poem as the server framework.
// Otherwise the image conversion is very slow.
// Example requests:
// Resizing and changing quality
// http://127.0.0.1:3003/images/sample.jpg?width=100&height=100&quality=80
// Cropping
// http://127.0.0.1:3003/images/sample.jpg?cx=50&cy=50&cwidth=100&cheight=100
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let optimizer = ImageOptimizer::new("./examples/images")?;

    #[cfg(feature = "axum")]
    {
        let router = axum::Router::new().nest("/images", optimizer.axum_router());

        let addr = SocketAddr::from(([127, 0, 0, 1], 3003));
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, router.into_make_service()).await?;
    }

    #[cfg(all(feature = "poem", not(feature = "axum")))]
    {
        let router = poem::Route::new().nest("images", optimizer.poem_router());
        poem::Server::new(poem::listener::TcpListener::bind("127.0.0.1:3003"))
            .run(router)
            .await?;
    }

    Ok(())
}
