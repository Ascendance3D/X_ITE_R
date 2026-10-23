//! Ports of X_ITE's `standard/Geospatial` (ReferenceEllipsoids, Geodetic, UniversalTransverseMercator)
//! and `x_ite/Browser/Geospatial` (Geocentric, GeospatialObject).

use x3d_fields::jsmath;
use x3d_fields::math::{Matrix4, Vector3};

/// standard/Math/Geometry/Spheroid3.js.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spheroid {
    pub semi_major_axis: f64,
    pub semi_minor_axis: f64,
}

impl Spheroid {
    /// `new Spheroid3 (a, c)`.
    pub const fn new(a: f64, c: f64) -> Self {
        Spheroid {
            semi_major_axis: a,
            semi_minor_axis: c,
        }
    }

    /// `new Spheroid3 (a, f_1, true)`: the minor axis from the inverse flattening.
    pub fn from_f_1(a: f64, f_1: f64) -> Self {
        Spheroid {
            semi_major_axis: a,
            semi_minor_axis: a * (1.0 - (1.0 / f_1)),
        }
    }
}

/// ReferenceEllipsoids.js.
pub fn reference_ellipsoid(name: &str) -> Option<Spheroid> {
    let f = Spheroid::from_f_1;

    Some(match name {
        "AA" => f(6377563.396, 299.3249646),
        "AM" => f(6377340.189, 299.3249646),
        "AN" => f(6378160.0, 298.25),
        "BN" => f(6377483.865, 299.1528128),
        "BR" => f(6377397.155, 299.1528128),
        "CC" => f(6378206.4, 294.9786982),
        "CD" => f(6378249.145, 293.465),
        "EA" => f(6377276.345, 300.8017),
        "EB" => f(6377298.556, 300.8017),
        "EC" => f(6377301.243, 300.8017),
        "ED" => f(6377295.664, 300.8017),
        "EE" => f(6377304.063, 300.8017),
        "EF" => f(6377309.613, 300.8017),
        "FA" => f(6378155.0, 298.3),
        "HE" => f(6378200.0, 298.3),
        "HO" => f(6378270.0, 297.0),
        "ID" => f(6378160.0, 298.247),
        "IN" => f(6378388.0, 297.0),
        "KA" => f(6378245.0, 298.3),
        "RF" => f(6378137.0, 298.257222101),
        "SA" => f(6378160.0, 298.25),
        "WD" => f(6378135.0, 298.26),
        "WE" => f(6378137.0, 298.257223563),
        "SUN" => f(696342000.0, 1.0 / 9e-6),
        "MERCURY" => Spheroid::new(2439700.0, 2439700.0),
        "VENUS" => Spheroid::new(6051800.0, 6051800.0),
        "MOON" => Spheroid::new(1738140.0, 1735970.0),
        "MARS" => Spheroid::new(3395428.0, 3377678.0),
        "JUPITER" => Spheroid::new(71492000.0, 66854000.0),
        "SATURN" => Spheroid::new(60268000.0, 54364000.0),
        "URANUS" => Spheroid::new(2555000.0, 24973000.0),
        "NEPTUNE" => Spheroid::new(24764000.0, 24341000.0),
        "PLUTO" => Spheroid::new(1153000.0, 1153000.0),
        _ => return None,
    })
}

const EPS_H: f64 = 1e-3;
const EPS_P: f64 = 1e-10;
const IMAX: usize = 30;

/// Geodetic.js.
#[derive(Clone, Copy, Debug)]
pub struct Geodetic {
    longitude_first: bool,
    degrees: bool,
    a: f64,
    c: f64,
    c2a2: f64,
    ecc2: f64,
}

impl Geodetic {
    pub fn new(spheroid: Spheroid, latitude_first: bool, radians: bool) -> Self {
        let a = spheroid.semi_major_axis;
        let c2a2 = jsmath::pow(spheroid.semi_minor_axis / a, 2.0);

        Geodetic {
            longitude_first: !latitude_first,
            degrees: !radians,
            a,
            c: spheroid.semi_minor_axis,
            c2a2,
            ecc2: 1.0 - c2a2,
        }
    }

    pub fn convert(&self, geodetic: &Vector3) -> Vector3 {
        let elevation = geodetic.z;
        let (mut latitude, mut longitude) = if self.longitude_first {
            (geodetic.y, geodetic.x)
        } else {
            (geodetic.x, geodetic.y)
        };

        if self.degrees {
            latitude *= std::f64::consts::PI / 180.0;
            longitude *= std::f64::consts::PI / 180.0;
        }

        self.convert_radians(latitude, longitude, elevation)
    }

