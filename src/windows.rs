use windows_sys::core::GUID;
use crate::{HidDevice, HidResult};

impl HidDevice {
    /// Get the container ID for a HID device.
    ///
    /// This function returns the `DEVPKEY_Device_ContainerId` property of the
    /// given device. This can be used to correlate different interfaces/ports
    /// on the same hardware device.
    pub fn get_container_id(&self) -> HidResult<GUID> {
        self.inner.get_container_id()
    }
}
