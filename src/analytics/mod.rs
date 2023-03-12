use std::{sync::mpsc::{Sender, Receiver}, collections::HashMap, io::Write};

use eframe::NativeOptions;

use crate::{Module, gui::analytics::{State, create_distributions}};

pub enum AnalyticsPackage {
    None,
    PassengerEvent(PassengerAnalyticsEvent),
    VehicleEvent(VehicleAnalyticsEvent),
    SimulationEvent(SimulationAnalyticsEvent)
}

impl AnalyticsPackage {
    fn handle(&self, analytics: &mut Analytics) {
        match self {
            AnalyticsPackage::None => {},
            AnalyticsPackage::PassengerEvent(event) =>  event.handle(analytics),
            AnalyticsPackage::VehicleEvent(event) => event.handle(analytics),
            AnalyticsPackage::SimulationEvent(event) => event.handle(analytics)
        }
    }
}

pub enum PassengerAnalyticsEvent {
    StartWalkingTick { id: u32 },
    EndWalkingTick { id: u32 },
    WaitingTick { id: u32, waiting_pos: (f64, f64) },
    InTransitTick { id: u32 }
}

impl PassengerAnalyticsEvent {
    fn handle(&self, analytics: &mut Analytics) {
        match self {
            PassengerAnalyticsEvent::WaitingTick { id, waiting_pos } => {
                // println!("Analytics: Passenger {} is waiting at {:?}", id, waiting_pos);
                analytics.passenger_waits.entry(*id).and_modify(|e| *e += 1).or_insert(1);
            },
            PassengerAnalyticsEvent::InTransitTick { id } => {
                // println!("Analytics: Passenger {} is in transit", id);
                analytics.passenger_travel.entry(*id).and_modify(|e| *e += 1).or_insert(1);
            },
            PassengerAnalyticsEvent::StartWalkingTick { id } => {
                analytics.passenger_walking.entry(*id).and_modify(|e| e.0 += 1).or_insert((1, 0));
            },
            PassengerAnalyticsEvent::EndWalkingTick { id } => {
                analytics.passenger_walking.entry(*id).and_modify(|e| e.1 += 1).or_insert((0, 1));
            }
        }
    }
}

pub enum VehicleAnalyticsEvent {
    MovementTick { id: u32, pos: (f64, f64) },
    PassengerPickup { id: u32, passenger_id: u32 },
    PassengerDropoff { id: u32, passenger_id: u32 }
}

impl VehicleAnalyticsEvent {
    fn handle(&self, analytics: &mut Analytics) {
        match self {
            VehicleAnalyticsEvent::MovementTick { id, pos } => {
                // println!("Analytics: Vehicle {} is at {:?}", id, pos);
                analytics.vehicle_travel.entry(*id).and_modify(|e| *e += 1).or_insert(1);
            },
            VehicleAnalyticsEvent::PassengerPickup { id, passenger_id } => {
                // println!("Analytics: Vehicle {} picked up passenger {}", id, passenger_id);
                analytics.vehicle_passengers.entry(*id).and_modify(|e| e.0 += 1).or_insert((1, 0));
            },
            VehicleAnalyticsEvent::PassengerDropoff { id, passenger_id } => {
                // println!("Analytics: Vehicle {} dropped off passenger {}", id, passenger_id);
                analytics.vehicle_passengers.entry(*id).and_modify(|e| e.1 += 1).or_insert((0, 1));
            }
        }
    }
}

pub enum SimulationAnalyticsEvent {
    TickTime { tick: u32, time: f64 }
}

impl SimulationAnalyticsEvent {
    fn handle(&self, analytics: &mut Analytics) {
        match self {
            SimulationAnalyticsEvent::TickTime { tick, time } => {
                // println!("Analytics: Tick {} took {} seconds", tick, time);
                analytics.tick_times.push(*time);
                analytics.avg_tick_time = analytics.tick_times.iter().sum::<f64>() / analytics.tick_times.len() as f64;
            }
        }
    }
}

pub struct Analytics {
    tx: Sender<AnalyticsPackage>,
    rx: Receiver<AnalyticsPackage>,

