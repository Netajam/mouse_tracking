// src/commands/stats.rs

use crate::persistence; 
use std::path::Path;

pub fn execute(data_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
     println!("Showing statistics...");
     println!("Database path: {:?}", data_path);

     use persistence::calculate_and_print_stats;

     let conn = persistence::open_connection(data_path)?; 

     calculate_and_print_stats(&conn)?;

     Ok(())
}