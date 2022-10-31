use std::{collections::HashMap, sync::Arc};
use std::error::Error;

use image::{RgbImage, DynamicImage};
use serde::{Serialize, Deserialize};

#[derive(Default, Debug)]
pub struct DemandResources {
    image_data: HashMap<u8, Arc<Box<ImageData>>>,
    selection: ImageSelection
}

impl DemandResources {

    pub fn new(selection: ImageSelection) -> Self {
        DemandResources { image_data: HashMap::new(), selection }
    }

    pub fn get_images(&self) -> &HashMap<u8, Arc<Box<ImageData>>> {
        &self.image_data
    }

    pub fn get_selection(&self) -> &ImageSelection {
        &self.selection
    }
}

#[derive(Debug)]
pub struct ImageData {
    image: RgbImage,
    width: u32,
    height: u32,
    max_weight: (u64, u64, u64) // Max weight (R, G, B) //TODO: u64 are a disaster waiting to happen. Max integer size of all weights in a completely white graph 4k x 4k is a 72 bits 
}

impl ImageData {
    pub fn new(image: DynamicImage) -> Self {
        let width = image.width();
        let height = image.height();
        let image = image.into_rgb8();

        ImageData { image, width, height, max_weight: (0, 0, 0) }
    }

    pub fn get_image(&self) -> &RgbImage {
        &self.image
    }

    pub fn get_width(&self) -> u32 {
        self.width
    }

    pub fn get_height(&self) -> u32 {
        self.height
    }

    pub fn get_max_weight(&self) -> (u64, u64, u64) {
        self.max_weight
    }

    pub fn calculate_max_weight(&mut self) {
        // self.image = self.image.into_rgb8();
        self.max_weight = self.image.pixels().fold((0, 0, 0), |acc, pix| {
            (acc.0 + pix.0[0] as u64, acc.1 + pix.0[1] as u64, acc.2 + pix.0[2] as u64)
        });
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "values")]
pub enum ImageSelection {
    #[serde(alias = "random")]
    RandomChoice, // Application chooses a random image to use for demand distribution for each one
    #[serde(alias = "constant")]
    ConstantChoice(u8), // Application uses a constant choice of one image
    #[serde(alias = "time")]
    TimeBasedChoice(Vec<u8>) // Application uses a map based on the hour of the day. Keys should contain keys 0-23 other keys would not be used
}

impl Default for ImageSelection {
    fn default() -> Self {
        ImageSelection::RandomChoice
    }
}

#[derive(Serialize, Deserialize, Default, Debug
)]
pub struct DemandResourcesConfig {
    pub paths: Vec<String>, // Map of path keys and paths
    pub select_by: ImageSelection
}

pub fn load_images(config: DemandResourcesConfig) -> Result<DemandResources, Box<dyn Error>> {
    let mut demand_resources = DemandResources::new(config.select_by);
    
    let mut key = 0;
    for path in config.paths {
        let img = image::io::Reader::open(format!("./data/img/{}", path))?.decode()?;
        let mut img = ImageData::new(img);
        img.calculate_max_weight();

        demand_resources.image_data.insert(key, Arc::from(Box::new(img)));
        key += 1;
    }

    Ok(demand_resources)
}

// TODO: Add fallback image for 0 demand which would not generate anything!!