    pub fn convert_radians(&self, latitude: f64, longitude: f64, elevation: f64) -> Vector3 {
        let slat = jsmath::sin(latitude);
        let slat2 = jsmath::pow(slat, 2.0);
        let clat = jsmath::cos(latitude);
        let n = self.a / (1.0 - self.ecc2 * slat2).sqrt();
        let nhl = (n + elevation) * clat;

        Vector3::new(
            nhl * jsmath::cos(longitude),
            nhl * jsmath::sin(longitude),
            (n * self.c2a2 + elevation) * slat,
        )
    }

    pub fn apply(&self, geocentric: &Vector3) -> Vector3 {
        let mut result = self.apply_radians(geocentric);

        if self.degrees {
            result.x *= 180.0 / std::f64::consts::PI;
            result.y *= 180.0 / std::f64::consts::PI;
        }

        if self.longitude_first {
            std::mem::swap(&mut result.x, &mut result.y);
        }

        result
    }

    pub fn apply_radians(&self, geocentric: &Vector3) -> Vector3 {
        let (x, y, z) = (geocentric.x, geocentric.y, geocentric.z);
        let p = (x * x + y * y).sqrt();

        // Handle pole case.
        if p == 0.0 {
            return Vector3::new(std::f64::consts::PI, 0.0, z - self.c);
        }

        let mut latitude = 0.0;
        let longitude = jsmath::atan2(y, x);
        let mut elevation = 0.0;
        let a = self.a;
        let mut n = a;
        let ecc2 = self.ecc2;

        for _ in 0..IMAX {
            let h0 = elevation;
            let b0 = latitude;

            latitude = jsmath::atan(z / p / (1.0 - ecc2 * n / (n + elevation)));

            let sin_p = jsmath::sin(latitude);

            n = a / (1.0 - ecc2 * sin_p * sin_p).sqrt();
            elevation = p / jsmath::cos(latitude) - n;

            if (elevation - h0).abs() < EPS_H && (latitude - b0).abs() < EPS_P {
                break;
            }
        }

        Vector3::new(latitude, longitude, elevation)
    }

    pub fn normal(&self, geocentric: &Vector3) -> Vector3 {
        let geodetic = self.apply_radians(geocentric);
        let (latitude, longitude) = (geodetic.x, geodetic.y);
        let clat = jsmath::cos(latitude);

        Vector3::new(
            jsmath::cos(longitude) * clat,
            jsmath::sin(longitude) * clat,
            jsmath::sin(latitude),
        )
    }
}

const N0: f64 = 1.0e7;
const E0: f64 = 5.0e5;
const K0: f64 = 0.9996;

/// UniversalTransverseMercator.js.
#[derive(Clone, Copy, Debug)]
pub struct Utm {
    southern_hemisphere: bool,
    easting_first: bool,
    a: f64,
    ecc2: f64,
    ee: f64,
    e8: f64,
    e9: f64,
    e252: f64,
    big_a: f64,
    big_b: f64,
    big_c: f64,
    big_d: f64,
    big_e: f64,
    big_w: f64,
    big_x: f64,
    big_y: f64,
    big_z: f64,
    longitude0: f64,
    geodetic: Geodetic,
}

impl Utm {
    pub fn new(
        spheroid: Spheroid,
        zone: i64,
        northern_hemisphere: bool,
        northing_first: bool,
    ) -> Self {
        let a = spheroid.semi_major_axis;
        let ecc2 = 1.0 - jsmath::pow(spheroid.semi_minor_axis / a, 2.0);
        let ee = ecc2 / (1.0 - ecc2);
        let e1 = (1.0 - (1.0 - ecc2).sqrt()) / (1.0 + (1.0 - ecc2).sqrt());

        Utm {
            southern_hemisphere: !northern_hemisphere,
            easting_first: !northing_first,
            a,
            ecc2,
            ee,
            e8: 8.0 * ee,
            e9: 9.0 * ee,
            e252: 252.0 * ee,
            big_a: K0
                * (a * (1.0
                    - ecc2 / 4.0
                    - 3.0 * ecc2 * ecc2 / 64.0
                    - 5.0 * ecc2 * ecc2 * ecc2 / 256.0)),
            big_b: 3.0 * e1 / 2.0 - 7.0 * e1 * e1 * e1 / 32.0,
            big_c: 21.0 * e1 * e1 / 16.0 - 55.0 * e1 * e1 * e1 * e1 / 32.0,
            big_d: 151.0 * e1 * e1 * e1 / 96.0,
            big_e: a * (1.0 - ecc2),
            big_w: 1.0 - ecc2 / 4.0 - 3.0 * ecc2 * ecc2 / 64.0 - 5.0 * ecc2 * ecc2 * ecc2 / 256.0,
            big_x: 3.0 * ecc2 / 8.0 + 3.0 * ecc2 * ecc2 / 32.0 + 45.0 * ecc2 * ecc2 * ecc2 / 1024.0,
            big_y: 15.0 * ecc2 * ecc2 / 256.0 + 45.0 * ecc2 * ecc2 * ecc2 / 1024.0,
            big_z: 35.0 * ecc2 * ecc2 * ecc2 / 3072.0,
            longitude0: (zone as f64 * 6.0 - 183.0) * (std::f64::consts::PI / 180.0),
            geodetic: Geodetic::new(spheroid, true, true),
        }
    }