    tick_times: Vec<f64>, // Ticks and the time it took to process them
    avg_tick_time: f64,

    passenger_waits: HashMap<u32, u32>, // Ticks passenger (key) spent waiting
    passenger_travel: HashMap<u32, u32>, // Ticks passenger (key) spent in transit
    passenger_walking: HashMap<u32, (u64, u64)>, // Ticks passenger (key) spent walking from start, ticks spent walking to end
    vehicle_travel: HashMap<u32, u32>, // Ticks vehicle (key) spent in transit
    vehicle_passengers: HashMap<u32, (u64, u64)> // Number of passengers vehicle (key) picked up, dropped off

}

impl Default for Analytics {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<AnalyticsPackage>();
        Self {
            rx,
            tx,
            tick_times: Vec::new(),
            avg_tick_time: 0.0,
            passenger_waits: HashMap::new(),
            passenger_travel: HashMap::new(),
            passenger_walking: HashMap::new(),
            vehicle_travel: HashMap::new(),
            vehicle_passengers: HashMap::new()
        }
    }
}

impl Module for Analytics {
    type ReturnType = Sender<AnalyticsPackage>;
    type Configuration = ();
    type Parameters = ();

    fn get_name(&self) -> &str {
        "Analytics"
    }

    fn init(
            &mut self,
            _config: Self::Configuration,
            _parameters: Self::Parameters,
        ) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
            let tx = self.tx.clone();
            Ok(tx)
    }
}

impl Analytics {
    // loop trhough the rx channel buffer and process the messages to create the analytics profile 
    pub fn run(&mut self) -> () {
        loop {
            match self.rx.try_recv() {
                Ok(package) => {
                    package.handle(self);
                },
                Err(e) => {
                    println!("Analytics: Error: {}", e);
                    break;
                }
            }
        }

        // Write analytics out to file
        // TODO: write to file

        println!("Average Tick Time: {}", self.avg_tick_time);
        println!("Analytics Sizes: \nPassengers with: \n\tWaits: {} \n\tTravel: {} \n\tWalking: {} \nVehicles with: \n\tTravel: {} \n\tPassengers: {}", self.passenger_waits.len(), self.passenger_travel.len(), self.passenger_walking.len(), self.vehicle_travel.len(), self.vehicle_passengers.len());

        let output_path_passenger = format!(r#"data/output/{}-passenger-output.csv"#, chrono::Local::now().format("%Y-%m-%d-%H-%M-%S"));
        let mut passenger_output_file = std::fs::File::create(&output_path_passenger).unwrap();
        writeln!(&mut passenger_output_file, "Passenger ID,Waiting Ticks,Travel Ticks,Start Walking Ticks,End Walking Ticks").unwrap();
        for (id, travel) in &self.passenger_travel {
            let wait = self.passenger_waits.get(id).unwrap_or(&0);
            let (walk_start, walk_end) = self.passenger_walking.get(id).unwrap_or(&(0,0));
            writeln!(passenger_output_file, "{},{},{},{},{}", id, wait, travel, walk_start, walk_end).unwrap();
        }

        let output_path = format!(r#"data/output/{}-vehicle-output.csv"#, chrono::Local::now().format("%Y-%m-%d-%H-%M-%S"));
        let mut vehicle_output_file = std::fs::File::create(&output_path).unwrap();
        writeln!(vehicle_output_file, "Vehicle ID,Travel Ticks,Passengers Picked Up,Passengers Dropped Off").unwrap();
        for (id, travel) in &self.vehicle_travel {
            let (pickup, dropoff) = self.vehicle_passengers.get(id).unwrap_or(&(0,0));
            writeln!(vehicle_output_file, "{},{},{},{}", id, travel, pickup, dropoff).unwrap();
        }

        let mut state = State::default();
        create_distributions(&mut state, vec![output_path, output_path_passenger]);
        
        match eframe::run_native("ODBRS_Analytics", NativeOptions::default(), Box::new(|_cc| Box::new(state))) {
            Ok(()) => (),
            Err(err) => panic!("Error: {:?}", err),
        }
    }
}