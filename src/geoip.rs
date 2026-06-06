use maxminddb::{geoip2, Reader};
use std::net::IpAddr;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct GeoResult {
    pub country_code: String,
    #[allow(dead_code)]
    pub country_name: String,
    pub flag: String,
}

pub struct GeoIp {
    reader: Option<Reader<Vec<u8>>>,
}

impl GeoIp {
    /// Opens the GeoLite2-Country database if it exists, otherwise degrades gracefully
    pub fn new<P: AsRef<Path>>(db_path: P) -> Self {
        let path = db_path.as_ref();
        if !path.exists() {
            eprintln!(
                "⚠️  Warning: GeoIP database not found at '{}'. Geolocation will be disabled.\n\
                 👉 Download a free GeoLite2 Country database from MaxMind and place it there.",
                path.display()
            );
            return Self { reader: None };
        }

        match Reader::open_readfile(path) {
            Ok(reader) => Self { reader: Some(reader) },
            Err(e) => {
                eprintln!(
                    "⚠️  Warning: Failed to load GeoIP database '{}': {}. Geolocation will be disabled.",
                    path.display(),
                    e
                );
                Self { reader: None }
            }
        }
    }

    /// Looks up the IP and returns the country code, name, and emoji flag
    pub fn lookup(&self, ip: IpAddr) -> Option<GeoResult> {
        let reader = self.reader.as_ref()?;
        
        // Skip private and loopback IPs
        if ip.is_loopback() || is_private_ip(ip) {
            return None;
        }

        match reader.lookup(ip) {
            Ok(lookup_result) => {
                match lookup_result.decode::<geoip2::Country>() {
                    Ok(Some(country_rec)) => {
                        let country_info = &country_rec.country;
                        let country_code = country_info.iso_code?.to_string();
                        
                        // Get English name if available, otherwise just use country code
                        let country_name = country_info
                            .names
                            .english
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| country_code.clone());

                        let flag = country_code_to_flag(&country_code);

                        Some(GeoResult {
                            country_code,
                            country_name,
                            flag,
                        })
                    }
                    _ => None,
                }
            }
            Err(_) => None,
        }
    }
}

/// Helper function to check if an IP is in a private/local range
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(ipv6) => {
            // simple check for unique local or link local ipv6
            let octets = ipv6.octets();
            (octets[0] & 0xfe) == 0xfc || (octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80)
        }
    }
}

/// Converts an ISO 3166-1 alpha-2 country code (e.g. "US") to a regional indicator flag emoji
pub fn country_code_to_flag(code: &str) -> String {
    if code.len() != 2 {
        return "🏳".to_string();
    }
    
    let mut flag = String::new();
    for c in code.to_uppercase().chars() {
        if c.is_ascii_alphabetic() {
            // Regional Indicator Symbol Letter A is U+1F1E6.
            // ASCII 'A' is 65. Difference is 127397.
            if let Some(indicator_char) = char::from_u32(c as u32 + 127397) {
                flag.push(indicator_char);
            } else {
                flag.push('🏳');
            }
        } else {
            flag.push('🏳');
        }
    }
    flag
}