    pub fn convert(&self, utm: &Vector3) -> Vector3 {
        let (mut northing, mut easting) = if self.easting_first {
            (utm.y, utm.x)
        } else {
            (utm.x, utm.y)
        };

        // Check for southern hemisphere and remove offset from easting.
        let mut s = self.southern_hemisphere;

        if northing < 0.0 {
            s = !self.southern_hemisphere;
            northing = -northing;
        }

        if s {
            northing -= N0;
        }

        easting -= E0;

        let mu = northing / self.big_a;
        let phi1 = mu
            + self.big_b * jsmath::sin(2.0 * mu)
            + self.big_c * jsmath::sin(4.0 * mu)
            + self.big_d * jsmath::sin(6.0 * mu);
        let sinphi1 = jsmath::pow(jsmath::sin(phi1), 2.0);
        let cosphi1 = jsmath::cos(phi1);
        let tanphi1 = jsmath::tan(phi1);
        let n1 = self.a / (1.0 - self.ecc2 * sinphi1).sqrt();
        let t2 = jsmath::pow(tanphi1, 2.0);
        let t8 = jsmath::pow(tanphi1, 8.0);
        let c1 = self.ee * t2;
        let c2 = c1 * c1;
        let r1 = self.big_e / jsmath::pow(1.0 - self.ecc2 * sinphi1, 1.5);
        let i = easting / (n1 * K0);
        let j = (5.0 + 3.0 * t2 + 10.0 * c1 - 4.0 * c2 - self.e9) * jsmath::pow(i, 4.0) / 24.0;
        let k = (61.0 + 90.0 * t2 + 298.0 * c1 + 45.0 * t8 - self.e252 - 3.0 * c2)
            * jsmath::pow(i, 6.0)
            / 720.0;
        let l = (5.0 - 2.0 * c1 + 28.0 * t2 - 3.0 * c2 + self.e8 + 24.0 * t8) * jsmath::pow(i, 5.0)
            / 120.0;
        let latitude = phi1 - (n1 * tanphi1 / r1) * (i * i / 2.0 - j + k);
        let longitude =
            self.longitude0 + (i - (1.0 + 2.0 * t2 + c1) * jsmath::pow(i, 3.0) / 6.0 + l) / cosphi1;

        self.geodetic.convert_radians(latitude, longitude, utm.z)
    }

    pub fn apply(&self, geocentric: &Vector3) -> Vector3 {
        let geodetic = self.geodetic.apply_radians(geocentric);
        let (latitude, longitude) = (geodetic.x, geodetic.y);
        let tanlat = jsmath::tan(latitude);
        let coslat = jsmath::cos(latitude);
        let ee = self.ee;
        let n = self.a / (1.0 - self.ecc2 * jsmath::pow(jsmath::sin(latitude), 2.0)).sqrt();
        let t = tanlat * tanlat;
        let t6 = t * t * t;
        let c = ee * coslat * coslat;
        let a = coslat * (longitude - self.longitude0);
        let m = self.a
            * (self.big_w * latitude - self.big_x * jsmath::sin(2.0 * latitude)
                + self.big_y * jsmath::sin(4.0 * latitude)
                - self.big_z * jsmath::sin(6.0 * latitude));
        let easting = K0
            * n
            * (a + (1.0 - t + c) * jsmath::pow(a, 3.0) / 6.0
                + (5.0 - 18.0 * t6 + 72.0 * c - 58.0 * ee) * jsmath::pow(a, 5.0) / 120.0)
            + E0;
        let mut northing = K0
            * (m + n
                * tanlat
                * (a * a / 2.0
                    + (5.0 - t + 9.0 * c + 4.0 * c * c) * jsmath::pow(a, 4.0) / 24.0
                    + (61.0 - 58.0 * t6 + 600.0 * c - 330.0 * ee) * jsmath::pow(a, 6.0) / 720.0));

        if latitude < 0.0 {
            northing += N0;

            if !self.southern_hemisphere {
                northing = -northing;
            }
        } else if self.southern_hemisphere {
            northing = -northing;
        }

        if self.easting_first {
            Vector3::new(easting, northing, geodetic.z)
        } else {
            Vector3::new(northing, easting, geodetic.z)
        }
    }
}

