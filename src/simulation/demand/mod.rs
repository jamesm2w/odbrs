use std::{
    collections::VecDeque,
    sync::{
        mpsc::{sync_channel, SyncSender},
        Arc, RwLock,
    },
};

use chrono::{DateTime, Utc, Timelike};
use rand::Rng;

use crate::{graph::Graph, resource::load_image::{DemandResources, ImageSelection, ImageData}};

const TICK_DEMAND: usize = 10; // 108

#[derive(Debug)]
enum DemandThreadMessage {
    Yield(usize, DateTime<Utc>),
    Stop,
}

#[derive(Debug)]
pub struct DemandGenerator {
    resources: DemandResources,
    bounds: (f32, f32, f32, f32),
    thread_gen_tx: SyncSender<DemandThreadMessage>,
    demand_queue: RwLock<VecDeque<Demand>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Demand(pub (f32, f32), pub (f32, f32), pub DateTime<Utc>);

impl DemandGenerator {

    // Send a ticks worth of demand request to the demand generator
    pub fn tick(&self, time: DateTime<Utc>) {
        self.send_demand_request(*self.resources.get_demand_levels().get(time.hour() as usize - 1).unwrap() as usize, time);
    }

    // Send a given amount of demand to the demand generator thread
    pub fn send_demand_request(&self, amount: usize, time: DateTime<Utc>) {
        match self.thread_gen_tx.send(DemandThreadMessage::Yield(amount, time)) {
            Ok(()) => (),
            Err(err) => panic!("Sending to demand gen thread failed {}", err),
        };
    }

    pub fn shutdown(&self) {
        match self.thread_gen_tx.send(DemandThreadMessage::Stop) {
            Ok(()) => (),
            Err(err) => panic!("Sending to demand gen thread failed {}", err),
        };
    }

    pub fn get_transform_info(graph: Arc<Graph>) -> (f32, f32, f32, f32) {
        match graph.get_transform().read() {
            Ok(transform) => {
                (transform.left, transform.right, transform.bottom, transform.top)
            },
            Err(err) => panic!("Error reading transform {}", err)
        }
    }

    pub fn get_demand_queue(&self) -> &RwLock<VecDeque<Demand>> {
        &self.demand_queue
    }

    // Creates a demand generator and runs a thread which does the actual generation
    pub fn start(resources: DemandResources, graph: Arc<Graph>) -> Arc<DemandGenerator> {
        let (tx, rx) = sync_channel(1);
        let demand_gen = DemandGenerator {
            resources,
            bounds: DemandGenerator::get_transform_info(graph), 
            thread_gen_tx: tx,
            demand_queue: RwLock::new(VecDeque::new()),
        };

        let demand_gen = Arc::from(demand_gen);
        let demand_gen_ref = demand_gen.clone();

        std::thread::spawn(move || {
            let mut buffer = VecDeque::new();
            let mut last_time: DateTime<Utc> = Default::default();
            let mut started = false;
            loop {
                match rx.try_recv() {
                    Ok(DemandThreadMessage::Yield(amount, time)) => {
                        let diff = amount.saturating_sub(buffer.len());

                        if diff == 0 {
                            buffer.drain(0..buffer.len());
                        }
                        
                        buffer.append(&mut demand_gen_ref.generate_amount(diff, &time));
                        last_time = time;

                        match demand_gen_ref.demand_queue.write() {
                            Ok(mut vecdeq) => vecdeq.extend(buffer.drain(0..amount)),
                            Err(err) => panic!("Error writing back demand! {}", err),
                        }

                        started = true;
                    }
                    Ok(DemandThreadMessage::Stop) => {
                        break;
                    }
                    Err(err) => {
                        match err {
                            std::sync::mpsc::TryRecvError::Disconnected => break,
                            std::sync::mpsc::TryRecvError::Empty => {
                                // if nothing to do on this go around why not pre-compute something
                                // TODO: probably some funky interactions with dates and times here!
                                if started && buffer.len() < 9 * TICK_DEMAND / 10 {
                                    // buffer about 90% of the demand on a tick (roughly)
                                    buffer.push_back(demand_gen_ref.generate_random_pixel(&last_time));
                                }
                            }
                        }
                    }
                }
            }
        });

        demand_gen
    }

    // Selects the right image based on numerous factors
    pub fn select_image(&self, time: &DateTime<Utc>) -> Arc<Box<ImageData>> {
        match self.resources.get_selection() {
            ImageSelection::ConstantChoice(i) => {
                self.resources.get_images().get(i).expect("Wrong key in selection").clone()
            },
            ImageSelection::RandomChoice => {
                let i = rand::thread_rng().gen_range(0..self.resources.get_images().len() as u8);
                self.resources.get_images().get(&i).expect("Couldn't randomise selection").clone()
            },
            ImageSelection::TimeBasedChoice(map) => {
                let i = map.get(time.hour() as usize).expect("Couldn't get time based index");
                
                // println!("time {:?} choice {:?}", time.hour(), i);
                self.resources.get_images().get(&i).expect("Couldn't select based on time").clone()
            }
        }
    }

