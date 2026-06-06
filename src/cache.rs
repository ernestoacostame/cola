use crate::geoip::GeoResult;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::RwLock;

pub struct IpCache {
    cache: RwLock<HashMap<IpAddr, Option<GeoResult>>>,
}

impl IpCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Retrieve an IP lookup result from cache if present.
    /// Returns `Some(Some(GeoResult))` if geolocated,
    /// `Some(None)` if lookup was tried but returned no result,
    /// and `None` if the IP is not in the cache yet.
    pub fn get(&self, ip: IpAddr) -> Option<Option<GeoResult>> {
        let read_guard = self.cache.read().ok()?;
        read_guard.get(&ip).cloned()
    }

    /// Insert an IP lookup result into the cache.
    pub fn insert(&self, ip: IpAddr, result: Option<GeoResult>) {
        if let Ok(mut write_guard) = self.cache.write() {
            write_guard.insert(ip, result);
        }
    }

    /// Clear the cache
    #[allow(dead_code)]
    pub fn clear(&self) {
        if let Ok(mut write_guard) = self.cache.write() {
            write_guard.clear();
        }
    }

    /// Returns cache statistics (total entries)
    pub fn size(&self) -> usize {
        self.cache.read().map(|g| g.len()).unwrap_or(0)
    }
}