/// The reference frame of a `geoSystem` (GeospatialObject.getReferenceFrame).
#[derive(Clone, Copy, Debug)]
pub enum ReferenceFrame {
    Gd(Geodetic),
    Utm(Utm),
    Gc,
}

/// GeospatialObject coordinate system kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinateSystem {
    Gd,
    Utm,
    Gc,
}

impl ReferenceFrame {
    pub fn convert(&self, v: &Vector3) -> Vector3 {
        match self {
            ReferenceFrame::Gd(g) => g.convert(v),
            ReferenceFrame::Utm(u) => u.convert(v),
            ReferenceFrame::Gc => *v,
        }
    }

    pub fn apply(&self, v: &Vector3) -> Vector3 {
        match self {
            ReferenceFrame::Gd(g) => g.apply(v),
            ReferenceFrame::Utm(u) => u.apply(v),
            ReferenceFrame::Gc => *v,
        }
    }
}

/// x_ite/Browser/Geospatial/GeospatialObject.js.
pub mod geo_system {
    use super::*;

    pub fn coordinate_system(geo_system: &[String]) -> CoordinateSystem {
        for gs in geo_system {
            match gs.as_str() {
                "GD" | "GDC" => return CoordinateSystem::Gd,
                "UTM" => return CoordinateSystem::Utm,
                "GC" | "GCC" | "GS" => return CoordinateSystem::Gc,
                _ => {}
            }
        }

        CoordinateSystem::Gd
    }

    pub fn ellipsoid(geo_system: &[String]) -> Spheroid {
        geo_system
            .iter()
            .find_map(|gs| reference_ellipsoid(gs))
            .unwrap_or_else(|| reference_ellipsoid("WE").unwrap())
    }

    pub fn latitude_first(geo_system: &[String]) -> bool {
        !geo_system.iter().any(|gs| gs == "longitude_first")
    }

    pub fn northing_first(geo_system: &[String]) -> bool {
        !geo_system.iter().any(|gs| gs == "easting_first")
    }

    /// `/^Z(\d+)$/` then parseInt.
    pub fn zone(geo_system: &[String]) -> i64 {
        for gs in geo_system {
            if let Some(d) = gs.strip_prefix('Z') {
                if !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()) {
                    return d.parse::<f64>().unwrap_or(0.0) as i64;
                }
            }
        }

        1
    }

    pub fn northern_hemisphere(geo_system: &[String]) -> bool {
        !geo_system.iter().any(|gs| gs == "S")
    }

    pub fn standard_order(geo_system: &[String]) -> bool {
        match coordinate_system(geo_system) {
            CoordinateSystem::Gd => latitude_first(geo_system),
            CoordinateSystem::Utm => northing_first(geo_system),
            CoordinateSystem::Gc => true,
        }
    }

    pub fn reference_frame(geo_system: &[String], radians: bool) -> ReferenceFrame {
        match coordinate_system(geo_system) {
            CoordinateSystem::Gd => ReferenceFrame::Gd(Geodetic::new(
                ellipsoid(geo_system),
                latitude_first(geo_system),
                radians,
            )),
            CoordinateSystem::Utm => ReferenceFrame::Utm(Utm::new(
                ellipsoid(geo_system),
                zone(geo_system),
                northern_hemisphere(geo_system),
                northing_first(geo_system),
            )),
            CoordinateSystem::Gc => ReferenceFrame::Gc,
        }
    }

    pub fn elevation_frame(geo_system: &[String], radians: bool) -> Geodetic {
        Geodetic::new(ellipsoid(geo_system), true, radians)
    }
}

/// The GeoOrigin a geospatial node uses (X3DGeospatialObject set_geoOrigin__).
#[derive(Clone, Copy, Debug)]
pub struct GeoOriginInfo {
    /// GeoOrigin.getOrigin (): `geoCoords` in the GeoOrigin's own reference frame.
    pub origin: Vector3,
    pub rotate_y_up: bool,
}

/// Components/Geospatial/X3DGeospatialObject.js: the per-node reference frames and origin.
#[derive(Clone, Copy, Debug)]
pub struct GeospatialObject {
    pub coordinate_system: CoordinateSystem,
    pub reference_frame: ReferenceFrame,
    pub elevation_frame: Geodetic,
    pub standard_order: bool,
    pub origin: Vector3,
    origin_matrix: Matrix4,
    inv_origin_matrix: Matrix4,
    rotate_y_up: bool,
}

