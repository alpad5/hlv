use rand_distr::{Distribution, Normal};

const EARTH_RADIUS_KM: f64 = 6371.0;

const GRID_METERS: f64 = 100.0;
const METERS_PER_DEGREE_LAT: f64 = 111_320.0;

/// Apply two-layer location fuzzing:
/// 1. Snap to a ~100m grid to remove sub-grid precision
/// 2. Add Gaussian noise with given sigma (meters)
pub fn fuzz_coordinates(lat: f64, lng: f64, noise_sigma_meters: f64) -> (f64, f64) {
    let mut rng = rand::thread_rng();

    // Layer 1: grid snap
    let grid_deg_lat = GRID_METERS / METERS_PER_DEGREE_LAT;
    let grid_deg_lng = GRID_METERS / (METERS_PER_DEGREE_LAT * lat.to_radians().cos());
    let snapped_lat = (lat / grid_deg_lat).round() * grid_deg_lat;
    let snapped_lng = (lng / grid_deg_lng).round() * grid_deg_lng;

    // Layer 2: Gaussian jitter
    let normal = Normal::new(0.0, noise_sigma_meters).unwrap();
    let jitter_lat = normal.sample(&mut rng) / METERS_PER_DEGREE_LAT;
    let jitter_lng =
        normal.sample(&mut rng) / (METERS_PER_DEGREE_LAT * snapped_lat.to_radians().cos());

    (snapped_lat + jitter_lat, snapped_lng + jitter_lng)
}

/// Haversine distance in kilometers between two lat/lng points.
pub fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlng / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}
