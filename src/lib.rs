use image::{imageops::FilterType, ImageFormat};
use serde::Deserialize;
use std::{io::Cursor, sync::Arc};

/// Currently only webp images are being served. Default quality is webp quality is 85.
#[derive(Clone, Debug)]
pub struct ImageOptimizer {
    dir: std::path::PathBuf,
    // The key is Resize::to_string + image name.
    cache: Arc<dashmap::DashMap<String, Vec<u8>>>,
}

impl ImageOptimizer {
    const CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

    pub fn new<P: AsRef<std::path::Path>>(dir: P) -> Result<Self, std::io::Error> {
        let dir = dir.as_ref().to_owned();
        tracing::debug!("serving images from {dir:?}");

        Ok(Self {
            dir,
            cache: Arc::new(dashmap::DashMap::new()),
        })
    }

    #[cfg(feature = "axum")]
    pub fn axum_router(self) -> axum::Router {
        use axum::{
            extract::{Path, Query},
            http::{header, HeaderMap, StatusCode},
            response::IntoResponse,
            routing::get,
            Router,
        };

        let f = |Path(image): Path<String>, Query(resize): Query<Resize>| async move {
            let image_server = self;

            tracing::debug!("image {image} requested");

            let mut headers = HeaderMap::new();
            let content_type = parse_content_type(&resize, &image);
            headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
            headers.insert(header::CACHE_CONTROL, Self::CACHE_CONTROL.parse().unwrap());

            (headers, image_server.get_image(&image, &resize))
        };

        Router::new().route("/:image", get(f))
    }

    #[cfg(feature = "poem")]
    pub fn poem_router(self) -> poem::Route {
        use poem::{
            get, handler,
            http::StatusCode,
            web::{Data, Path, Query},
            EndpointExt, Response, Result, Route,
        };

        #[handler]
        fn get_image(
            image_optimizer: Data<&ImageOptimizer>,
            resize: Query<Resize>,
            path: Path<String>,
        ) -> Result<Response> {
            let image = path.0;
            let resize = *resize;
            let content_type = parse_content_type(&resize, &image);

            tracing::debug!("image {image} requested");

            let bytes = image_optimizer
                .get_image(&image, &resize)
                .map_err(|_| http::StatusCode::NOT_FOUND)?;

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", content_type)
                .header("Cache-Control", ImageOptimizer::CACHE_CONTROL)
                .body(bytes))
        }

        let router = Route::new().at("/:image", poem::get(get_image).data(self));

        router
    }

    pub fn get_image(&self, image: &str, resize: &Resize) -> Result<Vec<u8>, ImageNotFound> {
        if let Some(bytes) = self.cache.get(&key(image, resize)) {
            return Ok(bytes.to_owned());
        } else {
            // Todo: Read with tokio instead of blocking
            // Todo: Handle error better than just ImageNotFound
            let mut im = image::open(self.dir.join(image)).map_err(|_| ImageNotFound)?;

            if resize.width.is_some() || resize.height.is_some() {
                im = im.resize(
                    resize.width.unwrap_or(u16::MAX) as u32,
                    resize.height.unwrap_or(u16::MAX) as u32,
                    FilterType::Lanczos3,
                );
            }

            if resize.cx.is_some()
                || resize.cy.is_some()
                || resize.cwidth.is_some()
                || resize.cheight.is_some()
            {
                im = im.crop_imm(
                    resize.cx.unwrap_or(0) as u32,
                    resize.cy.unwrap_or(0) as u32,
                    resize.cwidth.unwrap_or(u16::MAX) as u32,
                    resize.cheight.unwrap_or(u16::MAX) as u32,
                );
            }

            if resize.webp.unwrap_or(false) {
                // Todo: Consider other formats, like avif
                // Todo: Handle error better
                let im = webp::Encoder::from_image(&im)
                    .map_err(|_| ImageNotFound)?
                    .encode(resize.quality.unwrap_or(85) as f32);

                self.cache.insert(key(image, resize), im.to_owned());
                Ok(im.to_owned())
            } else {
                let mut v = Cursor::new(Vec::new());
                let format = match image.split('.').last().unwrap_or("jpg") {
                    "jpg" => ImageFormat::Jpeg,
                    "png" => ImageFormat::Png,
                    "gif" => ImageFormat::Gif,
                    _ => ImageFormat::Jpeg,
                };
                im.write_to(&mut v, format).map_err(|_| ImageNotFound)?;
                self.cache
                    .insert(key(image, resize), v.get_ref().to_owned());
                Ok(v.into_inner())
            }
        }
    }
}

fn parse_content_type(resize: &Resize, image: &str) -> String {
    if resize.webp.unwrap_or(false) {
        "image/webp".into()
    } else {
        let image_type = image.split('.').last().unwrap_or("jpg");
        format!("image/{image_type}")
    }
}

#[test]

fn test_poem() {
    #[cfg(feature = "poem")]
    {
        let optimizer = ImageOptimizer::new("./examples/images").unwrap();
        let router = optimizer.poem_router();
    }
}

fn key(image: &str, resize: &Resize) -> String {
    let mut key: String = resize.to_string();
    key.push_str(image);
    key
}

#[derive(thiserror::Error, Debug)]
#[error("Image not found")]
struct ImageNotFound;

impl From<std::io::Error> for ImageNotFound {
    fn from(_: std::io::Error) -> Self {
        Self
    }
}

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for ImageNotFound {
    fn into_response(self) -> axum::response::Response {
        axum::http::StatusCode::NOT_FOUND.into_response()
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq)]
pub struct Resize {
    pub webp: Option<bool>,
    pub quality: Option<u8>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub cx: Option<u16>,
    pub cy: Option<u16>,
    pub cwidth: Option<u16>,
    pub cheight: Option<u16>,
}

impl Resize {
    pub fn to_string(&self) -> String {
        let mut s = String::new();
        if let Some(_) = self.webp {
            s.push_str("webp");
        }
        if let Some(quality) = self.quality {
            s.push_str(&format!("q{}", quality));
        }
        if let Some(width) = self.width {
            s.push_str(&format!("w{}", width));
        }
        if let Some(height) = self.height {
            s.push_str(&format!("h{}", height));
        }
        if let Some(cx) = self.cx {
            s.push_str(&format!("cx{}", cx));
        }
        if let Some(cy) = self.cy {
            s.push_str(&format!("cy{}", cy));
        }
        if let Some(cwidth) = self.cwidth {
            s.push_str(&format!("cw{}", cwidth));
        }
        if let Some(cheight) = self.cheight {
            s.push_str(&format!("ch{}", cheight));
        }
        s
    }
}
