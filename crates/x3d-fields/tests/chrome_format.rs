//! Intl.NumberFormat ("en", { maximumSignificantDigits }) as Chrome 155 prints it (ICU's approximate-double
//! fast path included), captured in headless Chrome; see oracle/README.md, "Followers".

use x3d_fields::format::intl_standard;

const CASES: &[(u64, usize, &str)] = &[
    (0x3fefffffffffffff, 15, "1"),
    (0x3fefffffffffffff, 16, "1"),
    (0x3feffffffffffffe, 15, "1"),
    (0x3feffffffffffffe, 16, "1"),
    (0x3feffffffffffffd, 15, "1"),
    (0x3feffffffffffffd, 16, "1"),
    (0x3feffffffffffffc, 15, "1"),
    (0x3feffffffffffffc, 16, "1"),
    (0x3feffffffffffffb, 15, "1"),
    (0x3feffffffffffffb, 16, "0.999999999999999"),
    (0x3feffffffffffffa, 16, "0.9999999999999993"),
    (0x3feffffffffffff9, 16, "0.9999999999999992"),
    (0x3feffffffffffff8, 16, "0.9999999999999991"),
    (0x3feffffffffffff7, 16, "0.999999999999999"),
    (0x3feffffffffffff6, 16, "0.9999999999999989"),
    (0x3feffffffffffff5, 16, "0.9999999999999988"),
    (0x3d5e98751fd929fa, 15, "0.000000000000434788733750604"),
    (0x3d5e98751fd929fa, 7, "0.0000000000004347887"),
    (0x3fc10cd84c924c23, 15, "0.133204495800414"),
    (0x3fc10cd84c924c23, 7, "0.1332045"),
    (0x3f1b0cca0f30aabb, 15, "0.000103187401507536"),
    (0x3f1b0cca0f30aabb, 7, "0.0001031874"),
    (0x3e2a1c6eaab390a4, 15, "0.00000000303972787705654"),
    (0x3e2a1c6eaab390a4, 7, "0.000000003039728"),
    (0x3e5f55cf5a036778, 15, "0.0000000291831746553505"),
    (0x3e5f55cf5a036778, 7, "0.00000002918317"),
    (0x3de17daa6b494949, 15, "0.000000000127262913439342"),
    (0x3de17daa6b494949, 7, "0.0000000001272629"),
    (0x3e35259ef5740a08, 15, "0.00000000492365955580662"),
    (0x3e35259ef5740a08, 7, "0.00000000492366"),
    (0x3e9ee145e3af2ddc, 15, "0.000000460147450528841"),
    (0x3e9ee145e3af2ddc, 7, "0.0000004601475"),
    (0x3d5c1c8bdc65684e, 15, "0.000000000000399488571763304"),
    (0x3d5c1c8bdc65684e, 7, "0.0000000000003994886"),
    (0x3ef37c6c30ce927c, 15, "0.000018583322389925"),
    (0x3ef37c6c30ce927c, 7, "0.00001858332"),
    (0x3f6a1b061858ee78, 15, "0.00318671408129295"),
    (0x3f6a1b061858ee78, 7, "0.003186714"),
    (0x3f90e00f3aa78127, 15, "0.0164797191190484"),
    (0x3f90e00f3aa78127, 7, "0.01647972"),
    (0x3e96ea497688866a, 15, "0.000000341462842475427"),
    (0x3e96ea497688866a, 7, "0.0000003414628"),
    (0x3e8c2e94cfca7efd, 15, "0.000000209971950844601"),
    (0x3e8c2e94cfca7efd, 7, "0.000000209972"),
    (0x3d440f0926778eaa, 15, "0.000000000000142525872859627"),
    (0x3d440f0926778eaa, 7, "0.0000000000001425259"),
    (0x3ee81dc172b961e1, 15, "0.0000114995160266787"),
    (0x3ee81dc172b961e1, 7, "0.00001149952"),
    (0x3d80551e07a32b74, 15, "0.0000000000018567890798403"),
    (0x3d80551e07a32b74, 7, "0.000000000001856789"),
    (0x3ebebcc1a16ce0c2, 15, "0.00000183208761024398"),
    (0x3ebebcc1a16ce0c2, 7, "0.000001832088"),
    (0x3fb06cee8c6659d0, 15, "0.0641621678135842"),
    (0x3fb06cee8c6659d0, 7, "0.06416217"),
    (0x3f13ae090240f061, 15, "0.0000750725744759775"),
    (0x3f13ae090240f061, 7, "0.00007507257"),
];

#[test]
fn intl_standard_matches_chrome() {
    for (bits, precision, want) in CASES {
        let x = f64::from_bits(*bits);
        assert_eq!(
            intl_standard(x, *precision),
            *want,
            "{x:e} with {precision} significant digits"
        );
    }
}

#[test]
fn nearly_one_prints_like_icu() {
    // 1 - 5 ulp: the approximate product is exactly ...999.5, ICU rounds it up to 1 (15 digits).
    assert_eq!(intl_standard(0.9999999999999994, 15), "1");
    assert_eq!(intl_standard(0.9999999999999993, 15), "0.999999999999999");
    // 16 digits: the same fast path gives 15 digits.
    assert_eq!(intl_standard(0.9999999999999994, 16), "0.999999999999999");
}