    // Generates a singular demand
    pub fn generate_random_pixel(&self, time: &DateTime<Utc>) -> Demand {
        let image = self.select_image(time);

        let mut r_pix = None;
        let mut g_pix = None;
        let mut b_pix = None;

        let (r_w, g_w, b_w) = image.get_max_weight();

        // println!("image max weight {:?} {:?} {:?}", r_w, g_w, b_w);

        let mut rng_r = rand::thread_rng().gen_range(0..if r_w > 0 { r_w } else { 1 });
        let mut rng_g = rand::thread_rng().gen_range(0..if g_w > 0 { g_w } else { 1 });
        let mut rng_b = rand::thread_rng().gen_range(0..if b_w > 0 { b_w } else { 1 });

        for (i, pix) in image.get_image().pixels().enumerate() {
            if rng_r > 0 { rng_r = match rng_r.checked_sub(pix.0[0] as u64) {
                Some(a) => a,
                None => { r_pix = Some(i); 0 }
            } }

            if rng_g > 0 { rng_g = match rng_g.checked_sub(pix.0[1] as u64) {
                Some(a) => a,
                None => { g_pix = Some(i); 0 }
            } }
            
            if rng_b > 0 { rng_b = match rng_b.checked_sub(pix.0[2] as u64) {
                Some(a) => a,
                None => { b_pix = Some(i); 0 }
            } }

            if rng_r <= 0 && rng_g <= 0 && rng_b <= 0 {
                break;
            }
        }

        let width = image.get_width() as usize;

        let map_width = self.bounds.1 - self.bounds.0;
        let map_height = self.bounds.3 - self.bounds.2;

        let mut source = (self.bounds.0, self.bounds.3);
        let mut dest = (self.bounds.0, self.bounds.3);
    
        if let Some(r) = r_pix {
            let r_x_y = (r % width, r / width);
            // println!("Gen: random red value: {:?}", r_x_y);
            source = (
                (r_x_y.0 as f32 + rand::thread_rng().gen_range(0.0..1.0_f32)) *  (map_width as f32 / width as f32) + self.bounds.0,
                (r_x_y.1 as f32 + rand::thread_rng().gen_range(0.0..1.0_f32)) * -(map_height as f32 / image.get_height() as f32) + self.bounds.3
            )
        }
        
        if let Some(g) = g_pix {
            let _g_x_y = (g % width, g / width);
            // println!("Gen: random green value: {:?}", _g_x_y);
        }

        if let Some(b) = b_pix {
            let b_x_y = (b % width, b / width);
            // println!("Gen: random blue value: {:?}", b_x_y);
            dest = (
                (b_x_y.0 as f32 + rand::thread_rng().gen_range(0.0..1.0_f32)) *  (map_width as f32 / width as f32) + self.bounds.0,
                (b_x_y.1 as f32 + rand::thread_rng().gen_range(0.0..1.0_f32)) * -(map_height as f32 / image.get_height() as f32) + self.bounds.3
            )
        }

        // println!("Gen Pixel Src={:?} Dest={:?}", source, dest);
        if source == (0.0, 0.0) || dest == (0.0, 0.0) {
            println!("Generated a 0,0 source {:?} dest {:?}", source, dest);
        }

        return Demand(source, dest, DateTime::<Utc>::MIN_UTC);
    }

