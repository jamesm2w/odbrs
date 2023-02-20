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

use super::static_controller::routes::NetworkData;

const TICK_DEMAND: usize = 10; // 108

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
    pub fn _tick(&self, time: DateTime<Utc>) {
        self._send_demand_request(*self.resources.get_demand_levels().get(time.hour() as usize - 1).unwrap() as usize, time);
    }

    pub fn get_demand_level(&self, time: &DateTime<Utc>) -> usize {
        *self.resources.get_demand_levels().get(time.hour() as usize - 1).unwrap() as usize
    }

    // Send a given amount of demand to the demand generator thread
    pub fn _send_demand_request(&self, amount: usize, time: DateTime<Utc>) {
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
    pub fn start(resources: DemandResources, graph: Arc<Graph>, data: Result<Arc<Graph>, Arc<NetworkData>>) -> Arc<DemandGenerator> {
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
                        
                        buffer.append(&mut demand_gen_ref.generate_amount(diff, &time, data.clone()));
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
    pub fn generate_amount(&self, amount: usize, time: &DateTime<Utc>, data: Result<Arc<Graph>, Arc<NetworkData>>) -> VecDeque<Demand> {
        let mut vec = VecDeque::with_capacity(amount);
        let mut attempts = 0; // limit number of failed generation attempts to keep it fast

        while vec.len() < amount && attempts < 10 {
            println!("Generating demand {}/{}", vec.len(), amount);
            let demand = self.generate_random_pixel(time);
            if should_accept_demand(&demand, data.clone()) {
                vec.push_back(demand);
                attempts = 0; // reset attempts after successful generation
            } else {
                attempts += 1; // increment attempts after failed generation
                continue;
            }
        }
        vec
    }

    pub fn generate_scaled_amount(&self, scale: f64, time: &DateTime<Utc>, data: Result<Arc<Graph>, Arc<NetworkData>>) -> VecDeque<Demand> {
        let amount = (self.get_demand_level(time) as f64 * scale) as usize;
        self.generate_amount(amount, time, data)
    }
}

const HUMAN_WALKING_SPEED: f64 = 1.4; // m/s // TODO: is this consistent?

// Returns true if the demand should be rejected because it's more than 15 min from any bus-stop
pub fn should_accept_demand(demand: &Demand, data: Result<Arc<Graph>, Arc<NetworkData>>) -> bool {
    match data {
        Ok(graph) => {
            let mut min_src_dist = f64::MAX;
            let mut min_dest_dist = f64::MAX;
            
            for (_, node) in graph.get_nodelist() {
                let src_dist = distance(node.point, point64(demand.0));
                let dest_dst = distance(node.point, point64(demand.1));
                
                if src_dist < min_src_dist {
                    min_src_dist = src_dist;
                }

                if dest_dst < min_dest_dist {
                    min_dest_dist = dest_dst;
                }
            }
            
            min_dest_dist / HUMAN_WALKING_SPEED < 15.0 * 60.0 && min_src_dist / HUMAN_WALKING_SPEED < 15.0 * 60.0
        },
        Err(network) => {
            let mut min_src_dist = f64::MAX;
            let mut min_dest_dist = f64::MAX;
            
            for (_, stop) in network.stops.iter() {
                let src_dist = distance(stop.position(), point64(demand.0));
                let dest_dist = distance(stop.position(), point64(demand.1));

                if src_dist < min_src_dist {
                    min_src_dist = src_dist;
                }

                if dest_dist < min_dest_dist {
                    min_dest_dist = dest_dist;
                }
            }

            min_dest_dist / HUMAN_WALKING_SPEED < 15.0 * 60.0 && min_src_dist / HUMAN_WALKING_SPEED < 15.0 * 60.0
        }
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let xs = (a.0 - b.0).abs();
    let ys = (a.1 - b.1).abs();
    xs.hypot(ys)
}

fn point64((a, b): (f32, f32)) -> (f64, f64) {
    (a as f64, b as f64)
}