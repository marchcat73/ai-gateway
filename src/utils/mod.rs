// В том же файле или в utils/mod.rs

pub fn extract_site_name(site_key: &str) -> String {
    site_key
        .split('.')
        .nth(0)
        .unwrap_or(site_key)
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 { c.to_uppercase().collect::<String>() }
            else { c.to_string() }
        })
        .collect()
}

pub fn normalize_site_url(site_key: &str) -> String {
    if site_key.starts_with("http") {
        site_key.to_string()
    } else {
        format!("https://{}", site_key)
    }
}