    // Generates an amount of demand
    pub fn generate_amount(&self, amount: usize, time: &DateTime<Utc>) -> VecDeque<Demand> {
        let mut vec = VecDeque::with_capacity(amount);
        for _ in 0..amount {
            vec.push_back(self.generate_random_pixel(time));
        }
        vec
    }
}

// pub struct DemandGenerator {

//     // Reference to graph nodes, etc
//     graph: Arc<Graph>,

//     // Image
//     image: Option<GrayImage>,
//     normalisation_factor: u64,

//     // Map dimension information to use when scaling things
//     map_left: f64,
//     map_top: f64,
//     map_width: f64,
//     map_height: f64,

//     // Shared queue with the simulation thread to put demands when generated
//     demand_queue: Arc<RwLock<VecDeque<Demand>>>,
// }

// / vec of images as geographical weights
// / options such as "time" to select which image based on hour of day -- config and loaded at startup
// / fall back of just random uniform over whole graph.

// / mapping
// / x |-> x *   (map width / img width) + left map
// / y |-> y * - (map width / img width) + top map

// / image
// / pub fn generate_randompixel(param image to use) {
// /     1. generate random number between 0..(max weighted value)
// /     2. loop through image until hit that random value
// /     3. get (x,y) of pixel => apply mapping to the OS map
// /     return mapped x, y
// / }

// / demand gen -- separate thread to GUI/sim, could precompute the next batch of demand while GUI tick is occurring
// / 1. run rng pixel gen 1000 times in a tick
// / 2. push back map points onto the queue
// / 3. return something to give feedback to sim thread
// /

// pub enum SelectionStrategy {
//     Constant(u8),
//     Random,
//     Time
// }

// fn get_image(time: chrono::DateTime<Utc>, images: ImageResources, strat: SelectionStrategy) -> Option<&'static Box<DynamicImage>> {
//     match strat {
//         SelectionStrategy::Constant(i) => {
//             // Choose the one with key i always
//             match images.image_data.get(&i) {
//                 Some(bx) => Some(bx),
//                 None => None
//             }
//         },
//         SelectionStrategy::Random => {
//             // Choose a random one
//             let index = rand::thread_rng().gen_range(0..=images.image_data.len());
//             match images.image_data.values().nth(index) {
//                 Some(bx) => Some(bx),
//                 None => None
//             }
//         },
//         SelectionStrategy::Time => {
//             // Select 0 -> 12 for active hours 0->12
//             let hour = time.hour();
//             match hour {
//                 0..=6 => None,
//                 7..=19 => None, // Parse the time file
//                 20..=23 => None,
//                 _ => None
//             }
//         }
//     }
// }

// impl DemandGenerator {

//     pub fn new(graph: Arc<Graph>) -> Self {
//         DemandGenerator {
//             graph,
//             image: None,
//             normalisation_factor: 0,

//             map_left: 0.0,
//             map_top: 0.0,
//             map_width: 0.0,
//             map_height: 0.0,

//             demand_queue: Arc::new(RwLock::new(Default::default()))
//         }
//     }

//     pub fn load_imge(&mut self) -> Result<(), Box<dyn Error>> {
//         let img = image::io::Reader::open("./data/img/test.png")?.decode()?;
//         let map = img.into_luma8();

//         let total_img_brightness = map.pixels().fold(0_u64, |acc, pix| acc + pix.0[0] as u64);
//         println!("total brightness {:?}", total_img_brightness);
//         println!("image {:?}", map);
//         self.image = Some(map);
//         self.normalisation_factor = total_img_brightness;
//         Ok(())
//     }

//     pub fn idk(&self) {
//         match self.graph.get_transform().read() {
//             Ok(transform) => {

//             },
//             Err(err) => panic!("Couldn't read graph {}", err)
//         }
//     }

//     // Finds a random pixel weighted by the brightness
//     pub fn sample_random_pixel(&mut self) -> (f64, f64) {
//         let rng = rand::thread_rng().gen_range(0..=self.normalisation_factor+1);
//         let mut seen_total_weight = 0;

//         let image = self.image.as_ref().unwrap();

//         let (i, _) = image.pixels().enumerate().find(|&(_, pix)| {
//             seen_total_weight += pix.0[0] as u64;
//             if seen_total_weight > rng {
//                 true
//             } else {
//                 false
//             }
//         }).unwrap();

//         let y = i as u32 / image.width();
//         let x = i as u32 % image.width();

//         return (x as f64 + rand::thread_rng().gen_range(-0.5..=0.5), y as f64 + rand::thread_rng().gen_range(-0.5..=0.5));
//     }

//     pub fn do_generate_demand(&mut self) -> Vec<Demand> {
//         // Returns a list of demand objects generated.
//         let mut ret = vec![];
//         let mut rand = rand::thread_rng();
//         let node_a = self.graph.get_nodelist().keys().nth(rand.gen_range(0..=self.graph.get_nodelist().len()));
//         let node_b = self.graph.get_nodelist().keys().nth(rand.gen_range(0..=self.graph.get_nodelist().len()));

//         ret.push(Demand(*node_a.unwrap(), *node_b.unwrap(), 0));
//         ret
//     }
// }

// #[derive(Debug)]
// pub struct Demand(u128, u128, u128); // start, end, latest arrival time

// mod test {
//     use std::fs;

//     #[test]
//     pub fn test() {
//         use super::*;
//         use std::path::PathBuf;
//         use crate::Module;

//         let mut odbrs = crate::Main::default();

//         odbrs.init(PathBuf::from(r#"data/config.toml"#), ()).unwrap();
//         let mut demand = DemandGenerator::new(odbrs.graph.clone());

//         match demand.load_imge() {
//             Ok(()) => (),
//          Err(err) => panic!("Couldn't load image {}", err)
//         };

//         let mut demand_buffer = vec![];
//         let time = std::time::Instant::now();

//         for _ in 0..997 {
//             demand_buffer.push(demand.sample_random_pixel());
//         }

//         println!("Generated in {:?}", time.elapsed());
//         println!("Demand len {:?}", demand_buffer.len());
//         println!("{:?}", demand_buffer.get(0..10));

//         match fs::write("./data/dist.csv", demand_buffer.into_iter().map(|pt| format!("{},{}\n", pt.0, pt.1)).collect::<String>()) {
//             Ok(_) => (),
//             Err(err) => panic!("couldn't write to file {}", err)
//         }
//     }

// }
