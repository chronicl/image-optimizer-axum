use axum::{
    extract::{Path, Query},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use image::{imageops::FilterType, io::Reader, ImageError};
use serde::Deserialize;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};

/// Currently only webp images are being served. Default quality is webp quality is 85.
#[derive(Clone, Debug)]
pub struct ImageOptimizer {
    dir: std::path::PathBuf,
    // The key is Resize::to_string + image name.
    cache: Arc<dashmap::DashMap<String, Vec<u8>>>,
}

impl ImageOptimizer {
    pub fn new<P: AsRef<std::path::Path>>(dir: P) -> Result<Self, std::io::Error> {
        Ok(Self {
            dir: dir.as_ref().to_owned(),
            cache: Arc::new(dashmap::DashMap::new()),
        })
    }

    pub fn router(self) -> Router {
        let f = |Path(image): Path<String>, Query(resize): Query<Resize>| async move {
            let image_server = self;
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "image/webp".parse().unwrap());
            (headers, image_server.get_image(&image, &resize))
        };

        Router::new().route("/:image", get(f))
    }

    fn get_image(&self, image: &str, resize: &Resize) -> Result<Vec<u8>, ImageNotFound> {
        if let Some(bytes) = self.cache.get(&key(image, resize)) {
            return Ok(bytes.to_owned());
        } else {
            // Todo: Read with tokio instead of blocking
            // Todo: Handle error better than just ImageNotFound
            let mut im = Reader::open(self.dir.join(image))?
                .decode()
                .map_err(|_| ImageNotFound)?;

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

            // Todo: Consider other formats, like avif
            // Todo: Handle error better
            let im = webp::Encoder::from_image(&im)
                .map_err(|_| ImageNotFound)?
                .encode(resize.quality.unwrap_or(85) as f32);

            self.cache.insert(key(image, resize), im.to_owned());
            return Ok(im.to_owned());
        }
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

impl IntoResponse for ImageNotFound {
    fn into_response(self) -> axum::response::Response {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq)]
struct Resize {
    quality: Option<u8>,
    width: Option<u16>,
    height: Option<u16>,
    cx: Option<u16>,
    cy: Option<u16>,
    cwidth: Option<u16>,
    cheight: Option<u16>,
}

impl Resize {
    fn to_string(&self) -> String {
        let mut s = String::new();
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
