use serde::Deserialize;

use crate::Module;

/// Gui contains the GUI for the app obviously
/// - Function for view of the app
/// - Pan and Zoom capabilities
/// - Control over functions
/// - etc. display functionality
/// 
#[derive(Default, Clone)]
pub struct App {

}

impl Module for App {
    type Configuration = GuiConfig;
    type ReturnType = ();
    type Parameters = ();
    fn get_name(&self) -> &str {
        "App"
    }

    fn init(&mut self, config: Self::Configuration, parameters: Self::Parameters) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();
        
        Ok(println!("[{}] Initialised in {:?}", self.get_name(), time.elapsed()))
    }
}

#[derive(Default, Deserialize)]
pub struct GuiConfig {
    test: String
}