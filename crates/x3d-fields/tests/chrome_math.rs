//! Chrome 155 `Math` results that differ from glibc (and from node's fdlibm): the Chrome flavor of
//! `jsmath` is correctly rounded (CORE-MATH). Vectors: hex bits of the input and of Chrome's result,
//! captured in headless Chrome (oracle/README.md, "Followers"). A few per function that glibc gets wrong.

use x3d_fields::jsmath::{self, MathFlavor};

fn chrome() {
    jsmath::set_flavor(MathFlavor::Chrome);
}

const ACOS: &[(u64, u64)] = &[
    (0x3fef50df0409c3fc, 0x3fca83bed7f69649),
    (0x3fecf30d66a2d5f6, 0x3fdc2c1ee829ed01),
    (0x3fd884816b87d330, 0x3ff2d7b1daa90a6f),
    (0x3fe4c2da34b282a0, 0x3febac8353fb7699),
    (0xbfd906c5aec92f78, 0x3fff8f96b1e1bf92),
    (0x3fea96c7b5683c66, 0x3fe2e1cb3d823e2d),
    (0x3fe2e0c6fc65da54, 0x3fee12f3bc856193),
    (0x3fdc00bffece05dc, 0x3ff1e30956224ee3),
];

const ASIN: &[(u64, u64)] = &[
    (0xbfb98cc4c57da280, 0xbfb997ad1de03ce1),
    (0xbfd94f5956c124f8, 0xbfda055d496d3f67),
    (0xbfe9e14532a74550, 0xbfee250aaa929521),
    (0x3f98833ce08b0880, 0x3f9883d657c60558),
    (0x3fddbe06d9aa5b60, 0x3fdeeebfd7767bf7),
    (0xbfd684edef7d4fdc, 0xbfd7030c445d7669),
    (0xbfcf7e1c8a48dbb0, 0xbfcfd1bfaf66cc9f),
    (0x3fce842b3349ce00, 0x3fced02239eaedb9),
];

const SIN: &[(u64, u64)] = &[
    (0xbfc4d644da65bda0, 0xbfc4bebd80c34c7b),
    (0xc00b18b162781378, 0x3fcf1ad94ab666a3),
    (0x400b03976beb5720, 0xbfcdd2edfb5f0b65),
    (0xc00e05b6fbe3c732, 0x3fe25cf82689c635),
    (0x400a0f052caa5114, 0xbfbd904e7e0da65f),
    (0xc00a38529536ac0c, 0x3fc157c1859ab0c5),
    (0xbfd5d5a5f1f14bf0, 0xbfd569d825235469),
    (0xc004e6e382b5e6b4, 0xbfe02539fd704e81),
];

const COS: &[(u64, u64)] = &[
    (0x3feb22eaf76f67b0, 0x3fe52acdc024418d),
    (0x3ff1ce01251878f4, 0x3fdc4c4d7e78b1e5),
    (0x3ff32921a973f558, 0x3fd756637149ff05),
    (0x3fefcbd013106548, 0x3fe175fafb8f0d95),
    (0xbffeb29b61d06a98, 0xbfd5d04d3011209b),
    (0xbfff312e5d66aa34, 0xbfd7a98a46e8f627),
    (0x4001cc7917628b0e, 0xbfe37804ff5a92dd),
    (0xbfe99fe047ff1b30, 0x3fe646eae0aed533),
];

const TAN: &[(u64, u64)] = &[
    (0x3fc691e7d2715320, 0x3fc6ce8a79d39c49),
    (0x3ff505a153b86bac, 0x400e73211f7a4d3f),
    (0xbfe016adbc016408, 0xbfe198cd3b51e153),
    (0xc0084b532498b29c, 0x3fbaee49c2144175),
    (0x3fbf4657f99eca80, 0x3fbf6e69f691d8dd),
    (0x40087dbf85a5cc16, 0xbfb492c54374c738),
    (0xbfe613ad768d9308, 0xbfea67cdf2f38a55),
    (0xbffb19e9b3961d90, 0x40202c90eb667c05),
];

