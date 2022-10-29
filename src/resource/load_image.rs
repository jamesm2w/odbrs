use std::collections::HashMap;
use std::error::Error;

use image::DynamicImage;
use serde::{Serialize, Deserialize};

pub struct ImageResources {
    image_data: HashMap<u8, Box<DynamicImage>>,
    // some selection criteria but anyway
}

#[derive(Serialize, Deserialize, Default)]
pub struct ImagesConfig {
    paths: Vec<String>,
    select_by: String
}

#[derive(Serialize, Deserialize)]
pub struct ImageConfig {
    key: Option<String>,
    path: String
}

pub(super) fn load_images(config: ImagesConfig) -> Result<ImageResources, Box<dyn Error>> {
    let mut resources = ImageResources { image_data: HashMap::new() };
    let mut i = 0;
    for path in config.paths {
        let img = image::io::Reader::open(path)?.decode()?;
        resources.image_data.insert(i, Box::new(img));
        i += 1;
    }

    Ok(resources)
}