/// The orientation that keeps +Y gravitational up at `t` (getStandardLocationMatrix / set_originMatrix__).
fn location_frame(elevation_frame: &Geodetic, t: &Vector3) -> Matrix4 {
    let y = elevation_frame.normal(t);
    let mut x = Vector3::new(0.0, 0.0, 1.0);
    x.cross(&y);

    // Handle pole cases.
    if x.equals(&Vector3::ZERO) {
        x = Vector3::new(1.0, 0.0, 0.0);
    }

    let mut z = x;
    z.cross(&y);
    x.normalize();
    z.normalize();

    Matrix4([
        x.x, x.y, x.z, 0.0, y.x, y.y, y.z, 0.0, z.x, z.y, z.z, 0.0, t.x, t.y, t.z, 1.0,
    ])
}

impl GeospatialObject {
    pub fn new(geo_system: &[String], geo_origin: Option<GeoOriginInfo>, radians: bool) -> Self {
        let elevation_frame = geo_system::elevation_frame(geo_system, radians);
        let mut obj = GeospatialObject {
            coordinate_system: geo_system::coordinate_system(geo_system),
            reference_frame: geo_system::reference_frame(geo_system, radians),
            elevation_frame,
            standard_order: geo_system::standard_order(geo_system),
            origin: Vector3::ZERO,
            origin_matrix: Matrix4::IDENTITY,
            inv_origin_matrix: Matrix4::IDENTITY,
            rotate_y_up: false,
        };

        if let Some(o) = geo_origin {
            obj.origin = o.origin;
            obj.origin_matrix = location_frame(&elevation_frame, &o.origin);
            obj.inv_origin_matrix = obj.origin_matrix;
            obj.inv_origin_matrix.inverse();
            obj.rotate_y_up = o.rotate_y_up;
        }

        obj
    }

    pub fn get_coord(&self, geo_point: &Vector3) -> Vector3 {
        let mut p = self.reference_frame.convert(geo_point);

        if self.rotate_y_up {
            self.inv_origin_matrix.mult_vec_matrix3(&mut p);
        } else {
            p.subtract(&self.origin);
        }

        p
    }

    pub fn get_geo_coord(&self, point: &Vector3) -> Vector3 {
        let mut v = *point;

        if self.rotate_y_up {
            self.origin_matrix.mult_vec_matrix3(&mut v);
        } else {
            v.add(&self.origin);
        }

        self.reference_frame.apply(&v)
    }

    pub fn get_geo_elevation(&self, point: &Vector3) -> f64 {
        self.get_geo_coord(point).z
    }

    pub fn get_geo_up_vector(&self, point: &Vector3) -> Vector3 {
        let mut v = *point;

        if self.rotate_y_up {
            self.origin_matrix.mult_vec_matrix3(&mut v);
            let mut n = self.elevation_frame.normal(&v);
            self.inv_origin_matrix.mult_dir_matrix(&mut n);
            n
        } else {
            v.add(&self.origin);
            self.elevation_frame.normal(&v)
        }
    }

    pub fn get_location_matrix(&self, geo_point: &Vector3) -> Matrix4 {
        let t = self.reference_frame.convert(geo_point);
        let mut m = location_frame(&self.elevation_frame, &t);

        if self.rotate_y_up {
            m.mult_right(&self.inv_origin_matrix);
        } else {
            m.0[12] -= self.origin.x;
            m.0[13] -= self.origin.y;
            m.0[14] -= self.origin.z;
        }

        m
    }
}

/// Geocentric.slerp: interpolates direction on the sphere and length linearly.
pub fn geocentric_slerp(mut source: Vector3, mut destination: Vector3, t: f64) -> Vector3 {
    let source_length = source.norm();
    let destination_length = destination.norm();

    source.normalize();
    destination.normalize();

    // Algorithm.simpleSlerp (no short-path flip).
    let cosom = source.dot(&destination);

    if cosom < 1.0 {
        let omega = jsmath::acos(cosom);
        let sinom = jsmath::sin(omega);
        let scale0 = jsmath::sin((1.0 - t) * omega) / sinom;
        let scale1 = jsmath::sin(t * omega) / sinom;

        source.x = source.x * scale0 + destination.x * scale1;
        source.y = source.y * scale0 + destination.y * scale1;
        source.z = source.z * scale0 + destination.z * scale1;
    }

    source.multiply(source_length + t * (destination_length - source_length));
    source
}
