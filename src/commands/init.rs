use crate::error::Result;
use crate::state;

pub fn execute() -> Result<()> {
    state::initialize()?;
    println!("Initialized wortex at ~/.wortex");
    Ok(())
}
