use std::{sync::{Arc, RwLock}, error::Error, collections::VecDeque};

use image::GrayImage;
use rand::Rng;

use crate::graph::Graph;


struct DemandGenerator {

    // Reference to graph nodes, etc
    graph: Arc<Graph>,

    // Image 
    image: Option<GrayImage>,
    normalisation_factor: u64,

    // Map dimension information to use when scaling things
    map_left: f64,
    map_top: f64,
    map_width: f64,
    map_height: f64,

    // Shared queue with the simulation thread to put demands when generated
    demand_queue: Arc<RwLock<VecDeque<Demand>>>,
}

/// vec of images as geographical weights 
/// options such as "time" to select which image based on hour of day -- config and loaded at startup
/// fall back of just random uniform over whole graph.

/// mapping
/// x |-> x *   (map width / img width) + left map
/// y |-> y * - (map width / img width) + top map

/// image 
/// pub fn generate_randompixel(param image to use) {
///     1. generate random number between 0..(max weighted value)
///     2. loop through image until hit that random value
///     3. get (x,y) of pixel => apply mapping to the OS map 
///     return mapped x, y
/// }
 
/// demand gen -- separate thread to GUI/sim, could precompute the next batch of demand while GUI tick is occurring 
/// 1. run rng pixel gen 1000 times in a tick 
/// 2. push back map points onto the queue
/// 3. return something to give feedback to sim thread

impl DemandGenerator {

    pub fn new(graph: Arc<Graph>) -> Self {
        DemandGenerator { 
            graph, 
            image: None, 
            normalisation_factor: 0, 

            map_left: 0.0,
            map_top: 0.0,
            map_width: 0.0,
            map_height: 0.0,

            demand_queue: Arc::new(RwLock::new(Default::default())) 
        }
    }

    pub fn load_imge(&mut self) -> Result<(), Box<dyn Error>> {
        let img = image::io::Reader::open("./data/img/test.png")?.decode()?;
        let map = img.into_luma8();

        let total_img_brightness = map.pixels().fold(0_u64, |acc, pix| acc + pix.0[0] as u64);
        println!("total brightness {:?}", total_img_brightness);
        println!("image {:?}", map);
        self.image = Some(map);
        self.normalisation_factor = total_img_brightness;
        Ok(())
    }

    pub fn idk(&self) {
        match self.graph.get_transform().read() {
            Ok(transform) => {
                
            },
            Err(err) => panic!("Couldn't read graph {}", err)
        }
    }

    // Finds a random pixel weighted by the brightness
    pub fn sample_random_pixel(&mut self) -> (f64, f64) {
        let rng = rand::thread_rng().gen_range(0..=self.normalisation_factor+1);
        let mut seen_total_weight = 0;

        let image = self.image.as_ref().unwrap();

        let (i, _) = image.pixels().enumerate().find(|&(_, pix)| {
            seen_total_weight += pix.0[0] as u64;
            if seen_total_weight > rng {
                true
            } else {
                false
            }
        }).unwrap();

        let y = i as u32 / image.width();
        let x = i as u32 % image.width();

        return (x as f64 + rand::thread_rng().gen_range(-0.5..=0.5), y as f64 + rand::thread_rng().gen_range(-0.5..=0.5)); 
    }

    pub fn do_generate_demand(&mut self) -> Vec<Demand> {
        // Returns a list of demand objects generated.
        let mut ret = vec![];
        let mut rand = rand::thread_rng();
        let node_a = self.graph.get_nodelist().keys().nth(rand.gen_range(0..=self.graph.get_nodelist().len()));
        let node_b = self.graph.get_nodelist().keys().nth(rand.gen_range(0..=self.graph.get_nodelist().len()));
    
        ret.push(Demand(*node_a.unwrap(), *node_b.unwrap(), 0));
        ret
    } 
}

#[derive(Debug)]
pub struct Demand(u128, u128, u128); // start, end, latest arrival time

mod test {
    use std::fs;


    #[test]
    pub fn test() {
        use super::*;
        use std::path::PathBuf;
        use crate::Module;

        let mut odbrs = crate::Main::default();
        
        odbrs.init(PathBuf::from(r#"data/config.toml"#), ()).unwrap();
        let mut demand = DemandGenerator::new(odbrs.graph.clone());
        
        match demand.load_imge() {
            Ok(()) => (),
         Err(err) => panic!("Couldn't load image {}", err)
        };

        let mut demand_buffer = vec![];
        let time = std::time::Instant::now();

        for _ in 0..997 {
            demand_buffer.push(demand.sample_random_pixel());
        }

        println!("Generated in {:?}", time.elapsed());
        println!("Demand len {:?}", demand_buffer.len());
        println!("{:?}", demand_buffer.get(0..10));

        match fs::write("./data/dist.csv", demand_buffer.into_iter().map(|pt| format!("{},{}\n", pt.0, pt.1)).collect::<String>()) {
            Ok(_) => (),
            Err(err) => panic!("couldn't write to file {}", err)
        }
    }

}