use eframe::egui::{Context, plot::{Plot, BarChart, Bar}, CentralPanel};
use csv::ReaderBuilder;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    distributions: Vec<(String, HashMap<u64, usize>)>,
    selected_distribution: Option<usize>,
}

impl eframe::App for State {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        show_analytics(self, ctx, _frame);
    }
}

pub fn create_distributions(state: &mut State, paths: Vec<String>) {
    let mut distr = vec![];
    for path in paths {
        let distributions = read_csv_file(&path).unwrap();
        distr.extend(distributions.into_iter());
    }
    state.distributions = distr;
}

pub fn show_analytics(state: &mut State, ctx: &Context, _frame: &mut eframe::Frame) {
    // let distributions = read_csv_file("data/agent_distributions.csv").unwrap();
    
    CentralPanel::default().show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (i, (name, _)) in state.distributions.iter().enumerate() {
                if ui.small_button(format!("{}", name)).clicked() {
                    state.selected_distribution = Some(i);
                }
            }    
        });

        if let Some(selected_distribution) = state.selected_distribution {
            let (name, dist) = &state.distributions.get(selected_distribution).unwrap();
            
            let (min, q1, _med, q3, max) = calculate_stats(&dist).unwrap();
            let (mean, stdev) = calculate_mean_and_stdev(&dist).unwrap();
            let iqr = q3 - q1;
            let range = max - min;
            let h = freedman_diaconis(iqr as f64, dist.len()); // bar width

            ui.heading(format!("Distribution of {}", name));
            ui.label(format!("Min: {} Max: {} IQR: {} Median: {} Mean: {} StDev: {}", min, max, iqr, _med, mean, stdev));
            
            // This sometimes returns an inf (or usize::MAX) probably should handle that!
            let bar_count = (range as f64 / h).ceil() as usize; // bar count
            if bar_count == usize::MAX {
                ui.label("Error Displaying Distribution Chart");
                return;
            }

            let bars = BarChart::new((0..bar_count).map(|i| {
                let position = min as f64 + (i as f64 * h);
                let next_position = min as f64 + ((i + 1) as f64 * h);

                let mut height = 0.0;
                let bound_low = position.floor() as u64;
                let bound_high = next_position.ceil() as u64;
                for value in bound_low..bound_high {
                    if let Some(count) = dist.get(&value) {
                        height += *count as f64;
                    }
                }
                // for (value, count) in dist.iter() {
                //     if *value as f64 >= position && (*value as f64) < next_position {
                //         height += *count as f64;
                //         break;
                //     }
                // }
                
                Bar::new(position + h/2.0, height).width(h)
                // Bar::new(bound_low as f64 + (bound_high - bound_low) as f64 / 2.0, height).width(bound_high as f64 - bound_low as f64)
            }).collect());
            
            Plot::new("analytics_plot").auto_bounds_x().auto_bounds_y().show(ui, |plot_ui| {
                plot_ui.bar_chart(bars)
            });
        } else {
            ui.label("Select a distribution");
        }
    });
}

pub fn freedman_diaconis(iqr: f64, n: usize) -> f64 {
    let h = 2.0 * iqr / (n as f64).powf(1.0 / 3.0);
    h
}

fn read_csv_file(file_path: &str) -> Result<Vec<(String, HashMap<u64, usize>)>, Box<dyn std::error::Error>> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path)?;

    let headers = reader.headers()?.clone();

    // let mut freq_dists: Vec<HashMap<u64, usize>> = vec![HashMap::new(); headers.len()];
    let mut freq_dists: Vec<(String, HashMap<u64, usize>)> = headers.into_iter().map(|v| (v.to_string(), HashMap::new())).collect();

    for result in reader.records() {
        let record = result?;
        
        for (i, value) in record.iter().enumerate() {
            let freq_dist = &mut freq_dists.get_mut(i).unwrap().1;
            let num_value = value.parse::<u64>()?;
            let count = freq_dist.entry(num_value).or_insert(0);
            *count += 1;
        }
    }

    Ok(freq_dists)
}

fn calculate_stats(freq_dist: &HashMap<u64, usize>) -> Option<(u64, u64, u64, u64, u64)> {
    let mut values: Vec<u64> = Vec::new();
    let mut total_count = 0usize;

    for (value, count) in freq_dist.iter() {
        values.resize(values.len() + *count, *value);
        total_count += count;
    }

    values.sort_unstable();

    if total_count == 0 {
        return None; // no observations in the frequency distribution
    }

    let min = values[0];
    let max = values[total_count - 1];

    let n = total_count;
    let q1_index = (n as f64 * 0.25).ceil() as usize - 1;
    let q3_index = (n as f64 * 0.75).ceil() as usize - 1;
    let q1 = values[q1_index];
    let q3 = values[q3_index];

    let median = if n % 2 == 0 {
        (values[n/2 - 1] + values[n/2]) / 2
    } else {
        values[n/2]
    };

    Some((min, q1, median, q3, max))
}

fn calculate_mean_and_stdev(freq_dist: &HashMap<u64, usize>) -> Option<(f64, f64)> {
    let mut total_count = 0usize;
    let mut sum = 0f64;
    let mut sum_squared = 0f64;

    for (value, count) in freq_dist.iter() {
        let value_f64 = *value as f64;
        sum += value_f64 * (*count as f64);
        sum_squared += value_f64.powi(2) * (*count as f64);
        total_count += count;
    }

    if total_count == 0 {
        return None; // no observations in the frequency distribution
    }

    let mean = sum / (total_count as f64);
    let variance = (sum_squared / (total_count as f64)) - (mean.powi(2));
    let stdev = variance.sqrt();

    Some((mean, stdev))
}