const EXP: &[(u64, u64)] = &[
    (0xbfcb3f48361c015c, 0x3fe9dd48c736f9dd),
    (0xbfd65bd31fcf2fc5, 0x3fe690883cb4d961),
    (0xbfac4b5050090df0, 0x3fee47930b69b35f),
    (0xbfbe09f25bdb5dc0, 0x3fec7502865cd677),
    (0xbfd709cae7190aa8, 0x3fe65385421235f0),
    (0xbfd93c9baecfebd1, 0x3fe5928480b2949c),
    (0xbfbb329833efdc18, 0x3fecc6501f6998d6),
    (0xbfd6cd7f5123aa91, 0x3fe66897d8c993c4),
];

const LOG: &[(u64, u64)] = &[
    (0x400fc08a2f5e396b, 0x3ff60e68766ca6ae),
    (0x3ffd41392884e5aa, 0x3fe34f7f302e0f66),
    (0x3feada6a1d1dbc85, 0xbfc671ebc25ea783),
    (0x3ff1979ee2ed3852, 0x3fb8497379996af2),
    (0x4004416c6e1a5123, 0x3fedba44a9ffb5b9),
    (0x3fea395295c5fdf5, 0xbfc97aee70a8b652),
    (0x3ff21fc86baab586, 0x3fbfe9802a379761),
    (0x4004a59c4b340931, 0x3fee570908ec54ba),
];

const LOG10: &[(u64, u64)] = &[
    (0x3ff61b51fb0c688a, 0x3fc1f8c4d0a16652),
    (0x407812afceeb1698, 0x4004af69533b397b),
    (0x40683d4871821052, 0x40024d07514796dd),
    (0x406dc979d3349269, 0x40030455e759ac21),
    (0x40782163d0826e25, 0x4004b187ebeb760d),
    (0x403c05b2308a30e2, 0x3ff728f8e99df689),
    (0x403d11bbee6c47dd, 0x3ff76a39f65264dd),
    (0x4078f31ae5986181, 0x4004cf39990033f7),
];

const ATAN2: &[(u64, u64, u64)] = &[
    (0x3fb10471cbe5a458, 0x3fdc72159a31c72a, 0x3fc300bc52766b73),
    (0xbfc3a1527ea70264, 0x3fbc75ec022b32c0, 0xbfee317acfb4e653),
    (0xbfdd9797877b9076, 0x3fab844e658e80b0, 0xbff748040a362fc1),
    (0x3fd6f6b800d3b0e8, 0x3fdfd10920d00678, 0x3fe40174ddbb9b33),
    (0xbfca8f7160a7a50c, 0x3fd93a3f4f48c4ee, 0xbfdf02f72d642f7f),
    (0xbfb4b0a62f9b4d88, 0x3fdfab3c3e39c608, 0xbfc4b92ef8868213),
    (0xbfd051a106f1c530, 0x3fdff9ccb9b9da48, 0xbfde338261c48637),
    (0xbfdfd3a77b504bcc, 0x3fa7d97341f293c0, 0xbff7a36e2fd4aab3),
];

fn check(name: &str, table: &[(u64, u64)], f: fn(f64) -> f64) {
    chrome();
    for (x, want) in table {
        let got = f(f64::from_bits(*x));
        assert_eq!(
            got.to_bits(),
            *want,
            "{name}({:e}) = {:e}, Chrome {:e}",
            f64::from_bits(*x),
            got,
            f64::from_bits(*want)
        );
    }
}

#[test]
fn chrome_math_is_correctly_rounded() {
    check("acos", ACOS, jsmath::acos);
    check("asin", ASIN, jsmath::asin);
    check("sin", SIN, jsmath::sin);
    check("cos", COS, jsmath::cos);
    check("tan", TAN, jsmath::tan);
    check("exp", EXP, jsmath::exp);
    check("log", LOG, jsmath::log);
    check("log10", LOG10, jsmath::log10);

    for (y, x, want) in ATAN2 {
        let got = jsmath::atan2(f64::from_bits(*y), f64::from_bits(*x));
        assert_eq!(got.to_bits(), *want, "atan2");
    }
}
