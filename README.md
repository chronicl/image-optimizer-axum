### Image Optimizer for integration with axum

Provides an axum::Router that serves dynamically optimized images.
Currently all images are converted to webp and cached in RAM.

```Rust
let optimizer = ImageOptimizer::new("./images")?;
let router = Router::new().nest("/images", optimizer.router());

let addr = SocketAddr::from(([127, 0, 0, 1], 3003));
axum::Server::bind(&addr)
    .serve(router.into_make_service())
    .await
    .unwrap();

Ok(())
```
