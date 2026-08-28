const HEADER_LEN: usize = 80;
const TRIANGLE_RECORD_LEN: usize = 50;

pub(crate) const TRIANGLE_COUNT: u32 = 4;
pub(crate) const PAYLOAD_LEN: usize =
    HEADER_LEN + 4 + TRIANGLE_RECORD_LEN * TRIANGLE_COUNT as usize;
pub(crate) const MIME_TYPE: &str = "model/stl";
pub(crate) const UNIT_CODE_VALUE: &str = "mm";
pub(crate) const UNIT_CODING_SCHEME: &str = "UCUM";
pub(crate) const UNIT_CODE_MEANING: &str = "millimeter";

const HEADER: &[u8] = b"dicom-test-suite synthetic closed tetrahedron";
const ROOT_THREE_RECIPROCAL: f32 = 0.577_350_26;

type Point = [f32; 3];

#[derive(Debug, Clone, Copy)]
struct Triangle {
    normal: Point,
    vertices: [Point; 3],
}

const ORIGIN: Point = [0.0, 0.0, 0.0];
const X: Point = [10.0, 0.0, 0.0];
const Y: Point = [0.0, 10.0, 0.0];
const Z: Point = [0.0, 0.0, 10.0];

// Winding is outward for the tetrahedron occupying x >= 0, y >= 0, z >= 0,
// and x + y + z <= 10. Normals are encoded explicitly because viewers and
// independent validators should not need to infer their sign.
const TRIANGLES: [Triangle; TRIANGLE_COUNT as usize] = [
    Triangle {
        normal: [0.0, 0.0, -1.0],
        vertices: [ORIGIN, Y, X],
    },
    Triangle {
        normal: [0.0, -1.0, 0.0],
        vertices: [ORIGIN, X, Z],
    },
    Triangle {
        normal: [-1.0, 0.0, 0.0],
        vertices: [ORIGIN, Z, Y],
    },
    Triangle {
        normal: [ROOT_THREE_RECIPROCAL; 3],
        vertices: [X, Y, Z],
    },
];

pub(crate) fn closed_tetrahedron_binary_stl() -> Vec<u8> {
    let mut payload = Vec::with_capacity(PAYLOAD_LEN);
    payload.extend_from_slice(HEADER);
    payload.resize(HEADER_LEN, 0);
    payload.extend_from_slice(&TRIANGLE_COUNT.to_le_bytes());

    for triangle in TRIANGLES {
        append_point(&mut payload, triangle.normal);
        for vertex in triangle.vertices {
            append_point(&mut payload, vertex);
        }
        payload.extend_from_slice(&0_u16.to_le_bytes());
    }

    debug_assert_eq!(payload.len(), PAYLOAD_LEN);
    payload
}

fn append_point(payload: &mut Vec<u8>, point: Point) {
    for coordinate in point {
        payload.extend_from_slice(&coordinate.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256_hex;

    #[test]
    fn binary_stl_payload_is_exact_and_byte_stable() {
        let first = closed_tetrahedron_binary_stl();
        let second = closed_tetrahedron_binary_stl();

        assert_eq!(first, second);
        assert_eq!(first.len(), PAYLOAD_LEN);
        assert_eq!(&first[..HEADER.len()], HEADER);
        assert!(
            first[HEADER.len()..HEADER_LEN]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(
            u32::from_le_bytes(first[80..84].try_into().unwrap()),
            TRIANGLE_COUNT
        );
        assert_eq!(
            sha256_hex(&first),
            "3c3049d231f8e98c0d2fe7cb81cf6805141bcac39dd04b9cf7f8063ec44bbfb2"
        );
    }

    #[test]
    fn binary_stl_records_have_finite_geometry_and_zero_attributes() {
        let payload = closed_tetrahedron_binary_stl();
        for record in payload[84..].chunks_exact(TRIANGLE_RECORD_LEN) {
            for value in record[..48].chunks_exact(4) {
                let value = f32::from_le_bytes(value.try_into().unwrap());
                assert!(value.is_finite());
            }
            assert_eq!(u16::from_le_bytes(record[48..50].try_into().unwrap()), 0);
        }
    }
}
