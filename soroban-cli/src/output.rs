use std::fmt;

use crate::commands::{
    invoke::{InvokeCmd, InvokeOutput},
    simulate::{SimCost, SimulateOutput},
};

impl fmt::Display for SimCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "  CPU Instructions: {}\n  Memory Bytes: {}\n  Read Bytes: {}\n  Write Bytes: {}\n  Historical Data Bytes: {}\n  Footprint Bytes: {}",
            self.cpu_instructions,
            self.mem_bytes,
            self.read_bytes,
            self.write_bytes,
            self.historical_data_bytes,
            self.footprint_bytes,
        )
    }
}

impl fmt::Display for SimulateOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Simulation Cost:")?;
        write!(f, "{}", self.cost)?;
        
        if let Some(ref err) = self.error {
            writeln!(f, "\nError: {}", err)?;
        }
        
        if !self.results.is_empty() {
            writeln!(f, "\nResults:")?;
            for (i, result) in self.results.iter().enumerate() {
                writeln!(f, "  Instruction {}:", i + 1)?;
                if let Some(val) = &result.results.first() {
                    writeln!(f, "    Return Value: {:?}", val)?;
                }
                if !result.events.is_empty() {
                    writeln!(f, "    Events:")?;
                    for event in &result.events {
                        writeln!(f, "      {}", event)?;
                    }
                }
            }
        }
        
        Ok(())
    }
}

// Similar Display implementations for other output types...
