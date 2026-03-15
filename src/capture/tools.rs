use std::error::Error;
use xcap::Monitor;

pub fn same_monitor(left: &Monitor, right: &Monitor) -> Result<bool, Box<dyn Error>> {
    Ok(left.x()? == right.x()?
        && left.y()? == right.y()?
        && left.width()? == right.width()?
        && left.height()? == right.height()?)
}
