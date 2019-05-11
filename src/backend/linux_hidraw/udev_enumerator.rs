
use crate::backend::ApiDeviceInfo;
use crate::error::{HidResult, ResultExt};
use libudev::{Context, Enumerator as UdevEnumerator, Error as UdevError};
pub struct Enumerator<'a> {
    udev_ctx: &'a Context,
    enumerator: UdevEnumerator<'a>,
}

impl<'a> Enumerator<'a> {
    pub fn create(udev: &'a Context) -> HidResult<Self> {
        let mut enumerator = UdevEnumerator::new(udev).convert()?;

        enumerator.match_subsystem("hidraw").convert()?;

        Ok(Self {
            udev_ctx: udev,
            enumerator,
        })
    }

    pub fn enumerate<'b>(&'b mut self) -> HidResult<(impl Iterator<Item = DeviceInfo> + 'b)> {
        let devices = self.enumerator.scan_devices().convert()?;
        let device_info = devices.map(|d| DeviceInfo::from(d));
        Ok(device_info)
    }
}

pub struct DeviceInfo {
    path: Option<String>,
}

impl<'a> From<libudev::Device<'a>> for DeviceInfo {
    fn from(dev: libudev::Device<'a>) -> Self {
        Self {
            path: dev
                .devnode()
                .iter()
                .flat_map(|p| p.to_str().map(|s| s.to_owned()))
                .next(),
        }
    }
}


impl ApiDeviceInfo for DeviceInfo {
    fn path(&self) -> Option<String> {
        self.path.clone()
    }
    fn vendor_id(&self) -> u16 {
        unimplemented!()
    }
    fn product_id(&self) -> u16 {
        unimplemented!()
    }
    fn serial_number(&self) -> Option<String> {
        unimplemented!()
    }
    fn release_number(&self) -> u16 {
        unimplemented!()
    }
    fn manufacturer_string(&self) -> Option<String> {
        unimplemented!()
    }
    fn product_string(&self) -> Option<String> {
        unimplemented!()
    }
    fn usage_page(&self) -> Option<u16> {
        unimplemented!()
    }
    fn usage(&self) -> u16 {
        unimplemented!()
    }
    fn interface_number(&self) -> i32 {
        unimplemented!()
    }
}

// Some debugging utilities (implement fmt::Debug for external types)
// ------------------------------------------------------------------

struct UdevDevice<'a>(&'a libudev::Device<'a>);
struct Property<'a>(libudev::Property<'a>);
struct Attribute<'a>(libudev::Attribute<'a>);

impl<'a> std::fmt::Debug for UdevDevice<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_list()
            .entries(self.0.attributes().map(|a| Attribute(a)))
            .entries(self.0.properties().map(|p| Property(p)))
            .finish()  
    }
}
impl<'a> std::fmt::Debug for Property<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Property {{name: {}, value: {}}}", 
            self.0.name().to_string_lossy(), self.0.value().to_string_lossy())
    }
}
impl<'a> std::fmt::Debug for Attribute<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Attribute {{name: {}, value: {}}}", 
            self.0.name().to_string_lossy(), 
            self.0.value().map(|v| v.to_string_lossy()).
            unwrap_or(std::borrow::Cow::Borrowed("undefined")))
    }
}

#[cfg(test)]
mod test {
    use super::Enumerator;
    use libudev::Context;
    use crate::backend::ApiDeviceInfo;

    #[test]
    fn test_enumeration() {
        let udev = Context::new().unwrap();
        let mut e = Enumerator::create(&udev).unwrap();

        for dev in e.enumerate().unwrap() {
            println!("{}", dev.path().unwrap_or("<undefined>".to_string()));
        }
    }
}