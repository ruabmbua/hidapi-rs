use super::error::{Error, Result};
use udev::{Device, Enumerator};

pub struct UdevHidDeviceEnumerator {
    enumerator: Enumerator,
}

impl UdevHidDeviceEnumerator {
    pub fn new() -> Result<Self> {
        let mut enumerator = Enumerator::new().map_err(|e| Error::UdevError(e))?;
        
        enumerator.match_subsystem("hidraw")?;
        
        Ok(Self { enumerator })
    }
}
